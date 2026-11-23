//! Native lock and power events; idle input is deliberately not a suspension signal.
use crate::scheduler::SystemEvent;
use std::sync::OnceLock;
static CALLBACK: OnceLock<Box<dyn Fn(SystemEvent) + Send + Sync>> = OnceLock::new();

pub fn install(callback: impl Fn(SystemEvent) + Send + Sync + 'static) {
    if CALLBACK.set(Box::new(callback)).is_err() {
        return;
    }
    #[cfg(target_os = "macos")]
    unsafe {
        blink_install_system_events(receive);
    }
    #[cfg(target_os = "windows")]
    std::thread::spawn(windows_adapter::run);
}

extern "C" fn receive(kind: i32, value: bool) {
    if let Some(callback) = CALLBACK.get() {
        callback(if kind == 0 {
            SystemEvent::Locked(value)
        } else {
            SystemEvent::Sleeping(value)
        });
    }
}

#[cfg(target_os = "macos")]
extern "C" {
    fn blink_install_system_events(callback: extern "C" fn(i32, bool));
}

#[cfg(target_os = "windows")]
mod windows_adapter {
    use windows::{
        core::w,
        Win32::{
            Foundation::*,
            System::{LibraryLoader::GetModuleHandleW, RemoteDesktop::*},
            UI::WindowsAndMessaging::*,
        },
    };
    unsafe extern "system" fn window_proc(hwnd: HWND, msg: u32, wp: WPARAM, lp: LPARAM) -> LRESULT {
        match msg {
            WM_WTSSESSION_CHANGE => match wp.0 as u32 {
                WTS_SESSION_LOCK | WTS_CONSOLE_DISCONNECT | WTS_REMOTE_DISCONNECT => {
                    super::receive(0, true)
                }
                WTS_SESSION_UNLOCK | WTS_CONSOLE_CONNECT | WTS_REMOTE_CONNECT => {
                    super::receive(0, false)
                }
                _ => {}
            },
            WM_POWERBROADCAST => match wp.0 as u32 {
                PBT_APMSUSPEND => super::receive(1, true),
                PBT_APMRESUMEAUTOMATIC => super::receive(1, false),
                _ => {}
            },
            _ => {}
        }
        DefWindowProcW(hwnd, msg, wp, lp)
    }
    pub fn run() {
        unsafe {
            let instance = HINSTANCE(GetModuleHandleW(None).unwrap().0);
            let class = w!("BlinkSystemEvents");
            let wc = WNDCLASSW {
                lpfnWndProc: Some(window_proc),
                hInstance: instance,
                lpszClassName: class,
                ..Default::default()
            };
            if RegisterClassW(&wc) == 0 {
                log::error!("Cannot register system event window");
                return;
            }
            // A hidden top-level window receives power broadcasts; HWND_MESSAGE does not.
            let Ok(hwnd) = CreateWindowExW(
                WINDOW_EX_STYLE::default(),
                class,
                w!(""),
                WINDOW_STYLE::default(),
                0,
                0,
                0,
                0,
                None,
                None,
                instance,
                None,
            ) else {
                return;
            };
            if let Err(error) = WTSRegisterSessionNotification(hwnd, NOTIFY_FOR_THIS_SESSION) {
                log::error!("Session registration failed: {error}");
            }
            let mut message = MSG::default();
            while GetMessageW(&mut message, None, 0, 0).0 > 0 {
                let _ = TranslateMessage(&message);
                DispatchMessageW(&message);
            }
            let _ = WTSUnRegisterSessionNotification(hwnd);
            let _ = DestroyWindow(hwnd);
        }
    }
}
