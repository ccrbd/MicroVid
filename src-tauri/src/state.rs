//! Shared application state and helpers used by the queue and the command layer.
use crate::db::Db;
use crate::estimate::{self, EstimateContext};
use crate::models::*;
use crate::naming;
use crate::power::SleepGuard;
use crate::{ffmpeg, subtitles};
use anyhow::Result;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicUsize};
use std::sync::{Arc, Mutex, RwLock};
use tauri::{AppHandle, Emitter};
use tokio::sync::Notify;

pub struct RunHandle {
    pub pid: Option<u32>,
    pub cancel: Arc<Notify>,
    pub cancelled: Arc<AtomicBool>,
    pub concurrency: u32,
}

#[derive(Default)]
pub struct QueueInner {
    pub jobs: Vec<Job>,
    pub running: HashMap<String, RunHandle>,
    pub paused: bool,
    pub next_order: i64,
}

pub struct AppState {
    pub app: AppHandle,
    pub db: Db,
    pub settings: RwLock<AppSettings>,
    pub caps: RwLock<Option<Capabilities>>,
    pub machine: MachineInfo,
    pub queue: Mutex<QueueInner>,
    pub sleep: SleepGuard,
    pub cache_dir: PathBuf,
    pub default_output_dir: PathBuf,
    pub os_token: Mutex<Option<String>>,
    pub probe_sem: Arc<tokio::sync::Semaphore>,
    pub startup_interrupted: AtomicUsize,
    pub session_start: i64,
    /// Set when a runner starts; cleared when the queue drains, so `queue:finished` fires once.
    pub was_active: AtomicBool,
}

pub fn now() -> i64 {
    chrono::Utc::now().timestamp()
}

impl AppState {
    pub fn settings(&self) -> AppSettings {
        self.settings.read().unwrap().clone()
    }

    pub fn parallel_jobs(&self) -> u32 {
        let s = self.settings.read().unwrap();
        if s.parallel_jobs == 0 {
            self.machine.suggested_jobs
        } else {
            s.parallel_jobs.clamp(1, 6)
        }
    }

    /// Resolve ffmpeg lazily and cache the result; a custom path change clears the cache.
    pub fn caps(&self) -> Result<Capabilities> {
        if let Some(c) = self.caps.read().unwrap().as_ref() {
            return Ok(c.clone());
        }
        let custom = self.settings.read().unwrap().ffmpeg_path.clone();
        let c = ffmpeg::capabilities(custom.as_deref())?;
        *self.caps.write().unwrap() = Some(c.clone());
        Ok(c)
    }

    pub fn reset_caps(&self) {
        *self.caps.write().unwrap() = None;
    }

    pub fn compute_output(&self, job: &Job) -> String {
        let s = self.settings.read().unwrap();
        let out_h = job.estimate.as_ref().map(|e| e.out_height).unwrap_or_else(|| {
            job.settings.resolution.height().unwrap_or_else(|| job.info.as_ref().and_then(|i| i.video.as_ref()).map(|v| v.height).unwrap_or(0))
        });
        let dir = s.output_dir.as_ref().map(PathBuf::from).unwrap_or_else(|| self.default_output_dir.clone());
        naming::output_path(Path::new(&job.source), Path::new(&job.root), &dir, s.preserve_structure, &job.settings, &s.naming, out_h)
            .to_string_lossy()
            .into_owned()
    }

    pub fn compute_estimate(&self, job: &Job, settings: &EncodeSettings) -> Option<Estimate> {
        let info = job.info.as_ref()?;
        let (_, out_h) = crate::command::output_dims(info, settings, job.crop);
        let key = estimate::calibration_key(settings, out_h);
        let ctx = EstimateContext {
            calibrated_throughput: self.db.calibration(&key).map(|(t, _)| t),
            physical_cores: self.machine.physical_cores,
            parallel_jobs: self.parallel_jobs(),
        };
        Some(estimate::estimate(info, settings, job.crop, &ctx))
    }

    /// Refresh estimates of every waiting job (after calibration or a parallel-jobs change).
    pub fn refresh_pending_estimates(&self) {
        let mut jobs: Vec<Job> = self.queue.lock().unwrap().jobs.iter().filter(|j| matches!(j.status, JobStatus::Pending | JobStatus::Interrupted)).cloned().collect();
        for j in jobs.iter_mut() {
            j.estimate = self.compute_estimate(j, &j.settings.clone());
        }
        let mut q = self.queue.lock().unwrap();
        for j in jobs {
            if let Some(x) = q.jobs.iter_mut().find(|x| x.id == j.id) {
                x.estimate = j.estimate;
            }
        }
    }

    /// Recompute derived fields (estimate, output path, auto subtitle) after settings or info change.
    pub fn refresh_derived(&self, job: &mut Job) {
        job.estimate = self.compute_estimate(job, &job.settings.clone());
        job.output = self.compute_output(job);
        let lang = job.settings.subtitles.language.clone();
        job.auto_subtitle = subtitles::best_candidate(&job.sub_candidates, &lang).map(|c| c.path.clone());
    }

    pub fn emit_jobs(&self) {
        let jobs = self.queue.lock().unwrap().jobs.clone();
        let _ = self.app.emit("queue:changed", jobs);
    }

    pub fn emit_job(&self, job: &Job) {
        let _ = self.app.emit("job:progress", job);
    }

    pub fn persist(&self, job: &Job) {
        let _ = self.db.upsert_job(job);
    }

    pub fn with_job<R>(&self, id: &str, f: impl FnOnce(&mut Job) -> R) -> Option<R> {
        let mut q = self.queue.lock().unwrap();
        let job = q.jobs.iter_mut().find(|j| j.id == id)?;
        let r = f(job);
        let snapshot = job.clone();
        drop(q);
        self.persist(&snapshot);
        Some(r)
    }

    pub fn get_job(&self, id: &str) -> Option<Job> {
        self.queue.lock().unwrap().jobs.iter().find(|j| j.id == id).cloned()
    }

    pub fn stats(&self, cpu_percent: f32) -> QueueStats {
        let q = self.queue.lock().unwrap();
        let parallel = self.parallel_jobs();
        let mut st = QueueStats { parallel_jobs: parallel, paused: q.paused, cpu_percent, ..Default::default() };
        let mut remaining = 0.0f64;
        let mut pending_secs = 0.0f64;
        for j in &q.jobs {
            st.total += 1;
            match j.status {
                JobStatus::Done => {
                    st.done += 1;
                    st.in_bytes_done += j.in_size;
                    st.out_bytes_done += j.out_size.unwrap_or(0);
                }
                JobStatus::Running => {
                    st.running += 1;
                    st.fps += j.progress.fps;
                    st.speed += j.progress.speed;
                    if let Some(e) = j.progress.eta_secs {
                        remaining = remaining.max(e);
                    } else if let Some(e) = &j.estimate {
                        remaining = remaining.max(e.seconds);
                    }
                }
                JobStatus::Pending | JobStatus::Probing | JobStatus::Interrupted => {
                    st.pending += 1;
                    pending_secs += j.estimate.as_ref().map(|e| e.seconds).unwrap_or(0.0);
                }
                JobStatus::Failed => st.failed += 1,
                _ => {}
            }
        }
        if st.running > 0 {
            st.speed /= st.running as f64;
        }
        if st.running > 0 || st.pending > 0 {
            st.eta_secs = Some(remaining + pending_secs / parallel.max(1) as f64);
        }
        st
    }
}
