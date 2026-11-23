//! Fast native filtering; lifecycle and strokes remain owned by Scheduler.
use std::sync::{
    atomic::{AtomicBool, AtomicU64, Ordering},
    mpsc, OnceLock,
};
static ACTIVE: AtomicU64 = AtomicU64::new(0);
static TEMP_KEY: AtomicU64 = AtomicU64::new(0);
static ROUND: AtomicU64 = AtomicU64::new(0);
static OWNED: AtomicBool = AtomicBool::new(false);
static DOWN: AtomicBool = AtomicBool::new(false);
static EVENTS: OnceLock<mpsc::Sender<(u64, i32, f64, f64)>> = OnceLock::new();
// Control bounds are published on the UI thread, read without waiting in hooks.
static CONTROLS: std::sync::Mutex<Vec<[f64; 4]>> = std::sync::Mutex::new(Vec::new());
pub fn controls(bounds: Vec<[f64; 4]>) {
    *CONTROLS.lock().unwrap() = bounds;
}
pub fn active_round(round: Option<u64>) { ACTIVE.store(round.unwrap_or(0), Ordering::SeqCst); }
pub fn current_round() -> u64 { ACTIVE.load(Ordering::SeqCst) }
pub fn temporary_key(key: u32) { TEMP_KEY.store(key as u64, Ordering::SeqCst); }
#[no_mangle]
pub extern "C" fn blink_modifier_event(key: u32, down: bool) {
    if !down && TEMP_KEY.load(Ordering::SeqCst) == key as u64 { capture(None); }
    if let Some(tx) = EVENTS.get() {
        let _ = tx.send((current_round(), if down { 5 } else { 6 }, key as f64, 0.0));
    }
}
#[no_mangle]
pub extern "C" fn blink_pointer_initial(down: bool) { DOWN.store(down, Ordering::SeqCst); }
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
    } else if kind == 4 {
        consume = round != 0;
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
            let now = std::time::Instant::now();
            let mut scheduler = shared.lock().unwrap();
            if kind == 5 || kind == 6 {
                if scheduler.snapshot(now).round_id == Some(round) {
                    scheduler.system_event(crate::scheduler::SystemEvent::Modifier { key: x as u32, down: kind == 5 }, now);
                }
            } else {
                scheduler.eraser_input(round, kind, x, y, now);
            }
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
                WM_MOUSEWHEEL | WM_MOUSEHWHEEL => Some(4),
                _ => None,
            };
            if let Some(kind) = kind {
                if blink_pointer_event(kind, if kind == 4 { ((event.mouseData >> 16) as i16) as f64 / 120.0 } else { event.pt.x as f64 }, event.pt.y as f64) {
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
                blink_pointer_initial((windows::Win32::UI::Input::KeyboardAndMouse::GetAsyncKeyState(1) as u16 & 0x8000) != 0);
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

#[cfg(test)]
mod tests {
    use super::*;
    /// The native process owns singleton hook state. Run alone so other scheduler
    /// tests cannot mutate global eligibility between physical input events.
    #[test]
    #[ignore = "native singleton state: run with --ignored --test-threads=1"]
    fn native_events_preserve_desktop_drag_and_swallowed_release_pair() {
        controls(vec![[100.0, 100.0, 150.0, 150.0]]);
        capture(None);
        blink_pointer_initial(false);
        assert!(!blink_pointer_event(1, 0.0, 0.0)); // desktop started the drag
        capture(Some(42)); // modifier activates in the middle
        assert!(!blink_pointer_event(0, 20.0, 20.0));
        assert!(!blink_pointer_event(2, 20.0, 20.0)); // desktop gets its release
        assert!(blink_pointer_event(4, 1.0, 0.0)); // wheel belongs to held eraser
        assert!(blink_pointer_event(1, 10.0, 10.0));
        capture(None); // end in the middle of an owned drag
        assert!(blink_pointer_event(2, 10.0, 10.0));
        assert!(!blink_pointer_event(4, 1.0, 0.0));
        assert!(!blink_pointer_event(1, 0.0, 0.0));
        assert!(!blink_pointer_event(2, 0.0, 0.0));
        capture(Some(43));
        assert!(!blink_pointer_event(1, 110.0, 110.0)); // holder stays clickable
        assert!(!blink_pointer_event(2, 110.0, 110.0));
        temporary_key(0x12);
        blink_modifier_event(0x12, false); // release disables capture before queue drains
        assert!(!blink_pointer_event(1, 0.0, 0.0));
        assert!(!blink_pointer_event(2, 0.0, 0.0));
        capture(None);
        controls(Vec::new());
    }
}
