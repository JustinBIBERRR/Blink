use tauri::{AppHandle, Emitter, Manager};
use tauri::menu::{Menu, MenuItem, PredefinedMenuItem, Submenu};
use tauri::tray::TrayIconBuilder;

pub fn setup_tray(app: &AppHandle) -> tauri::Result<()> {
    let report = MenuItem::with_id(app, "report", "📊  View Report", true, None::<&str>)?;
    let settings = MenuItem::with_id(app, "settings", "⚙   Settings", true, None::<&str>)?;

    let pause_1h = MenuItem::with_id(app, "pause_1h", "1 Hour", true, None::<&str>)?;
    let pause_3h = MenuItem::with_id(app, "pause_3h", "3 Hours", true, None::<&str>)?;
    let pause_today = MenuItem::with_id(app, "pause_today", "☾  Today Off", true, None::<&str>)?;
    let pause_sub = Submenu::with_id_and_items(
        app,
        "pause",
        "⏸  Pause",
        true,
        &[&pause_1h, &pause_3h, &pause_today],
    )?;

    let sep1 = PredefinedMenuItem::separator(app)?;
    let sep2 = PredefinedMenuItem::separator(app)?;
    let quit = MenuItem::with_id(app, "quit", "✕  Quit Blink", true, None::<&str>)?;

    let menu = Menu::with_items(app, &[
        &report,
        &settings,
        &sep1,
        &pause_sub,
        &sep2,
        &quit,
    ])?;

    TrayIconBuilder::new()
        .icon(app.default_window_icon().unwrap().clone())
        .menu(&menu)
        .tooltip("Blink — Next break in 20:00")
        .on_menu_event(|app, event| match event.id().as_ref() {
            "report" => {
                if let Some(w) = app.get_webview_window("stats") {
                    let _ = w.show();
                    let _ = w.set_focus();
                }
            }
            "settings" => {
                if let Some(w) = app.get_webview_window("settings") {
                    let _ = w.show();
                    let _ = w.set_focus();
                }
            }
            "pause_1h" => {
                let _ = app.emit("blink://pause", 3600u64);
            }
            "pause_3h" => {
                let _ = app.emit("blink://pause", 10800u64);
            }
            "pause_today" => {
                let secs = secs_until_midnight();
                let _ = app.emit("blink://pause", secs);
            }
            "quit" => {
                app.exit(0);
            }
            _ => {}
        })
        .build(app)?;

    Ok(())
}

fn secs_until_midnight() -> u64 {
    use chrono::Local;
    let now = Local::now();
    let tomorrow = now.date_naive().succ_opt().unwrap_or(now.date_naive());
    let midnight = chrono::NaiveDateTime::new(
        tomorrow,
        chrono::NaiveTime::from_hms_opt(0, 0, 0).unwrap(),
    );
    let midnight_local = chrono::TimeZone::from_local_datetime(&Local, &midnight)
        .unwrap();
    let secs = (midnight_local - now).num_seconds().max(0) as u64;
    secs
}

#[cfg(test)]
mod tests {
    #[test]
    fn tray_module_compiles() {
        // Tray requires OS resources — verified by cargo build
    }
}
