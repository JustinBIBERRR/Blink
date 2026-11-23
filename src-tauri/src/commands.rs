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
    shared
        .0
        .lock()
        .unwrap()
        .update_config(new_config.clone(), std::time::Instant::now());
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
    let generation = shared
        .0
        .lock()
        .unwrap()
        .snapshot(std::time::Instant::now())
        .generation;
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
                .filter(|label| label.starts_with("overlay-") || label.starts_with("controls-"))
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
    shared
        .0
        .lock()
        .unwrap()
        .preview_if_current(generation, std::time::Instant::now());
    Ok(())
}

#[tauri::command]
pub fn overlay_ready(
    window: tauri::WebviewWindow,
    ready: State<crate::presentation::ReadyWindows>,
) {
    ready.0.lock().unwrap().insert(window.label().to_string());
}

#[tauri::command]
pub fn finish_reminder(round_id: u64, shared: State<AppSharedState>) {
    shared
        .0
        .lock()
        .unwrap()
        .finish(round_id, std::time::Instant::now());
}

#[tauri::command]
pub fn fog_capture(window: tauri::WebviewWindow) -> Result<Option<String>, String> {
    #[cfg(target_os = "windows")]
    {
        let monitor = window
            .current_monitor()
            .map_err(|e| e.to_string())?
            .ok_or("monitor unavailable")?;
        let position = monitor.position();
        let size = monitor.size();
        return crate::capture::capture_monitor_data_url(
            position.x,
            position.y,
            size.width as i32,
            size.height as i32,
            2,
        )
        .map(Some);
    }
    #[cfg(not(target_os = "windows"))]
    {
        let _ = window;
        Ok(None)
    }
}

#[tauri::command]
pub fn fog_mask(
    window: tauri::WebviewWindow,
    round_id: u64,
    alpha: Vec<u8>,
    width: i32,
    height: i32,
    shared: State<AppSharedState>,
) -> Result<(), String> {
    if width < 1
        || height < 1
        || width > 320
        || height > 180
        || alpha.len() != (width * height) as usize
    {
        return Err("invalid mask".into());
    }
    if shared
        .0
        .lock()
        .unwrap()
        .snapshot(std::time::Instant::now())
        .round_id
        != Some(round_id)
    {
        return Ok(());
    }
    #[cfg(target_os = "macos")]
    {
        let handle = window.clone();
        window
            .run_on_main_thread(move || {
                // Recheck at execution time: delayed masks must never revive an old round.
                let state = handle.state::<AppSharedState>();
                if state
                    .0
                    .lock()
                    .unwrap()
                    .snapshot(std::time::Instant::now())
                    .round_id
                    != Some(round_id)
                {
                    return;
                }
                if let Ok(raw) = handle.ns_window() {
                    unsafe {
                        blink_fog_mask(raw, alpha.as_ptr(), width, height);
                    }
                }
            })
            .map_err(|e| e.to_string())?;
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = window;
    }
    Ok(())
}
#[cfg(target_os = "macos")]
extern "C" {
    pub fn blink_fog_mask(window: *mut std::ffi::c_void, alpha: *const u8, width: i32, height: i32);
}

#[tauri::command]
pub async fn set_eraser_held(
    app: tauri::AppHandle,
    round_id: u64,
    held: bool,
    shared: State<'_, AppSharedState>,
) -> Result<bool, String> {
    if held {
        let (tx, rx) = tokio::sync::oneshot::channel();
        app.run_on_main_thread(move || {
            let _ = tx.send(crate::eraser_input::available());
        })
        .map_err(|e| e.to_string())?;
        if !rx.await.unwrap_or(false) {
            return Err("请在系统设置中允许 Blink 使用辅助功能后重试。关闭按钮仍然可用。".into());
        }
    }
    Ok(shared
        .0
        .lock()
        .unwrap()
        .set_eraser_held(round_id, held, std::time::Instant::now()))
}

#[tauri::command]
pub fn overlay_geometry(window: tauri::WebviewWindow) -> Result<[f64; 3], String> {
    let pos = window.outer_position().map_err(|e| e.to_string())?;
    let scale = window.scale_factor().map_err(|e| e.to_string())?;
    #[cfg(target_os = "macos")]
    {
        let _ = (pos, scale);
        let mut bounds = [0.0; 4];
        unsafe {
            crate::eraser_input::blink_window_bounds(
                window.ns_window().map_err(|e| e.to_string())?,
                bounds.as_mut_ptr(),
            );
        }
        Ok([bounds[0], bounds[1], 1.0])
    }
    #[cfg(not(target_os = "macos"))]
    {
        Ok([pos.x as f64, pos.y as f64, scale])
    }
}
