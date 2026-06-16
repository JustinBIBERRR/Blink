mod commands;
mod daemon;
mod db;
mod hotkey;
mod silence;
mod startup;
mod state;
mod tray;

use commands::{AppConfig, AppDb};
use daemon::SharedState;
use db::Db;
use state::{DaemonConfig, DaemonState, Phase};
use std::sync::{Arc, Mutex};
use tauri::{Emitter, Listener, Manager};

pub fn run() {
    let config = DaemonConfig::default();

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
    let shared_state: SharedState = Arc::new(Mutex::new(DaemonState::new(
        shared_config.lock().unwrap().clone(),
    )));

    let db_clone = db.clone();
    let state_clone = shared_state.clone();

    tauri::Builder::default()
        .plugin(tauri_plugin_notification::init())
        .manage(AppDb(db.clone()))
        .manage(AppConfig(shared_config.clone()))
        .setup(move |app| {
            // System tray
            tray::setup_tray(&app.handle())?;

            // First-run Toast: no sessions ever recorded
            {
                let db_guard = db_clone.lock().unwrap();
                let is_first_run: bool = db_guard
                    .conn
                    .query_row("SELECT COUNT(*) FROM sessions", [], |r| r.get::<_, i64>(0))
                    .unwrap_or(1)
                    == 0;
                drop(db_guard);
                if is_first_run {
                    use tauri_plugin_notification::NotificationExt;
                    let _ = app
                        .handle()
                        .notification()
                        .builder()
                        .title("👁 Blink is running")
                        .body("First eye break in 20 minutes. Right-click tray icon to configure.")
                        .show();
                }
            }

            // Daemon loop
            let app_handle = app.handle().clone();
            let db_for_daemon = db_clone.clone();
            tauri::async_runtime::spawn(daemon::run(
                state_clone.clone(),
                || silence::SilenceChecker::new().is_silenced(),
                move |phase, outcome| {
                    // Record completed sessions
                    if let Some(outcome) = outcome {
                        if let Ok(db) = db_for_daemon.lock() {
                            db.record_session(&outcome).ok();
                        }
                    }
                    // Emit phase change to all windows
                    let _ = app_handle.emit("blink://phase", &phase);
                    // Show/hide overlay window
                    if let Some(overlay) = app_handle.get_webview_window("overlay") {
                        match phase {
                            Phase::Monitoring => {
                                let _ = overlay.hide();
                            }
                            _ => {
                                let _ = overlay.show();
                            }
                        }
                    }
                },
            ));

            // Hotkey listener thread
            // HotkeyManager MUST stay alive inside the thread to keep the hotkey registered.
            let hotkey_mgr = hotkey::HotkeyManager::new().expect("Hotkey registration failed");
            let hotkey_id = hotkey_mgr.hotkey_id;
            let app_handle2 = app.handle().clone();
            let state_for_hotkey = state_clone.clone();
            let db_for_hotkey = db_clone.clone();
            std::thread::spawn(move || {
                // Move hotkey_mgr into the thread so it stays alive as long as this thread runs.
                let _hotkey_mgr = hotkey_mgr;
                let receiver = global_hotkey::GlobalHotKeyEvent::receiver();
                loop {
                    if let Ok(event) = receiver.try_recv() {
                        if hotkey::is_escape_event(&event, hotkey_id) {
                            let outcome = {
                                let mut s = state_for_hotkey.lock().unwrap();
                                if matches!(s.phase, Phase::Monitoring | Phase::Dissolving) {
                                    // Nothing to escape in these phases
                                    continue;
                                }
                                let outcome = daemon::escape_outcome(&s.phase);
                                s.transition_to(Phase::Monitoring);
                                outcome
                            };
                            if let Ok(db) = db_for_hotkey.lock() {
                                db.record_session(&outcome).ok();
                            }
                            let _ = app_handle2.emit("blink://phase", Phase::Monitoring);
                            if let Some(overlay) = app_handle2.get_webview_window("overlay") {
                                let _ = overlay.hide();
                            }
                        }
                    }
                    std::thread::sleep(std::time::Duration::from_millis(50));
                }
            });

            // Pause event listener (emitted by tray.rs as "blink://pause")
            let state_for_pause = state_clone.clone();
            app.listen("blink://pause", move |event: tauri::Event| {
                if let Ok(secs) = serde_json::from_str::<u64>(event.payload()) {
                    let mut s = state_for_pause.lock().unwrap();
                    s.paused_until = Some(
                        std::time::Instant::now() + std::time::Duration::from_secs(secs),
                    );
                    s.transition_to(Phase::Monitoring);
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
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
