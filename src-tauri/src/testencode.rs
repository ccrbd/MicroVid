//! 30-second test encode from the middle of a file, frame comparison, and the parallel benchmark.
use crate::command::{self, BuildInput};
use crate::ffmpeg::{self, hide_console_tokio, ProgressLine};
use crate::models::*;
use crate::state::AppState;
use anyhow::{anyhow, Context, Result};
use base64::Engine;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::io::{AsyncBufReadExt, BufReader};

pub const CLIP_SECS: f64 = 30.0;

struct ClipRun {
    elapsed: f64,
    frames: u64,
    fps: f64,
    speed: f64,
}

async fn run_clip(ffmpeg_path: &str, args: &[String]) -> Result<ClipRun> {
    let mut cmd = tokio::process::Command::new(ffmpeg_path);
    cmd.args(args).stdin(std::process::Stdio::null()).stdout(std::process::Stdio::piped()).stderr(std::process::Stdio::piped()).kill_on_drop(true);
    hide_console_tokio(&mut cmd);
    let started = std::time::Instant::now();
    let mut child = cmd.spawn().context("starting ffmpeg")?;
    let stdout = child.stdout.take().unwrap();
    let stderr = child.stderr.take().unwrap();
    let err_task = tauri::async_runtime::spawn(async move {
        let mut lines = BufReader::new(stderr).lines();
        let mut v = vec![];
        while let Ok(Some(l)) = lines.next_line().await {
            v.push(l);
        }
        v
    });
    let mut lines = BufReader::new(stdout).lines();
    let mut acc = ProgressLine::default();
    let mut last = ProgressLine::default();
    while let Ok(Some(l)) = lines.next_line().await {
        if let Some(b) = ffmpeg::parse_progress_line(&mut acc, &l) {
            if b.frame.is_some() {
                last = b.clone();
            }
            if b.end {
                break;
            }
        }
    }
    let status = child.wait().await?;
    let elapsed = started.elapsed().as_secs_f64();
    if !status.success() {
        let err = err_task.await.unwrap_or_default();
        return Err(anyhow!("{}", err.last().cloned().unwrap_or_else(|| format!("ffmpeg exited with {status}"))));
    }
    let frames = last.frame.unwrap_or(0);
    Ok(ClipRun { elapsed, frames, fps: frames as f64 / elapsed.max(0.001), speed: last.speed.unwrap_or(0.0) })
}

async fn grab_frame(ffmpeg_path: &str, input: &Path, at: f64, w: u32, h: u32) -> Option<String> {
    let mut cmd = tokio::process::Command::new(ffmpeg_path);
    cmd.args(["-hide_banner", "-nostdin", "-loglevel", "error", "-ss", &format!("{at:.3}"), "-i"])
        .arg(input)
        .args(["-frames:v", "1", "-vf", &format!("scale={w}:{h}:flags=lanczos"), "-q:v", "3", "-f", "image2pipe", "-c:v", "mjpeg", "-"]);
    hide_console_tokio(&mut cmd);
    let out = cmd.output().await.ok()?;
    if !out.status.success() || out.stdout.is_empty() {
        return None;
    }
    Some(base64::engine::general_purpose::STANDARD.encode(&out.stdout))
}

pub async fn test_encode(state: Arc<AppState>, id: &str, start_secs: Option<f64>) -> Result<TestEncodeResult> {
    let job = state.get_job(id).ok_or_else(|| anyhow!("job not found"))?;
    let info = job.info.clone().ok_or_else(|| anyhow!("file not analysed yet"))?;
    let caps = state.caps()?;
    let dur = info.duration_secs;
    let clip = CLIP_SECS.min(dur.max(1.0));
    let start = start_secs.unwrap_or((dur / 2.0 - clip / 2.0).max(0.0)).clamp(0.0, (dur - clip).max(0.0));
    let out = state.cache_dir.join(format!("test-{}.{}", job.id, job.settings.container.ext()));
    let _ = std::fs::remove_file(&out);
    let args = command::build_args(BuildInput { info: &info, settings: &job.settings, crop: job.crop, external_sub: None, output: &out, caps: &caps, clip: Some((start, clip)) })?;
    let run = run_clip(&caps.ffmpeg_path, &args).await?;
    let clip_size = std::fs::metadata(&out).map(|m| m.len()).unwrap_or(0);
    let (w, h) = command::output_dims(&info, &job.settings, job.crop);
    let mid = start + clip / 2.0;
    let before = grab_frame(&caps.ffmpeg_path, Path::new(&info.path), mid, w, h).await;
    let after = grab_frame(&caps.ffmpeg_path, &out, clip / 2.0, w, h).await;
    // Feed the calibration table when the queue is idle (otherwise fps is shared).
    let idle = state.queue.lock().unwrap().running.is_empty();
    if idle && run.fps > 0.0 {
        let key = crate::estimate::calibration_key(&job.settings, h);
        let _ = state.db.update_calibration(&key, run.fps);
    }
    let factor = dur / clip.max(0.001);
    Ok(TestEncodeResult {
        clip_secs: clip,
        start_secs: start,
        out_path: out.to_string_lossy().into_owned(),
        clip_size_bytes: clip_size,
        extrapolated_size_bytes: (clip_size as f64 * factor) as u64,
        elapsed_secs: run.elapsed,
        fps: run.fps,
        speed: if run.speed > 0.0 { run.speed } else { clip / run.elapsed.max(0.001) },
        extrapolated_secs: run.elapsed * factor,
        before_jpeg_b64: before,
        after_jpeg_b64: after,
        out_width: w,
        out_height: h,
    })
}

/// Run a 15 s clip with 1, 2 and 3 simultaneous encoders and report total throughput.
pub async fn benchmark(state: Arc<AppState>, id: &str, max_jobs: u32) -> Result<Vec<BenchmarkPoint>> {
    let job = state.get_job(id).ok_or_else(|| anyhow!("job not found"))?;
    let info = job.info.clone().ok_or_else(|| anyhow!("file not analysed yet"))?;
    let caps = state.caps()?;
    if !state.queue.lock().unwrap().running.is_empty() {
        return Err(anyhow!("Pause the queue before running a benchmark"));
    }
    let dur = info.duration_secs;
    let clip = 15.0f64.min(dur.max(1.0));
    let start = (dur / 2.0 - clip / 2.0).max(0.0);
    let mut points = vec![];
    for n in 1..=max_jobs.clamp(1, 4) {
        let mut handles = vec![];
        for k in 0..n {
            let out: PathBuf = state.cache_dir.join(format!("bench-{k}.{}", job.settings.container.ext()));
            let args = command::build_args(BuildInput { info: &info, settings: &job.settings, crop: job.crop, external_sub: None, output: &out, caps: &caps, clip: Some((start, clip)) })?;
            let p = caps.ffmpeg_path.clone();
            handles.push(tauri::async_runtime::spawn(async move { run_clip(&p, &args).await }));
        }
        let mut total_frames = 0u64;
        let mut max_elapsed = 0.0f64;
        for h in handles {
            let r = h.await.map_err(|e| anyhow!("{e}"))??;
            total_frames += r.frames;
            max_elapsed = max_elapsed.max(r.elapsed);
        }
        let total_fps = total_frames as f64 / max_elapsed.max(0.001);
        points.push(BenchmarkPoint { jobs: n, total_fps, per_job_fps: total_fps / n as f64 });
        for k in 0..n {
            let _ = std::fs::remove_file(state.cache_dir.join(format!("bench-{k}.{}", job.settings.container.ext())));
        }
    }
    Ok(points)
}
