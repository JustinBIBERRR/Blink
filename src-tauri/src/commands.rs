use crate::daemon::SharedState;
use crate::db::{DayStats, Db, StatsToday};
use crate::state::DaemonConfig;
use std::sync::{Arc, Mutex};
use tauri::{Manager, State};

pub struct AppDb(pub Arc<Mutex<Db>>);
pub struct AppConfig(pub Arc<Mutex<DaemonConfig>>);
pub struct AppSharedState(pub SharedState);

#[tauri::command]
pub fn get_stats_today(db: State<AppDb>) -> Result<StatsToday, String> {
    db.0.lock()
        .unwrap()
        .get_stats_today()
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn get_stats_week(db: State<AppDb>) -> Result<Vec<DayStats>, String> {
    db.0.lock()
        .unwrap()
        .get_stats_week()
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn get_stats_month(db: State<AppDb>) -> Result<Vec<DayStats>, String> {
    db.0.lock()
        .unwrap()
        .get_stats_month()
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn get_config(config: State<AppConfig>) -> DaemonConfig {
    config.0.lock().unwrap().clone()
}

#[tauri::command]
pub fn close_panel(window: tauri::Window) -> Result<(), String> {
    window.hide().map_err(|e| e.to_string())
}

#[tauri::command]
pub fn save_config(
    new_config: DaemonConfig,
    config: State<AppConfig>,
    shared: State<AppSharedState>,
) -> Result<(), String> {
    let auto_start = new_config.auto_start;
    *config.0.lock().unwrap() = new_config.clone();
    shared.0.lock().unwrap().config = new_config.clone();
    // Push the new combo to the live keyboard hook and persist to disk.
    crate::hotkey::set_combo(crate::hotkey::parse_combo(&new_config.hotkey));
    crate::config_store::save(&new_config)?;
    crate::startup::set_auto_start(auto_start).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn get_reminder(shared: State<AppSharedState>) -> crate::scheduler::ReminderSnapshot {
    shared.0.lock().unwrap().snapshot(std::time::Instant::now())
}

#[tauri::command]
pub async fn preview_reminder(
    app: tauri::AppHandle,
    shared: State<'_, AppSharedState>,
) -> Result<(), String> {
    // Refresh monitor topology before creating the round; all windows remain hidden.
    let (tx, rx) = tokio::sync::oneshot::channel();
    let handle = app.clone();
    app.run_on_main_thread(move || {
        let _ = tx.send(crate::presentation::ensure_windows(&handle).map_err(|e| e.to_string()));
    })
    .map_err(|e| e.to_string())?;
    rx.await.map_err(|e| e.to_string())??;
    // Wait for every webview to install its listener before starting one shared clock.
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    loop {
        let ready = app.state::<crate::presentation::ReadyWindows>();
        let all_ready = {
            let ready = ready.0.lock().unwrap();
            app.webview_windows()
                .keys()
                .filter(|label| label.starts_with("overlay-"))
                .all(|label| ready.contains(label))
        };
        if all_ready {
            break;
        }
        if std::time::Instant::now() >= deadline {
            return Err("提示窗口尚未就绪，请重试。".into());
        }
        tokio::time::sleep(std::time::Duration::from_millis(20)).await;
    }
    shared.0.lock().unwrap().preview(std::time::Instant::now());
    Ok(())
}

#[tauri::command]
pub fn overlay_ready(
    window: tauri::WebviewWindow,
    ready: State<crate::presentation::ReadyWindows>,
) {
    ready.0.lock().unwrap().insert(window.label().to_string());
}
