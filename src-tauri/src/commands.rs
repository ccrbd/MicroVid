//! Tauri command layer: thin wrappers that translate UI intents into state changes.
use crate::models::*;
use crate::state::AppState;
use crate::subtitles::OsSearchResult;
use crate::{queue, subtitles, testencode};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tauri::State;

type Res<T> = Result<T, String>;
fn err<E: std::fmt::Display>(e: E) -> String {
    e.to_string()
}

#[derive(Serialize)]
pub struct StartupInfo {
    pub interrupted: usize,
    pub machine: MachineInfo,
    pub settings: AppSettings,
    pub capabilities: Option<Capabilities>,
    pub capabilities_error: Option<String>,
    pub default_output_dir: String,
    pub jobs: Vec<Job>,
    pub paused: bool,
}

#[tauri::command]
pub fn startup_info(state: State<'_, Arc<AppState>>) -> StartupInfo {
    let caps = state.caps();
    let q = state.queue.lock().unwrap();
    StartupInfo {
        interrupted: state.startup_interrupted.load(std::sync::atomic::Ordering::SeqCst),
        machine: state.machine.clone(),
        settings: state.settings(),
        capabilities: caps.as_ref().ok().cloned(),
        capabilities_error: caps.err().map(|e| e.to_string()),
        default_output_dir: state.default_output_dir.to_string_lossy().into_owned(),
        jobs: q.jobs.clone(),
        paused: q.paused,
    }
}

#[tauri::command]
pub fn get_settings(state: State<'_, Arc<AppState>>) -> AppSettings {
    state.settings()
}

#[tauri::command]
pub fn set_settings(state: State<'_, Arc<AppState>>, settings: AppSettings) -> Res<AppSettings> {
    let old = state.settings();
    if old.ffmpeg_path != settings.ffmpeg_path {
        state.reset_caps();
    }
    state.db.save_settings(&settings).map_err(err)?;
    *state.settings.write().unwrap() = settings.clone();
    // Naming / output folder changes affect every unfinished job.
    if old.naming != settings.naming || old.output_dir != settings.output_dir || old.preserve_structure != settings.preserve_structure || old.parallel_jobs != settings.parallel_jobs {
        let mut jobs = state.queue.lock().unwrap().jobs.clone();
        for j in jobs.iter_mut().filter(|j| !j.status.is_finished() && j.status != JobStatus::Running) {
            state.refresh_derived(j);
        }
        {
            let mut q = state.queue.lock().unwrap();
            for j in jobs.iter() {
                if let Some(x) = q.jobs.iter_mut().find(|x| x.id == j.id) {
                    x.output = j.output.clone();
                    x.estimate = j.estimate.clone();
                }
            }
        }
        let _ = state.db.upsert_jobs(&jobs);
        state.emit_jobs();
    }
    let st: &Arc<AppState> = &state;
    queue::tick(st);
    Ok(settings)
}

#[tauri::command]
pub fn get_capabilities(state: State<'_, Arc<AppState>>) -> Res<Capabilities> {
    state.reset_caps();
    state.caps().map_err(err)
}

#[tauri::command]
pub fn get_machine_info(state: State<'_, Arc<AppState>>) -> MachineInfo {
    state.machine.clone()
}

#[tauri::command]
pub fn add_sources(state: State<'_, Arc<AppState>>, paths: Vec<String>) -> usize {
    let st: &Arc<AppState> = &state;
    queue::add_sources(st, &paths)
}

#[tauri::command]
pub fn list_jobs(state: State<'_, Arc<AppState>>) -> Vec<Job> {
    state.queue.lock().unwrap().jobs.clone()
}

#[tauri::command]
pub fn update_job_settings(state: State<'_, Arc<AppState>>, ids: Vec<String>, settings: EncodeSettings) -> Res<Vec<Job>> {
    let mut changed = vec![];
    {
        let mut q = state.queue.lock().unwrap();
        for j in q.jobs.iter_mut().filter(|j| ids.contains(&j.id) && j.status != JobStatus::Running) {
            j.settings = settings.clone();
            changed.push(j.clone());
        }
    }
    for j in changed.iter_mut() {
        state.refresh_derived(j);
        state.with_job(&j.id, |x| {
            x.estimate = j.estimate.clone();
            x.output = j.output.clone();
            x.auto_subtitle = j.auto_subtitle.clone();
        });
    }
    state.emit_jobs();
    Ok(changed)
}

#[tauri::command]
pub fn estimate_settings(state: State<'_, Arc<AppState>>, id: String, settings: EncodeSettings) -> Res<Estimate> {
    let job = state.get_job(&id).ok_or("job not found")?;
    state.compute_estimate(&job, &settings).ok_or_else(|| "file not analysed yet".into())
}

#[tauri::command]
pub fn preview_output_name(_state: State<'_, Arc<AppState>>, naming: NamingSettings, settings: EncodeSettings) -> String {
    crate::naming::output_file_name(Path::new("/x/The Wire S01E03.mkv"), &settings, &naming, settings.resolution.height().unwrap_or(1080))
}

#[tauri::command]
pub fn remove_jobs(state: State<'_, Arc<AppState>>, ids: Vec<String>) {
    let st: &Arc<AppState> = &state;
    for id in &ids {
        queue::cancel(st, id);
    }
    {
        let mut q = state.queue.lock().unwrap();
        q.jobs.retain(|j| !(ids.contains(&j.id) && j.status != JobStatus::Running));
    }
    for id in &ids {
        let _ = state.db.delete_job(id);
    }
    state.emit_jobs();
}

#[tauri::command]
pub fn clear_finished(state: State<'_, Arc<AppState>>) {
    let removed: Vec<String> = {
        let mut q = state.queue.lock().unwrap();
        let ids: Vec<String> = q.jobs.iter().filter(|j| j.status.is_finished()).map(|j| j.id.clone()).collect();
        q.jobs.retain(|j| !j.status.is_finished());
        ids
    };
    for id in removed {
        let _ = state.db.delete_job(&id);
    }
    state.emit_jobs();
}

#[tauri::command]
pub fn retry_jobs(state: State<'_, Arc<AppState>>, ids: Vec<String>) {
    let st: &Arc<AppState> = &state;
    queue::retry(st, &ids);
}

#[tauri::command]
pub fn cancel_jobs(state: State<'_, Arc<AppState>>, ids: Vec<String>) {
    let st: &Arc<AppState> = &state;
    for id in &ids {
        queue::cancel(st, id);
    }
    // Pending jobs that never started are simply marked cancelled.
    {
        let mut q = state.queue.lock().unwrap();
        for j in q.jobs.iter_mut().filter(|j| ids.contains(&j.id) && matches!(j.status, JobStatus::Pending | JobStatus::Interrupted)) {
            j.status = JobStatus::Cancelled;
        }
    }
    let jobs = state.queue.lock().unwrap().jobs.clone();
    let _ = state.db.upsert_jobs(&jobs);
    state.emit_jobs();
}

#[tauri::command]
pub fn reorder_jobs(state: State<'_, Arc<AppState>>, ids: Vec<String>) {
    {
        let mut q = state.queue.lock().unwrap();
        for (i, id) in ids.iter().enumerate() {
            if let Some(j) = q.jobs.iter_mut().find(|j| &j.id == id) {
                j.order = i as i64 + 1;
            }
        }
        q.jobs.sort_by_key(|j| j.order);
        q.next_order = q.jobs.len() as i64;
    }
    let jobs = state.queue.lock().unwrap().jobs.clone();
    let _ = state.db.upsert_jobs(&jobs);
    state.emit_jobs();
}

#[tauri::command]
pub fn start_queue(state: State<'_, Arc<AppState>>) {
    let st: &Arc<AppState> = &state;
    // Interrupted jobs are re-queued when the user presses start.
    let ids: Vec<String> = state.queue.lock().unwrap().jobs.iter().filter(|j| j.status == JobStatus::Interrupted).map(|j| j.id.clone()).collect();
    if !ids.is_empty() {
        queue::retry(st, &ids);
    }
    queue::resume(st);
    state.emit_jobs();
}

#[tauri::command]
pub fn pause_queue(state: State<'_, Arc<AppState>>) {
    let st: &Arc<AppState> = &state;
    queue::pause(st);
    state.emit_jobs();
}

#[tauri::command]
pub fn queue_stats(state: State<'_, Arc<AppState>>) -> QueueStats {
    state.stats(0.0)
}

#[tauri::command]
pub fn refresh_subtitles(state: State<'_, Arc<AppState>>, id: String) -> Res<Job> {
    let job = state.get_job(&id).ok_or("job not found")?;
    let cands = subtitles::find_candidates(Path::new(&job.source));
    let mut j = job.clone();
    j.sub_candidates = cands;
    state.refresh_derived(&mut j);
    state.with_job(&id, |x| {
        x.sub_candidates = j.sub_candidates.clone();
        x.auto_subtitle = j.auto_subtitle.clone();
    });
    state.emit_jobs();
    Ok(j)
}

#[derive(Deserialize)]
pub struct OsSearchArgs {
    pub id: String,
    pub query: Option<String>,
    pub languages: Option<String>,
}

fn os_lang_codes(settings: &AppSettings) -> String {
    if !settings.opensubtitles.languages.trim().is_empty() {
        return settings.opensubtitles.languages.trim().to_string();
    }
    match settings.defaults.subtitles.language.as_str() {
        "spa" => "es",
        "fre" => "fr",
        "ger" => "de",
        "ita" => "it",
        "por" => "pt-br,pt-pt",
        "rus" => "ru",
        "jpn" => "ja",
        "kor" => "ko",
        "chi" => "zh-cn,zh-tw",
        "ara" => "ar",
        "hin" => "hi",
        "ben" => "bn",
        "tur" => "tr",
        "dut" => "nl",
        _ => "en",
    }
    .to_string()
}

#[tauri::command]
pub async fn opensubtitles_search(state: State<'_, Arc<AppState>>, args: OsSearchArgs) -> Res<Vec<OsSearchResult>> {
    let settings = state.settings();
    if settings.opensubtitles.api_key.trim().is_empty() {
        return Err("Add your OpenSubtitles API key in Settings first".into());
    }
    let job = state.get_job(&args.id).ok_or("job not found")?;
    let src = PathBuf::from(&job.source);
    let hash = subtitles::movie_hash(&src).ok();
    let query = args.query.unwrap_or_else(|| src.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default());
    let langs = args.languages.filter(|l| !l.trim().is_empty()).unwrap_or_else(|| os_lang_codes(&settings));
    let size = std::fs::metadata(&src).ok().map(|m| m.len());
    subtitles::os_search(&settings.opensubtitles.api_key, &query, &langs, hash.as_deref(), size).await.map_err(err)
}

#[tauri::command]
pub async fn opensubtitles_download(state: State<'_, Arc<AppState>>, id: String, file_id: u64, language: String) -> Res<Job> {
    let settings = state.settings();
    let os = &settings.opensubtitles;
    if os.api_key.trim().is_empty() || os.username.trim().is_empty() {
        return Err("Add your OpenSubtitles API key, username and password in Settings".into());
    }
    let token = {
        let cached = state.os_token.lock().unwrap().clone();
        match cached {
            Some(t) => t,
            None => {
                let t = subtitles::os_login(&os.api_key, &os.username, &os.password).await.map_err(err)?;
                *state.os_token.lock().unwrap() = Some(t.clone());
                t
            }
        }
    };
    let job = state.get_job(&id).ok_or("job not found")?;
    let src = PathBuf::from(&job.source);
    let stem = src.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_else(|| "subtitle".into());
    let lang_tag = subtitles::parse_lang(&language).unwrap_or_else(|| language.clone());
    let dest_dir = if settings.save_subs_next_to_video { src.parent().map(|p| p.to_path_buf()).unwrap_or_else(|| state.cache_dir.clone()) } else { state.cache_dir.join("subs") };
    let dest = dest_dir.join(format!("{stem}.{language}.srt"));
    let path = match subtitles::os_download(&os.api_key, &token, file_id, &dest).await {
        Ok(p) => p,
        Err(e) => {
            // Token may have expired: retry once with a fresh login.
            *state.os_token.lock().unwrap() = None;
            let t = subtitles::os_login(&os.api_key, &os.username, &os.password).await.map_err(err)?;
            *state.os_token.lock().unwrap() = Some(t.clone());
            subtitles::os_download(&os.api_key, &t, file_id, &dest).await.map_err(|e2| format!("{e} / {e2}"))?
        }
    };
    let mut j = job.clone();
    let p = path.to_string_lossy().into_owned();
    j.sub_candidates.insert(0, SubCandidate { path: p.clone(), language: Some(lang_tag.clone()), source: "downloaded".into(), score: 110 });
    j.settings.subtitles.mode = SubtitleMode::File;
    j.settings.subtitles.file = Some(p);
    j.settings.subtitles.language = lang_tag;
    state.refresh_derived(&mut j);
    state.with_job(&id, |x| {
        x.sub_candidates = j.sub_candidates.clone();
        x.settings = j.settings.clone();
        x.auto_subtitle = j.auto_subtitle.clone();
        x.estimate = j.estimate.clone();
    });
    state.emit_jobs();
    Ok(j)
}

#[tauri::command]
pub async fn test_encode(state: State<'_, Arc<AppState>>, id: String, start_secs: Option<f64>) -> Res<TestEncodeResult> {
    let st: Arc<AppState> = state.inner().clone();
    testencode::test_encode(st, &id, start_secs).await.map_err(err)
}

#[tauri::command]
pub async fn benchmark(state: State<'_, Arc<AppState>>, id: String, max_jobs: u32) -> Res<Vec<BenchmarkPoint>> {
    let st: Arc<AppState> = state.inner().clone();
    testencode::benchmark(st, &id, max_jobs).await.map_err(err)
}

#[derive(Serialize)]
pub struct Analytics {
    pub history: Vec<HistoryRow>,
    pub calibration: Vec<CalibrationRow>,
    pub session_start: i64,
}

#[tauri::command]
pub fn get_analytics(state: State<'_, Arc<AppState>>) -> Analytics {
    Analytics { history: state.db.list_history(2000), calibration: state.db.list_calibration(), session_start: state.session_start }
}

#[tauri::command]
pub fn clear_history(state: State<'_, Arc<AppState>>) -> Res<()> {
    state.db.clear_history().map_err(err)
}

#[tauri::command]
pub fn export_history_csv(state: State<'_, Arc<AppState>>, path: String) -> Res<usize> {
    let rows = state.db.list_history(100_000);
    let mut s = String::from("finished_at,source,output,codec,resolution,crf,content_type,in_size,out_size,duration_secs,elapsed_secs,avg_fps,avg_speed,parallel_jobs\n");
    let q = |v: &str| format!("\"{}\"", v.replace('"', "\"\""));
    for r in &rows {
        s.push_str(&format!(
            "{},{},{},{},{},{},{},{},{},{:.1},{:.1},{:.2},{:.2},{}\n",
            r.finished_at, q(&r.source), q(&r.output), r.codec, r.resolution, r.crf, r.content_type, r.in_size, r.out_size, r.duration_secs, r.elapsed_secs, r.avg_fps, r.avg_speed, r.parallel_jobs
        ));
    }
    std::fs::write(&path, s).map_err(err)?;
    Ok(rows.len())
}

#[tauri::command]
pub fn reveal_path(path: String) -> Res<()> {
    let p = PathBuf::from(&path);
    let target = if p.exists() { p } else { p.parent().map(|x| x.to_path_buf()).unwrap_or(p) };
    tauri_plugin_opener::reveal_item_in_dir(target).map_err(err)
}

#[tauri::command]
pub fn open_path(path: String) -> Res<()> {
    tauri_plugin_opener::open_path(path, None::<&str>).map_err(err)
}

#[tauri::command]
pub fn delete_cache(state: State<'_, Arc<AppState>>) -> Res<u64> {
    let mut freed = 0u64;
    if let Ok(rd) = std::fs::read_dir(&state.cache_dir) {
        for e in rd.flatten() {
            let p = e.path();
            if p.is_file() {
                freed += e.metadata().map(|m| m.len()).unwrap_or(0);
                let _ = std::fs::remove_file(p);
            }
        }
    }
    Ok(freed)
}

#[tauri::command]
pub fn path_exists(path: String) -> bool {
    Path::new(&path).exists()
}
