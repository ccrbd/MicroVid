//! Job ingestion, probing, scheduling and the ffmpeg runner.
use crate::command::{self, BuildInput};
use crate::ffmpeg::{self, hide_console_tokio, ProgressLine};
use crate::models::*;
use crate::state::{now, AppState, RunHandle};
use crate::{cropdetect, power, probe, subtitles};
use anyhow::{anyhow, Context, Result};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tauri::Emitter;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::sync::Notify;
use walkdir::WalkDir;

/// Collect video files from a mix of files and folders. Returns (source, root) pairs.
pub fn collect_sources(paths: &[String], recursive: bool) -> Vec<(PathBuf, PathBuf)> {
    let mut out = vec![];
    for p in paths {
        let path = PathBuf::from(p);
        if path.is_dir() {
            let walker = if recursive { WalkDir::new(&path) } else { WalkDir::new(&path).max_depth(1) };
            let mut files: Vec<PathBuf> = walker
                .into_iter()
                .filter_entry(|e| !e.file_name().to_string_lossy().starts_with('.'))
                .flatten()
                .filter(|e| e.file_type().is_file() && probe::is_video_file(e.path()))
                .map(|e| e.path().to_path_buf())
                .collect();
            files.sort();
            out.extend(files.into_iter().map(|f| (f, path.clone())));
        } else if path.is_file() && probe::is_video_file(&path) {
            let root = path.parent().map(|d| d.to_path_buf()).unwrap_or_default();
            out.push((path, root));
        }
    }
    out
}

pub fn add_sources(state: &Arc<AppState>, paths: &[String]) -> usize {
    let settings = state.settings();
    let sources = collect_sources(paths, settings.recursive);
    let mut new_ids = vec![];
    {
        let mut q = state.queue.lock().unwrap();
        for (src, root) in sources {
            let src_s = src.to_string_lossy().into_owned();
            if q.jobs.iter().any(|j| j.source == src_s && !j.status.is_finished()) {
                continue;
            }
            q.next_order += 1;
            let mut job = Job {
                id: uuid::Uuid::new_v4().to_string(),
                source: src_s,
                root: root.to_string_lossy().into_owned(),
                settings: settings.defaults.clone(),
                status: JobStatus::Probing,
                created_at: now(),
                in_size: std::fs::metadata(&src).map(|m| m.len()).unwrap_or(0),
                order: q.next_order,
                ..Default::default()
            };
            job.output = state.compute_output(&job);
            new_ids.push(job.id.clone());
            q.jobs.push(job);
        }
    }
    let jobs = state.queue.lock().unwrap().jobs.clone();
    let _ = state.db.upsert_jobs(&jobs);
    state.emit_jobs();
    for id in &new_ids {
        spawn_probe(state.clone(), id.clone());
    }
    new_ids.len()
}

pub fn spawn_probe(state: Arc<AppState>, id: String) {
    tauri::async_runtime::spawn(async move {
        let _permit = state.probe_sem.acquire().await;
        let Some(job) = state.get_job(&id) else { return };
        let caps = match state.caps() {
            Ok(c) => c,
            Err(e) => {
                state.with_job(&id, |j| {
                    j.status = JobStatus::Failed;
                    j.error = Some(e.to_string());
                });
                state.emit_jobs();
                return;
            }
        };
        let src = PathBuf::from(&job.source);
        match probe::probe(Path::new(&caps.ffprobe_path), &src).await {
            Ok(info) => {
                let crop = if job.settings.crop == CropMode::Auto { cropdetect::detect(Path::new(&caps.ffmpeg_path), &info).await } else { None };
                let cands = subtitles::find_candidates(&src);
                state.with_job(&id, |j| {
                    j.info = Some(info);
                    j.crop = crop;
                    j.sub_candidates = cands;
                    if j.status == JobStatus::Probing {
                        j.status = JobStatus::Pending;
                    }
                });
                let job = state.get_job(&id);
                if let Some(mut j) = job {
                    state.refresh_derived(&mut j);
                    state.with_job(&id, |x| {
                        x.estimate = j.estimate.clone();
                        x.output = j.output.clone();
                        x.auto_subtitle = j.auto_subtitle.clone();
                    });
                }
            }
            Err(e) => {
                state.with_job(&id, |j| {
                    j.status = JobStatus::Failed;
                    j.error = Some(format!("Could not read file: {e}"));
                });
            }
        }
        state.emit_jobs();
        tick(&state);
    });
}

/// Start runners while capacity allows.
pub fn tick(state: &Arc<AppState>) {
    let parallel = state.parallel_jobs() as usize;
    let mut to_start = vec![];
    {
        let mut q = state.queue.lock().unwrap();
        if q.paused {
            return;
        }
        let mut running = q.running.len();
        let mut order: Vec<usize> = (0..q.jobs.len()).collect();
        order.sort_by_key(|&i| q.jobs[i].order);
        for i in order {
            if running >= parallel {
                break;
            }
            if q.jobs[i].status == JobStatus::Pending && q.jobs[i].info.is_some() {
                q.jobs[i].status = JobStatus::Running;
                q.jobs[i].started_at = Some(now());
                q.jobs[i].progress = Progress::default();
                q.jobs[i].error = None;
                let id = q.jobs[i].id.clone();
                q.running.insert(
                    id.clone(),
                    RunHandle { pid: None, cancel: Arc::new(Notify::new()), cancelled: Arc::new(AtomicBool::new(false)), concurrency: (running + 1) as u32 },
                );
                running += 1;
                to_start.push(id);
            }
        }
    }
    if to_start.is_empty() {
        maybe_finished(state);
        return;
    }
    if state.settings().prevent_sleep {
        state.sleep.acquire();
    }
    state.was_active.store(true, Ordering::SeqCst);
    state.emit_jobs();
    for id in to_start {
        let st = state.clone();
        tauri::async_runtime::spawn(async move { run_job(st, id).await });
    }
}

fn maybe_finished(state: &Arc<AppState>) {
    let (idle, summary) = {
        let q = state.queue.lock().unwrap();
        let has_work = q.jobs.iter().any(|j| matches!(j.status, JobStatus::Running | JobStatus::Pending | JobStatus::Probing));
        let done: Vec<&Job> = q.jobs.iter().filter(|j| j.status == JobStatus::Done).collect();
        let failed = q.jobs.iter().filter(|j| j.status == JobStatus::Failed).count();
        let in_b: u64 = done.iter().map(|j| j.in_size).sum();
        let out_b: u64 = done.iter().map(|j| j.out_size.unwrap_or(0)).sum();
        (!has_work && q.running.is_empty(), serde_json::json!({ "done": done.len(), "failed": failed, "in_bytes": in_b, "out_bytes": out_b }))
    };
    if idle {
        state.sleep.release();
    }
    if idle && state.was_active.swap(false, Ordering::SeqCst) {
        let _ = state.app.emit("queue:finished", summary);
        run_post_action(&state.settings().post_queue_action);
    }
}

/// "sleep" / "shutdown" after the queue drains; "none" and "notify" do nothing here.
fn run_post_action(action: &str) {
    let mut cmd = match (action, std::env::consts::OS) {
        ("sleep", "macos") => { let mut c = std::process::Command::new("pmset"); c.arg("sleepnow"); c }
        ("sleep", "windows") => { let mut c = std::process::Command::new("rundll32.exe"); c.args(["powrprof.dll,SetSuspendState", "0,1,0"]); c }
        ("sleep", _) => { let mut c = std::process::Command::new("systemctl"); c.arg("suspend"); c }
        ("shutdown", "macos") => { let mut c = std::process::Command::new("osascript"); c.args(["-e", "tell application \"System Events\" to shut down"]); c }
        ("shutdown", "windows") => { let mut c = std::process::Command::new("shutdown"); c.args(["/s", "/t", "60"]); c }
        ("shutdown", _) => { let mut c = std::process::Command::new("systemctl"); c.arg("poweroff"); c }
        _ => return,
    };
    crate::ffmpeg::hide_console(&mut cmd);
    let _ = cmd.spawn();
}

pub fn pause(state: &Arc<AppState>) {
    let mut q = state.queue.lock().unwrap();
    q.paused = true;
    for h in q.running.values() {
        if let Some(pid) = h.pid {
            power::suspend(pid);
        }
    }
}

pub fn resume(state: &Arc<AppState>) {
    {
        let mut q = state.queue.lock().unwrap();
        q.paused = false;
        for h in q.running.values() {
            if let Some(pid) = h.pid {
                power::resume(pid);
            }
        }
    }
    tick(state);
}

pub fn cancel(state: &Arc<AppState>, id: &str) {
    let handle = {
        let q = state.queue.lock().unwrap();
        q.running.get(id).map(|h| (h.cancel.clone(), h.cancelled.clone(), h.pid))
    };
    if let Some((notify, flag, pid)) = handle {
        flag.store(true, Ordering::SeqCst);
        if let Some(pid) = pid {
            power::resume(pid); // a suspended process cannot die
        }
        notify.notify_waiters();
        notify.notify_one();
    }
}

/// Prepare the external subtitle: pick it, and shift it into a temp file when burning in with a delay.
fn resolve_external_sub(state: &AppState, job: &Job) -> Result<Option<PathBuf>> {
    let s = &job.settings.subtitles;
    let chosen = match s.mode {
        SubtitleMode::File => s.file.clone(),
        SubtitleMode::Auto => job.auto_subtitle.clone(),
        SubtitleMode::Source | SubtitleMode::None => None,
    };
    let Some(path) = chosen else { return Ok(None) };
    let path = PathBuf::from(path);
    if !path.exists() {
        return Err(anyhow!("subtitle file not found: {}", path.display()));
    }
    if s.burn_in && s.delay_ms != 0 && path.extension().and_then(|e| e.to_str()).map(|e| e.eq_ignore_ascii_case("srt")).unwrap_or(false) {
        let text = std::fs::read_to_string(&path).context("reading subtitle")?;
        let shifted = subtitles::shift_srt(&text, s.delay_ms);
        let tmp = state.cache_dir.join(format!("{}-shifted.srt", job.id));
        std::fs::write(&tmp, shifted)?;
        return Ok(Some(tmp));
    }
    Ok(Some(path))
}

async fn run_job(state: Arc<AppState>, id: String) {
    let result = run_job_inner(&state, &id).await;
    let concurrency = {
        let mut q = state.queue.lock().unwrap();
        let h = q.running.remove(&id);
        h.map(|h| h.concurrency).unwrap_or(1)
    };
    let finished = state.get_job(&id);
    match (result, finished) {
        (Ok(outcome), Some(_)) => {
            state.with_job(&id, |j| {
                j.status = outcome;
                j.finished_at = Some(now());
                j.pid = None;
                if outcome != JobStatus::Done {
                    j.progress.eta_secs = None;
                }
            });
            if outcome == JobStatus::Done {
                if let Some(j) = state.get_job(&id) {
                    let _ = state.db.add_history(&j, concurrency);
                    if let (Some(fps), Some(e)) = (j.avg_fps, j.estimate.as_ref()) {
                        let key = crate::estimate::calibration_key(&j.settings, e.out_height);
                        let _ = state.db.update_calibration(&key, fps * concurrency as f64);
                        state.refresh_pending_estimates();
                    }
                }
            }
        }
        (Err(e), _) => {
            state.with_job(&id, |j| {
                j.status = JobStatus::Failed;
                j.error = Some(e.to_string());
                j.finished_at = Some(now());
                j.pid = None;
                j.progress.eta_secs = None;
            });
        }
        _ => {}
    }
    state.emit_jobs();
    tick(&state);
}

async fn run_job_inner(state: &Arc<AppState>, id: &str) -> Result<JobStatus> {
    let job = state.get_job(id).ok_or_else(|| anyhow!("job vanished"))?;
    let settings = state.settings();
    let caps = state.caps()?;
    let info = job.info.clone().ok_or_else(|| anyhow!("file was not analysed"))?;
    let output = PathBuf::from(&job.output);
    if settings.skip_existing && output.exists() {
        state.with_job(id, |j| j.error = Some("Output already exists".into()));
        return Ok(JobStatus::Skipped);
    }
    if let Some(parent) = output.parent() {
        std::fs::create_dir_all(parent).with_context(|| format!("creating {}", parent.display()))?;
    }
    let external_sub = resolve_external_sub(state, &job)?;
    let part = PathBuf::from(format!("{}.part", job.output));
    let args = command::build_args(BuildInput {
        info: &info,
        settings: &job.settings,
        crop: job.crop,
        external_sub: external_sub.as_deref(),
        output: &part,
        caps: &caps,
        clip: None,
    })?;

    let (cancel, cancelled) = {
        let q = state.queue.lock().unwrap();
        let h = q.running.get(id).ok_or_else(|| anyhow!("no run handle"))?;
        (h.cancel.clone(), h.cancelled.clone())
    };

    let mut cmd = tokio::process::Command::new(&caps.ffmpeg_path);
    cmd.args(&args).stdin(std::process::Stdio::null()).stdout(std::process::Stdio::piped()).stderr(std::process::Stdio::piped()).kill_on_drop(true);
    hide_console_tokio(&mut cmd);
    let started = std::time::Instant::now();
    let mut child = cmd.spawn().with_context(|| format!("starting ffmpeg at {}", caps.ffmpeg_path))?;
    let pid = child.id();
    {
        let mut q = state.queue.lock().unwrap();
        if let Some(h) = q.running.get_mut(id) {
            h.pid = pid;
        }
        if q.paused {
            if let Some(p) = pid {
                power::suspend(p);
            }
        }
    }
    state.with_job(id, |j| {
        j.pid = pid;
        j.log_tail = format!("ffmpeg {}\n", args.iter().map(|a| if a.contains(' ') { format!("\"{a}\"") } else { a.clone() }).collect::<Vec<_>>().join(" "));
    });

    let stderr = child.stderr.take().unwrap();
    let stderr_task = tauri::async_runtime::spawn(async move {
        let mut lines = BufReader::new(stderr).lines();
        let mut tail: Vec<String> = vec![];
        while let Ok(Some(l)) = lines.next_line().await {
            tail.push(l);
            if tail.len() > 80 {
                tail.remove(0);
            }
        }
        tail
    });

    let stdout = child.stdout.take().unwrap();
    let mut lines = BufReader::new(stdout).lines();
    let mut acc = ProgressLine::default();
    let duration = info.duration_secs.max(0.001);
    let mut last_frame = 0u64;
    let mut was_cancelled = false;
    loop {
        tokio::select! {
            _ = cancel.notified() => {
                was_cancelled = true;
                let _ = child.start_kill();
                break;
            }
            line = lines.next_line() => {
                match line {
                    Ok(Some(l)) => {
                        if let Some(block) = ffmpeg::parse_progress_line(&mut acc, &l) {
                            let out_t = block.out_time_secs.unwrap_or(0.0).max(0.0);
                            let speed = block.speed.unwrap_or(0.0);
                            last_frame = block.frame.unwrap_or(last_frame);
                            let percent = (out_t / duration * 100.0).clamp(0.0, 100.0);
                            let eta = if speed > 0.05 { Some(((duration - out_t) / speed).max(0.0)) } else { None };
                            let snapshot = state.with_job(id, |j| {
                                j.progress = Progress {
                                    percent,
                                    frame: last_frame,
                                    fps: block.fps.unwrap_or(0.0),
                                    speed,
                                    out_time_secs: out_t,
                                    out_size_bytes: block.total_size.unwrap_or(0),
                                    eta_secs: eta,
                                };
                                j.clone()
                            });
                            if let Some(j) = snapshot { state.emit_job(&j); }
                            if block.end { break; }
                        }
                    }
                    Ok(None) => break,
                    Err(_) => break,
                }
            }
        }
    }
    if cancelled.load(Ordering::SeqCst) {
        was_cancelled = true;
    }
    let status = child.wait().await?;
    let tail = stderr_task.await.unwrap_or_default();
    state.with_job(id, |j| {
        j.log_tail.push_str(&tail.join("\n"));
    });
    let elapsed = started.elapsed().as_secs_f64();

    if was_cancelled || cancelled.load(Ordering::SeqCst) {
        let _ = std::fs::remove_file(&part);
        return Ok(JobStatus::Cancelled);
    }
    if !status.success() {
        let _ = std::fs::remove_file(&part);
        let msg = tail.iter().rev().find(|l| !l.trim().is_empty()).cloned().unwrap_or_else(|| format!("ffmpeg exited with {status}"));
        return Err(anyhow!("{msg}"));
    }
    if output.exists() {
        std::fs::remove_file(&output).ok();
    }
    std::fs::rename(&part, &output).with_context(|| format!("finalising {}", output.display()))?;
    let out_size = std::fs::metadata(&output).map(|m| m.len()).unwrap_or(0);
    let frames = if last_frame > 0 { last_frame as f64 } else { duration * info.video.as_ref().map(|v| v.fps).unwrap_or(24.0) };
    state.with_job(id, |j| {
        j.out_size = Some(out_size);
        j.elapsed_secs = Some(elapsed);
        j.avg_fps = Some(frames / elapsed.max(0.001));
        j.avg_speed = Some(duration / elapsed.max(0.001));
        j.progress.percent = 100.0;
        j.progress.eta_secs = Some(0.0);
    });
    Ok(JobStatus::Done)
}

/// Called once at startup: kill orphans, mark interrupted jobs, remove partial files.
pub fn recover(state: &Arc<AppState>) -> usize {
    let mut count = 0;
    let mut reprobe = vec![];
    {
        let mut q = state.queue.lock().unwrap();
        for j in q.jobs.iter_mut() {
            match j.status {
                JobStatus::Running => {
                    if let Some(pid) = j.pid {
                        power::kill_orphan_ffmpeg(pid);
                    }
                    let _ = std::fs::remove_file(format!("{}.part", j.output));
                    j.status = JobStatus::Interrupted;
                    j.pid = None;
                    j.progress = Progress::default();
                    j.error = Some("Interrupted by a crash or shutdown".into());
                    count += 1;
                }
                JobStatus::Probing => reprobe.push(j.id.clone()),
                _ => {}
            }
        }
        q.next_order = q.jobs.iter().map(|j| j.order).max().unwrap_or(0);
        q.paused = true;
    }
    let jobs = state.queue.lock().unwrap().jobs.clone();
    let _ = state.db.upsert_jobs(&jobs);
    for id in reprobe {
        spawn_probe(state.clone(), id);
    }
    count
}

/// Re-queue interrupted / failed / cancelled jobs.
pub fn retry(state: &Arc<AppState>, ids: &[String]) {
    {
        let mut q = state.queue.lock().unwrap();
        for j in q.jobs.iter_mut() {
            if ids.contains(&j.id) && matches!(j.status, JobStatus::Interrupted | JobStatus::Failed | JobStatus::Cancelled | JobStatus::Skipped) {
                j.status = if j.info.is_some() { JobStatus::Pending } else { JobStatus::Probing };
                j.error = None;
                j.progress = Progress::default();
                j.out_size = None;
                j.finished_at = None;
            }
        }
    }
    let jobs = state.queue.lock().unwrap().jobs.clone();
    let _ = state.db.upsert_jobs(&jobs);
    for j in jobs.iter().filter(|j| ids.contains(&j.id) && j.status == JobStatus::Probing) {
        spawn_probe(state.clone(), j.id.clone());
    }
    state.emit_jobs();
    tick(state);
}
