use crate::daemon::SharedState;
use crate::db::{Db, DayStats, StatsToday};
use crate::state::DaemonConfig;
use std::sync::{Arc, Mutex};
use tauri::State;

pub struct AppDb(pub Arc<Mutex<Db>>);
pub struct AppConfig(pub Arc<Mutex<DaemonConfig>>);
pub struct AppSharedState(pub SharedState);

#[tauri::command]
pub fn get_stats_today(db: State<AppDb>) -> Result<StatsToday, String> {
    db.0.lock().unwrap().get_stats_today().map_err(|e| e.to_string())
}

#[tauri::command]
pub fn get_stats_week(db: State<AppDb>) -> Result<Vec<DayStats>, String> {
    db.0.lock().unwrap().get_stats_week().map_err(|e| e.to_string())
}

#[tauri::command]
pub fn get_stats_month(db: State<AppDb>) -> Result<Vec<DayStats>, String> {
    db.0.lock().unwrap().get_stats_month().map_err(|e| e.to_string())
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
    shared.0.lock().unwrap().config = new_config;
    crate::startup::set_auto_start(auto_start).map_err(|e| e.to_string())
}
