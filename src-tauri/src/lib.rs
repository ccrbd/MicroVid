pub mod command;
mod commands;
pub mod cropdetect;
mod db;
mod estimate;
pub mod ffmpeg;
mod machine;
pub mod models;
mod naming;
mod power;
pub mod probe;
mod queue;
mod state;
pub mod subtitles;
mod testencode;

use state::{AppState, QueueInner};
use std::sync::atomic::Ordering;
use std::sync::{Arc, Mutex, RwLock};
use tauri::{Emitter, Manager};

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_notification::init())
        .setup(|app| {
            let data_dir = app.path().app_data_dir()?;
            let cache_dir = app.path().app_cache_dir()?;
            std::fs::create_dir_all(&data_dir)?;
            std::fs::create_dir_all(&cache_dir)?;
            let db = db::Db::open(&data_dir.join("microvid.db"))?;
            let settings = db.load_settings();
            let jobs = db.load_jobs();
            let default_output_dir = app.path().video_dir().unwrap_or_else(|_| data_dir.clone()).join("MicroVid");
            let machine = machine::info();
            let state = Arc::new(AppState {
                app: app.handle().clone(),
                db,
                settings: RwLock::new(settings),
                caps: RwLock::new(None),
                machine,
                queue: Mutex::new(QueueInner { jobs, running: Default::default(), paused: true, next_order: 0 }),
                sleep: power::SleepGuard::new(),
                cache_dir,
                default_output_dir,
                os_token: Mutex::new(None),
                probe_sem: Arc::new(tokio::sync::Semaphore::new(3)),
                startup_interrupted: Default::default(),
                session_start: state::now(),
                was_active: Default::default(),
            });
            let interrupted = queue::recover(&state);
            state.startup_interrupted.store(interrupted, Ordering::SeqCst);
            app.manage(state.clone());

            // Dev hook: MICROVID_AUTO_ADD=/path/to/folder adds it and starts the queue after launch.
            if let Ok(path) = std::env::var("MICROVID_AUTO_ADD") {
                if let Ok(out) = std::env::var("MICROVID_AUTO_OUT") {
                    state.settings.write().unwrap().output_dir = Some(out);
                }
                let st = state.clone();
                tauri::async_runtime::spawn(async move {
                    tokio::time::sleep(std::time::Duration::from_secs(3)).await;
                    // Same thread a drag-drop IPC command runs on.
                    let (st2, path2) = (st.clone(), path.clone());
                    let _ = st.app.run_on_main_thread(move || {
                        queue::add_sources(&st2, &[path2]);
                    });
                    tokio::time::sleep(std::time::Duration::from_secs(6)).await;
                    let interrupted: Vec<String> = st.queue.lock().unwrap().jobs.iter().filter(|j| j.status == models::JobStatus::Interrupted).map(|j| j.id.clone()).collect();
                    queue::retry(&st, &interrupted);
                    queue::resume(&st);
                    // MICROVID_AUTO_ADD2: add another path while the queue is running (exercises the review hold).
                    if let Ok(more) = std::env::var("MICROVID_AUTO_ADD2") {
                        tokio::time::sleep(std::time::Duration::from_secs(8)).await;
                        let st3 = st.clone();
                        let _ = st.app.run_on_main_thread(move || {
                            queue::add_sources(&st3, &[more]);
                        });
                    }
                });
            }

            // Stats ticker: one line for the status bar, once a second.
            let st = state.clone();
            tauri::async_runtime::spawn(async move {
                let mut sys = sysinfo::System::new();
                loop {
                    tokio::time::sleep(std::time::Duration::from_secs(1)).await;
                    sys.refresh_cpu_usage();
                    let cpu = sys.global_cpu_usage();
                    let stats = st.stats(cpu);
                    let _ = st.app.emit("queue:stats", stats);
                }
            });
            Ok(())
        })
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::Destroyed = event {
                // Flush job state and stop encoders before exit.
                if let Some(state) = window.try_state::<Arc<AppState>>() {
                    let ids: Vec<String> = state.queue.lock().unwrap().running.keys().cloned().collect();
                    for id in ids {
                        queue::cancel(&state, &id);
                    }
                    let jobs = state.queue.lock().unwrap().jobs.clone();
                    let _ = state.db.upsert_jobs(&jobs);
                    state.sleep.release();
                }
            }
        })
        .invoke_handler(tauri::generate_handler![
            commands::startup_info,
            commands::get_settings,
            commands::set_settings,
            commands::get_capabilities,
            commands::get_machine_info,
            commands::add_sources,
            commands::list_jobs,
            commands::update_job_settings,
            commands::estimate_settings,
            commands::preview_output_name,
            commands::release_jobs,
            commands::remove_jobs,
            commands::clear_finished,
            commands::retry_jobs,
            commands::cancel_jobs,
            commands::reorder_jobs,
            commands::start_queue,
            commands::pause_queue,
            commands::queue_stats,
            commands::refresh_subtitles,
            commands::opensubtitles_search,
            commands::opensubtitles_download,
            commands::test_encode,
            commands::benchmark,
            commands::get_analytics,
            commands::clear_history,
            commands::export_history_csv,
            commands::reveal_path,
            commands::open_path,
            commands::delete_cache,
            commands::path_exists,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
