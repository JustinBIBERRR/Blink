//! Native windows are projections of the scheduler, reconciled on the UI thread.
use crate::{commands::AppSharedState, scheduler::ReminderSnapshot};
use tauri::{Emitter, Manager, WebviewUrl, WebviewWindowBuilder};

#[derive(Default)]
pub struct ReadyWindows(pub std::sync::Mutex<std::collections::HashSet<String>>);

pub fn ensure_windows(app: &tauri::AppHandle) -> tauri::Result<()> {
    let monitors = app.available_monitors()?;
    let mut bounds = Vec::new();
    let eraser_enabled = {
        let state = app.state::<AppSharedState>();
        let state = state.0.lock().unwrap();
        let snapshot = state.snapshot(std::time::Instant::now());
        if snapshot.round_id.is_some() {
            snapshot.eraser.enabled
        } else {
            state.config.eraser_enabled
        }
    };
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
            .shadow(false)
            .always_on_top(true)
            .skip_taskbar(true)
            .resizable(false)
            .build()?,
        };
        #[cfg(target_os = "windows")]
        if let Ok(hwnd) = window.hwnd() {
            let _ = crate::capture::exclude_from_capture(hwnd.0);
        }
        window.set_ignore_cursor_events(true)?;
        window.set_size(*monitor.size())?;
        window.set_position(*monitor.position())?;
        let control_label = format!("controls-{index}");
        let control = match app.get_webview_window(&control_label) {
            Some(window) => window,
            None => WebviewWindowBuilder::new(
                app,
                &control_label,
                WebviewUrl::App("index.html#/controls".into()),
            )
            .visible(false)
            .focused(false)
            .focusable(false)
            .decorations(false)
            .transparent(true)
            .shadow(false)
            .always_on_top(true)
            .skip_taskbar(true)
            .resizable(false)
            .inner_size(48.0, 48.0)
            .build()?,
        };
        let size = (48.0 * monitor.scale_factor()) as u32;
        let width = ((if eraser_enabled { 208.0 } else { 48.0 }) * monitor.scale_factor()) as u32;
        let margin = (20.0 * monitor.scale_factor()) as i32;
        control.set_size(tauri::PhysicalSize::new(width, size))?;
        control.set_position(tauri::PhysicalPosition::new(
            monitor.position().x + monitor.size().width as i32 - width as i32 - margin,
            monitor.position().y + monitor.size().height as i32 - size as i32 - margin,
        ))?;
        #[cfg(target_os = "macos")]
        {
            let mut rect = [0.0; 4];
            unsafe {
                crate::eraser_input::blink_window_bounds(control.ns_window()?, rect.as_mut_ptr());
            }
            bounds.push(rect);
        }
        #[cfg(not(target_os = "macos"))]
        {
            let p = control.outer_position()?;
            bounds.push([
                p.x as f64,
                p.y as f64,
                p.x as f64 + width as f64,
                p.y as f64 + size as f64,
            ]);
        }
    }
    crate::eraser_input::controls(bounds);
    for (label, window) in app.webview_windows() {
        if let Some(index) = label
            .strip_prefix("overlay-")
            .or_else(|| label.strip_prefix("controls-"))
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
    let request = state
        .0
        .lock()
        .unwrap()
        .pending_presentation(std::time::Instant::now());
    if let Some(request) = request {
        match ensure_windows(app) {
            Ok(()) => {
                let windows = app.webview_windows();
                let ready = app.state::<ReadyWindows>();
                let ready = ready.0.lock().unwrap();
                let mut participating = windows.keys().filter(|label| {
                    label.starts_with("overlay-") || label.starts_with("controls-")
                });
                let all_ready = participating.clone().next().is_some()
                    && participating.all(|label| ready.contains(label));
                if all_ready {
                    state
                        .0
                        .lock()
                        .unwrap()
                        .present_if_current(request, std::time::Instant::now());
                }
            }
            Err(error) => log::warn!("monitor preparation failed: {error}"),
        }
    }
    let (snapshot, notifications): (ReminderSnapshot, _) = {
        let mut scheduler = state.0.lock().unwrap();
        (
            scheduler.snapshot(std::time::Instant::now()),
            scheduler.take_notifications(),
        )
    };
    for message in notifications {
        crate::notifications::submit(app, message);
    }
    let key = (snapshot.round_id, snapshot.phase.clone());
    let _ = app.emit("blink://reminder", &snapshot);
    if previous.as_ref() != Some(&key) {
        #[cfg(target_os = "macos")]
        if previous.as_ref().map(|p| p.0) != Some(snapshot.round_id) {
            for (label, window) in app.webview_windows() {
                if label.starts_with("overlay-") {
                    if let Ok(raw) = window.ns_window() {
                        unsafe {
                            crate::commands::blink_fog_mask(raw, std::ptr::null(), 0, 0);
                        }
                    }
                }
            }
        }
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
        let controls = label.starts_with("controls-");
        if !label.starts_with("overlay-") && !controls {
            continue;
        }
        let visible = snapshot.round_id.is_some()
            && (!controls
                || (snapshot.mode == crate::state::ReminderMode::Fog
                    && snapshot.elapsed_ms >= 3000));
        if visible {
            let _ = window.set_ignore_cursor_events(!controls);
            if !window.is_visible().unwrap_or(false) {
                let _ = window.show();
            }
        } else {
            #[cfg(target_os = "macos")]
            if !controls {
                if let Ok(raw) = window.ns_window() {
                    unsafe {
                        crate::commands::blink_fog_mask(raw, std::ptr::null(), 0, 0);
                    }
                }
            }
            let _ = window.set_ignore_cursor_events(true);
            if window.is_visible().unwrap_or(false) {
                let _ = window.hide();
            }
        }
    }
}
