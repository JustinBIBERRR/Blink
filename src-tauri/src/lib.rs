mod capture;
mod commands;
mod config_store;
mod daemon;
mod db;
mod eraser_input;
mod input_idle;
mod modifier_input;
mod notifications;
mod pause_store;
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
use tauri::Manager;

pub fn run() {
    // Default to info-level logging unless overridden via RUST_LOG.
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    let loaded_config = config_store::load();
    let show_settings = loaded_config.is_none() || std::env::args().any(|arg| arg == "--settings");
    let config = loaded_config.unwrap_or_default();

    let args: Vec<_> = std::env::args().collect();
    let test_interval_secs = args
        .windows(2)
        .find(|pair| pair[0] == "--test-interval-secs")
        .map(|pair| pair[1].clone())
        .or_else(|| std::env::var("BLINK_DEV_INTERVAL").ok())
        .and_then(|value| value.parse::<u64>().ok())
        .filter(|seconds| *seconds > 0);
    let runtime = config_store::RuntimeConfig { test_interval_secs };
    if let Some(seconds) = test_interval_secs {
        log::info!("local acceptance interval: {seconds}s (not persisted)");
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
        runtime.scheduler_config(&shared_config.lock().unwrap()),
        std::time::Instant::now(),
    )));

    shared_state
        .lock()
        .unwrap()
        .set_acceptance_interval(test_interval_secs);
    shared_state.lock().unwrap().restore_pause(
        pause_store::load(&pause_store::path()),
        pause_store::wall_now(),
        std::time::Instant::now(),
    );

    let db_for_daemon = db.clone();
    let state_clone = shared_state.clone();
    let cfg_for_modifier = shared_config.clone();

    tauri::Builder::default()
        .plugin(tauri_plugin_notification::init())
        .manage(runtime)
        .manage(presentation::ReadyWindows::default())
        .manage(AppDb(db.clone()))
        .manage(AppConfig(shared_config.clone()))
        .manage(AppSharedState(shared_state.clone()))
        .setup(move |app| {
            eraser_input::install(state_clone.clone());
            // System tray
            tray::setup_tray(&app.handle())?;

            presentation::ensure_windows(app.handle())?;
            if show_settings {
                if let Some(window) = app.get_webview_window("settings") {
                    window.show()?;
                    window.set_focus()?;
                }
            }
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
                    tray::sync_pause(&handle);
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
                    tray::sync_pause(&handle);
                });
            }));

            modifier_input::install();
            {
                let cfg = cfg_for_modifier.lock().unwrap();
                if cfg.eraser_enabled && cfg.eraser_modifier.is_some() {
                    eraser_input::available();
                }
            }

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::get_stats_today,
            commands::get_stats_week,
            commands::get_stats_month,
            commands::get_config,
            commands::get_settings_environment,
            commands::get_test_interval,
            commands::save_config,
            commands::close_panel,
            commands::preview_reminder,
            commands::get_reminder,
            commands::get_pause,
            commands::set_pause,
            commands::overlay_ready,
            commands::finish_reminder,
            commands::fog_mask,
            commands::fog_capture,
            commands::set_eraser_held,
            commands::holder_click,
            commands::overlay_geometry,
        ])
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(|app, event| {
            #[cfg(target_os = "macos")]
            if let tauri::RunEvent::Reopen {
                has_visible_windows: false,
                ..
            } = event
            {
                if let Some(window) = app.get_webview_window("settings") {
                    let _ = window.show();
                    let _ = window.set_focus();
                }
            }
            #[cfg(not(target_os = "macos"))]
            let _ = (app, event);
        });
}
