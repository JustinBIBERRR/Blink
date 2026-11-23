//! Session last-input age only: never observes keys, buttons, or their contents.
use std::time::Duration;

#[cfg(target_os = "windows")]
pub fn duration() -> Option<Duration> {
    use windows::Win32::System::SystemInformation::GetTickCount64;
    use windows::Win32::UI::Input::KeyboardAndMouse::{GetLastInputInfo, LASTINPUTINFO};
    let mut info = LASTINPUTINFO {
        cbSize: std::mem::size_of::<LASTINPUTINFO>() as u32,
        dwTime: 0,
    };
    unsafe {
        if !GetLastInputInfo(&mut info).as_bool() {
            return None;
        }
        // LASTINPUTINFO is a wrapping 32-bit uptime timestamp.
        let elapsed = (GetTickCount64() as u32).wrapping_sub(info.dwTime);
        Some(Duration::from_millis(u64::from(elapsed)))
    }
}

#[cfg(target_os = "macos")]
pub fn duration() -> Option<Duration> {
    #[link(name = "CoreGraphics", kind = "framework")]
    extern "C" {
        fn CGEventSourceSecondsSinceLastEventType(state: i32, event_type: u32) -> f64;
    }
    // Combined session state; kCGAnyInputEventType. No input-monitoring tap needed.
    let seconds = unsafe { CGEventSourceSecondsSinceLastEventType(0, u32::MAX) };
    Duration::try_from_secs_f64(seconds).ok()
}

#[cfg(not(any(target_os = "windows", target_os = "macos")))]
pub fn duration() -> Option<Duration> {
    None
}
