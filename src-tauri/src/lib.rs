mod commands;
mod config_store;
mod daemon;
mod db;
mod hotkey;
mod input_idle;
mod presentation;
mod scheduler;
mod silence;
mod startup;
mod state;
mod system_events;
mod tray;

use commands::{AppConfig, AppDb, AppSharedState};
use daemon::SharedState;
use db::Db;
use scheduler::Scheduler;
use std::sync::{Arc, Mutex};
use tauri::Listener;

pub fn run() {
    // Default to info-level logging unless overridden via RUST_LOG.
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    let mut config = config_store::load().unwrap_or_default();

    // Dev affordance: `BLINK_DEV_INTERVAL=5` triggers a break after 5s so the
    // overlay can be exercised without waiting 20 minutes.
    if let Some(secs) = std::env::var("BLINK_DEV_INTERVAL")
        .ok()
        .and_then(|s| s.parse::<u64>().ok())
    {
        config.interval_secs = secs;
        log::info!("dev: interval_secs overridden to {secs}");
    }

    // SQLite path: %LOCALAPPDATA%/blink-rs/sessions.db
    let db_path = dirs::data_local_dir()
        .unwrap_or_else(|| std::path::PathBuf::from("."))
        .join("blink-rs")
        .join("sessions.db");
    std::fs::create_dir_all(db_path.parent().unwrap()).unwrap();

    let db = Arc::new(Mutex::new(
        Db::open(db_path.to_str().unwrap()).expect("Failed to open SQLite DB"),
    ));

    // Cleanup old records on startup
    {
        let db_guard = db.lock().unwrap();
        db_guard.cleanup_old_records().ok();
    }

    let shared_config = Arc::new(Mutex::new(config));
    let shared_state: SharedState = Arc::new(Mutex::new(Scheduler::new(
        shared_config.lock().unwrap().clone(),
        std::time::Instant::now(),
    )));

    let db_for_daemon = db.clone();
    let db_for_hotkey = db.clone();
    let state_clone = shared_state.clone();
    let cfg_for_hotkey = shared_config.clone();

    tauri::Builder::default()
        .plugin(tauri_plugin_notification::init())
        .manage(presentation::ReadyWindows::default())
        .manage(AppDb(db.clone()))
        .manage(AppConfig(shared_config.clone()))
        .manage(AppSharedState(shared_state.clone()))
        .setup(move |app| {
            // System tray
            tray::setup_tray(&app.handle())?;

            presentation::ensure_windows(app.handle())?;
            let app_handle = app.handle().clone();
            let previous = Arc::new(Mutex::new(None));
            let event_state = state_clone.clone();
            let event_app = app.handle().clone();
            let event_previous = previous.clone();
            system_events::install(move |event| {
                event_state
                    .lock()
                    .unwrap()
                    .system_event(event, std::time::Instant::now());
                let handle = event_app.clone();
                let previous = event_previous.clone();
                let _ = event_app.run_on_main_thread(move || {
                    presentation::reconcile(&handle, &mut previous.lock().unwrap());
                });
            });
            tauri::async_runtime::spawn(daemon::run(state_clone.clone(), move |outcome| {
                if let Some(outcome) = outcome {
                    let _ = db_for_daemon.lock().unwrap().record_session(&outcome);
                }
                let handle = app_handle.clone();
                let previous = previous.clone();
                let _ = app_handle.run_on_main_thread(move || {
                    presentation::reconcile(&handle, &mut previous.lock().unwrap());
                });
            }));

            let (esc_tx, esc_rx) = std::sync::mpsc::channel::<()>();
            hotkey::install(esc_tx);
            hotkey::set_combo(hotkey::parse_combo(&cfg_for_hotkey.lock().unwrap().hotkey));
            let state_for_hotkey = state_clone.clone();
            std::thread::spawn(move || {
                while esc_rx.recv().is_ok() {
                    let now = std::time::Instant::now();
                    let mut state = state_for_hotkey.lock().unwrap();
                    if let Some(id) = state.snapshot(now).round_id {
                        if state.finish(id, now) {
                            let _ = db_for_hotkey
                                .lock()
                                .unwrap()
                                .record_session(&state::SessionOutcome::WarnSkipped);
                        }
                    }
                }
            });

            let state_for_pause = state_clone.clone();
            app.listen("blink://pause", move |event: tauri::Event| {
                if let Ok(secs) = serde_json::from_str::<u64>(event.payload()) {
                    state_for_pause
                        .lock()
                        .unwrap()
                        .pause(secs, std::time::Instant::now());
                }
            });

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::get_stats_today,
            commands::get_stats_week,
            commands::get_stats_month,
            commands::get_config,
            commands::save_config,
            commands::close_panel,
            commands::preview_reminder,
            commands::get_reminder,
            commands::overlay_ready,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
