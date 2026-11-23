use tauri::menu::{Menu, MenuItem, PredefinedMenuItem, Submenu};
use tauri::tray::TrayIconBuilder;
use tauri::{AppHandle, Manager};

struct PauseMenu {
    submenu: Submenu<tauri::Wry>,
    resume: MenuItem<tauri::Wry>,
    previous: std::sync::Mutex<Option<bool>>,
}

pub fn sync_pause(app: &AppHandle) {
    let paused = app
        .state::<crate::commands::AppSharedState>()
        .0
        .lock()
        .unwrap()
        .pause_deadline()
        .is_some();
    let menu = app.state::<PauseMenu>();
    let mut previous = menu.previous.lock().unwrap();
    if *previous == Some(paused) {
        return;
    }
    let _ = menu
        .submenu
        .set_text(if paused { "⏸  Paused" } else { "⏸  Pause" });
    let _ = menu.resume.set_enabled(paused);
    *previous = Some(paused);
}

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

    let resume = MenuItem::with_id(app, "resume", "▶  Resume", false, None::<&str>)?;
    app.manage(PauseMenu {
        submenu: pause_sub.clone(),
        resume: resume.clone(),
        previous: std::sync::Mutex::new(None),
    });
    sync_pause(app);
    let sep1 = PredefinedMenuItem::separator(app)?;
    let sep2 = PredefinedMenuItem::separator(app)?;
    let quit = MenuItem::with_id(app, "quit", "✕  Quit Blink", true, None::<&str>)?;

    let menu = Menu::with_items(
        app,
        &[&report, &settings, &sep1, &pause_sub, &resume, &sep2, &quit],
    )?;

    TrayIconBuilder::new()
        .icon(app.default_window_icon().unwrap().clone())
        .menu(&menu)
        .tooltip("Blink")
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
                apply_pause(app, 3600);
            }
            "pause_3h" => {
                apply_pause(app, 10800);
            }
            "pause_today" => {
                let secs = secs_until_midnight();
                apply_pause(app, secs);
            }
            "resume" => apply_pause(app, 0),
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
    let midnight =
        chrono::NaiveDateTime::new(tomorrow, chrono::NaiveTime::from_hms_opt(0, 0, 0).unwrap());
    let midnight_local = chrono::TimeZone::from_local_datetime(&Local, &midnight).unwrap();
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

fn apply_pause(app: &AppHandle, seconds: u64) {
    if let Err(error) = crate::commands::set_pause_shared(app, seconds) {
        log::error!("Unable to save pause: {error}");
    }
}
