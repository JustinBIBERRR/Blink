//! Native windows are projections of the scheduler, reconciled on the UI thread.
use crate::{commands::AppSharedState, scheduler::ReminderSnapshot};
use tauri::{Emitter, Manager, WebviewUrl, WebviewWindowBuilder};

#[derive(Default)]
pub struct ReadyWindows(pub std::sync::Mutex<std::collections::HashSet<String>>);

pub fn ensure_windows(app: &tauri::AppHandle) -> tauri::Result<()> {
    let monitors = app.available_monitors()?;
    for (index, monitor) in monitors.iter().enumerate() {
        let label = format!("overlay-{index}");
        let window = match app.get_webview_window(&label) {
            Some(window) => window,
            None => WebviewWindowBuilder::new(
                app,
                &label,
                WebviewUrl::App("index.html#/overlay".into()),
            )
            .visible(false)
            .focused(false)
            .focusable(false)
            .decorations(false)
            .transparent(true)
            .always_on_top(true)
            .skip_taskbar(true)
            .resizable(false)
            .build()?,
        };
        window.set_ignore_cursor_events(true)?;
        window.set_size(*monitor.size())?;
        window.set_position(*monitor.position())?;
    }
    for (label, window) in app.webview_windows() {
        if let Some(index) = label
            .strip_prefix("overlay-")
            .and_then(|s| s.parse::<usize>().ok())
        {
            if index >= monitors.len() {
                app.state::<ReadyWindows>().0.lock().unwrap().remove(&label);
                window.close()?;
            }
        }
    }
    Ok(())
}

/// Call only on the main thread. Read the authoritative state at execution time,
/// so an old queued callback cannot resurrect a completed/replaced reminder.
pub fn reconcile(
    app: &tauri::AppHandle,
    previous: &mut Option<(Option<u64>, crate::state::Phase)>,
) {
    let state = app.state::<AppSharedState>();
    let snapshot: ReminderSnapshot = state.0.lock().unwrap().snapshot(std::time::Instant::now());
    let key = (snapshot.round_id, snapshot.phase.clone());
    if previous.as_ref() != Some(&key) {
        if snapshot.round_id.is_some() {
            if let Err(error) = ensure_windows(app) {
                log::warn!("monitor refresh failed: {error}");
            }
        }
        let _ = app.emit("blink://reminder", &snapshot);
        let _ = app.emit("blink://phase", &snapshot.phase);
        *previous = Some(key);
    }
    for (label, window) in app.webview_windows() {
        if !label.starts_with("overlay-") {
            continue;
        }
        if snapshot.round_id.is_some() {
            let _ = window.set_ignore_cursor_events(true);
            if !window.is_visible().unwrap_or(false) {
                let _ = window.show();
            }
        } else {
            let _ = window.set_ignore_cursor_events(true);
            if window.is_visible().unwrap_or(false) {
                let _ = window.hide();
            }
        }
    }
}
