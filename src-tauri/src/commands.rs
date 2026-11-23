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
pub fn get_test_interval(runtime: State<crate::config_store::RuntimeConfig>) -> Option<u64> {
    runtime.test_interval_secs
}

#[tauri::command]
pub fn close_panel(window: tauri::Window) -> Result<(), String> {
    window.hide().map_err(|e| e.to_string())
}

#[tauri::command]
pub fn save_config(
    app: tauri::AppHandle,
    new_config: DaemonConfig,
    runtime: State<crate::config_store::RuntimeConfig>,
    config: State<AppConfig>,
    shared: State<AppSharedState>,
) -> Result<(), String> {
    let auto_start = new_config.auto_start;
    let previous = config.0.lock().unwrap().clone();
    let enable_modifier = new_config.eraser_enabled
        && new_config.eraser_modifier.is_some()
        && (!previous.eraser_enabled || previous.eraser_modifier != new_config.eraser_modifier);
    let startup_changed = auto_start != previous.auto_start;
    if startup_changed {
        crate::startup::set_auto_start(auto_start).map_err(|e| e.to_string())?;
    }
    if let Err(error) = crate::config_store::save(&new_config) {
        if startup_changed {
            if let Err(rollback) = crate::startup::set_auto_start(previous.auto_start) {
                log::warn!("Could not restore startup preference after save failure: {rollback}");
            }
        }
        return Err(error);
    }
    *config.0.lock().unwrap() = new_config.clone();
    shared.0.lock().unwrap().update_config(
        runtime.scheduler_config(&new_config),
        std::time::Instant::now(),
    );
    if enable_modifier {
        app.run_on_main_thread(|| {
            crate::eraser_input::available();
        })
        .map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[tauri::command]
pub fn get_reminder(shared: State<AppSharedState>) -> crate::scheduler::ReminderSnapshot {
    shared.0.lock().unwrap().snapshot(std::time::Instant::now())
}

#[tauri::command]
pub async fn preview_reminder(shared: State<'_, AppSharedState>) -> Result<(), String> {
    let request = shared.0.lock().unwrap().request_preview();
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    loop {
        {
            let mut scheduler = shared.0.lock().unwrap();
            let now = std::time::Instant::now();
            if scheduler.snapshot(now).generation != request {
                return Ok(());
            }
            if now >= deadline {
                scheduler.cancel_presentation(request);
                return Err("提示窗口尚未就绪，请重试。".into());
            }
        }
        tokio::time::sleep(std::time::Duration::from_millis(20)).await;
    }
}

#[tauri::command]
pub fn overlay_ready(
    window: tauri::WebviewWindow,
    ready: State<crate::presentation::ReadyWindows>,
) {
    ready.0.lock().unwrap().insert(window.label().to_string());
}

/// All pause entry points share persistence and lifecycle mutation under one lock.
pub fn set_pause_shared(app: &tauri::AppHandle, seconds: u64) -> Result<(), String> {
    let shared = app.state::<AppSharedState>();
    let mut scheduler = shared.0.lock().unwrap();
    let wall = crate::pause_store::wall_now();
    let deadline = if seconds == 0 {
        None
    } else {
        Some(wall.saturating_add(seconds))
    };
    crate::pause_store::save(&crate::pause_store::path(), deadline)?;
    scheduler.restore_pause(deadline, wall, std::time::Instant::now());
    drop(scheduler);
    let handle = app.clone();
    app.run_on_main_thread(move || {
        crate::presentation::reconcile(&handle, &mut None);
        crate::tray::sync_pause(&handle);
    })
    .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn set_pause(app: tauri::AppHandle, seconds: u64) -> Result<(), String> {
    set_pause_shared(&app, seconds)
}

#[tauri::command]
pub fn get_pause(shared: State<AppSharedState>) -> Option<u64> {
    shared.0.lock().unwrap().pause_deadline()
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
pub async fn holder_click(
    app: tauri::AppHandle,
    round_id: u64,
    double: bool,
    shared: State<'_, AppSharedState>,
) -> Result<bool, String> {
    // Double click remains an exit even if Accessibility was denied on first pickup.
    if double {
        return Ok(shared.0.lock().unwrap().holder_click(
            round_id,
            true,
            std::time::Instant::now(),
        ));
    }
    let held = shared
        .0
        .lock()
        .unwrap()
        .snapshot(std::time::Instant::now())
        .eraser
        .held;
    if !held {
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
        .holder_click(round_id, false, std::time::Instant::now()))
}

#[tauri::command]
pub async fn overlay_geometry(window: tauri::WebviewWindow) -> Result<[f64; 3], String> {
    let (tx, rx) = tokio::sync::oneshot::channel();
    let handle = window.clone();
    window
        .run_on_main_thread(move || {
            let result = (|| {
                #[cfg(target_os = "macos")]
                {
                    let mut bounds = [0.0; 4];
                    unsafe {
                        crate::eraser_input::blink_window_bounds(
                            handle.ns_window().map_err(|e| e.to_string())?,
                            bounds.as_mut_ptr(),
                        );
                    }
                    Ok([bounds[0], bounds[1], 1.0])
                }
                #[cfg(not(target_os = "macos"))]
                {
                    let pos = handle.outer_position().map_err(|e| e.to_string())?;
                    let scale = handle.scale_factor().map_err(|e| e.to_string())?;
                    Ok([pos.x as f64, pos.y as f64, scale])
                }
            })();
            let _ = tx.send(result);
        })
        .map_err(|e| e.to_string())?;
    rx.await.map_err(|e| e.to_string())?
}

#[derive(serde::Serialize)]
pub struct SettingsEnvironment {
    platform: &'static str,
    reduce_transparency: bool,
    auto_start_supported: bool,
}

#[tauri::command]
pub async fn get_settings_environment(
    app: tauri::AppHandle,
) -> Result<SettingsEnvironment, String> {
    #[cfg(target_os = "macos")]
    {
        let (tx, rx) = tokio::sync::oneshot::channel();
        app.run_on_main_thread(move || {
            let _ = tx.send(SettingsEnvironment {
                platform: "macos",
                reduce_transparency: unsafe { blink_reduce_transparency() },
                auto_start_supported: false,
            });
        })
        .map_err(|e| e.to_string())?;
        rx.await.map_err(|e| e.to_string())
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = app;
        Ok(SettingsEnvironment {
            platform: std::env::consts::OS,
            reduce_transparency: false,
            auto_start_supported: cfg!(target_os = "windows"),
        })
    }
}

#[cfg(target_os = "macos")]
extern "C" {
    fn blink_reduce_transparency() -> bool;
}
