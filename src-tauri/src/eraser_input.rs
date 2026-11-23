//! Fast native filtering; lifecycle and strokes remain owned by Scheduler.
use std::sync::{
    atomic::{AtomicBool, AtomicU64, Ordering},
    mpsc, OnceLock,
};
static ROUND: AtomicU64 = AtomicU64::new(0);
static OWNED: AtomicBool = AtomicBool::new(false);
static DOWN: AtomicBool = AtomicBool::new(false);
static EVENTS: OnceLock<mpsc::Sender<(u64, i32, f64, f64)>> = OnceLock::new();
// Control bounds are published on the UI thread, read without waiting in hooks.
static CONTROLS: std::sync::Mutex<Vec<[f64; 4]>> = std::sync::Mutex::new(Vec::new());
pub fn controls(bounds: Vec<[f64; 4]>) {
    *CONTROLS.lock().unwrap() = bounds;
}
pub fn capture(round: Option<u64>) {
    ROUND.store(round.unwrap_or(0), Ordering::SeqCst);
}
#[no_mangle]
pub extern "C" fn blink_pointer_event(kind: i32, x: f64, y: f64) -> bool {
    let round = ROUND.load(Ordering::SeqCst);
    if kind == 3 {
        capture(None);
    }
    let mut consume = false;
    if kind == 1 {
        let was_down = DOWN.swap(true, Ordering::SeqCst);
        // Fail open if control geometry is being refreshed.
        let on_control = CONTROLS
            .try_lock()
            .map(|r| {
                r.iter()
                    .any(|r| x >= r[0] && y >= r[1] && x < r[2] && y < r[3])
            })
            .unwrap_or(true);
        consume = !was_down && round != 0 && !on_control;
        OWNED.store(consume, Ordering::SeqCst);
    } else if kind == 2 {
        DOWN.store(false, Ordering::SeqCst);
        // Match swallowed presses even if the reminder ended during the drag.
        consume = OWNED.swap(false, Ordering::SeqCst);
    }
    if round != 0 && (kind == 0 || consume || kind == 3) {
        if let Some(tx) = EVENTS.get() {
            let _ = tx.send((round, kind, x, y));
        }
    }
    consume
}
pub fn install(shared: crate::daemon::SharedState) {
    let (tx, rx) = mpsc::channel();
    let _ = EVENTS.set(tx);
    std::thread::spawn(move || {
        while let Ok((round, kind, x, y)) = rx.recv() {
            shared
                .lock()
                .unwrap()
                .eraser_input(round, kind, x, y, std::time::Instant::now());
        }
    });
}
#[cfg(target_os = "macos")]
pub fn available() -> bool {
    unsafe { blink_start_pointer() }
}
#[cfg(target_os = "macos")]
extern "C" {
    fn blink_start_pointer() -> bool;
    pub fn blink_window_bounds(window: *mut std::ffi::c_void, bounds: *mut f64);
}
#[cfg(target_os = "windows")]
pub fn available() -> bool {
    windows_hook::start()
}
#[cfg(not(any(target_os = "macos", target_os = "windows")))]
pub fn available() -> bool {
    false
}

#[cfg(target_os = "windows")]
mod windows_hook {
    use super::*;
    use windows::Win32::{
        Foundation::{LPARAM, LRESULT, WPARAM},
        UI::WindowsAndMessaging::*,
    };
    static READY: OnceLock<bool> = OnceLock::new();
    unsafe extern "system" fn hook(code: i32, w: WPARAM, l: LPARAM) -> LRESULT {
        if code >= 0 {
            let event = &*(l.0 as *const MSLLHOOKSTRUCT);
            let kind = match w.0 as u32 {
                WM_MOUSEMOVE => Some(0),
                WM_LBUTTONDOWN => Some(1),
                WM_LBUTTONUP => Some(2),
                _ => None,
            };
            if let Some(kind) = kind {
                if blink_pointer_event(kind, event.pt.x as f64, event.pt.y as f64) {
                    return LRESULT(1);
                }
            }
        }
        CallNextHookEx(None, code, w, l)
    }
    pub fn start() -> bool {
        *READY.get_or_init(|| {
            let (tx, rx) = mpsc::channel();
            std::thread::spawn(move || unsafe {
                let hook = SetWindowsHookExW(WH_MOUSE_LL, Some(hook), None, 0);
                let _ = tx.send(hook.is_ok());
                if let Ok(handle) = hook {
                    let mut msg = MSG::default();
                    while GetMessageW(&mut msg, None, 0, 0).as_bool() {
                        let _ = TranslateMessage(&msg);
                        DispatchMessageW(&msg);
                    }
                    let _ = UnhookWindowsHookEx(handle);
                }
            });
            rx.recv().unwrap_or(false)
        })
    }
}
