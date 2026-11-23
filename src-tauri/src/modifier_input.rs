//! Windows modifier changes for temporary eraser pickup. All keys pass through.
#[cfg(target_os = "windows")]
pub fn normalize_vk(vk: u32) -> u32 {
    match vk {
        0xA2 | 0xA3 => 0x11, // L/R Ctrl  -> Ctrl
        0xA0 | 0xA1 => 0x10, // L/R Shift -> Shift
        0xA4 | 0xA5 => 0x12, // L/R Alt   -> Alt
        0x5C => 0x5B,        // R Win     -> Win
        other => other,
    }
}

#[cfg(target_os = "windows")]
mod hook {
    use super::normalize_vk;
    use windows::Win32::Foundation::{HINSTANCE, LPARAM, LRESULT, WPARAM};
    use windows::Win32::System::LibraryLoader::GetModuleHandleW;
    use windows::Win32::UI::Input::KeyboardAndMouse::GetAsyncKeyState;
    use windows::Win32::UI::WindowsAndMessaging::{
        CallNextHookEx, GetMessageW, SetWindowsHookExW, HHOOK, KBDLLHOOKSTRUCT, MSG,
        WH_KEYBOARD_LL, WM_KEYDOWN, WM_KEYUP, WM_SYSKEYDOWN, WM_SYSKEYUP,
    };

    unsafe fn async_down(vk: u32) -> bool {
        (GetAsyncKeyState(vk as i32) as u16 & 0x8000) != 0
    }

    pub fn install() {
        std::thread::spawn(|| unsafe {
            let hmod = GetModuleHandleW(None).unwrap_or_default();
            let hook = SetWindowsHookExW(
                WH_KEYBOARD_LL,
                Some(keyboard_hook_proc),
                HINSTANCE(hmod.0),
                0,
            );
            if hook.is_err() {
                log::error!("SetWindowsHookExW failed: {:?}", hook.err());
                return;
            }
            // Low-level hooks require a message pump on the installing thread.
            let mut msg = MSG::default();
            while GetMessageW(&mut msg, None, 0, 0).as_bool() {}
        });
    }

    unsafe extern "system" fn keyboard_hook_proc(
        code: i32,
        wparam: WPARAM,
        lparam: LPARAM,
    ) -> LRESULT {
        if code >= 0 {
            let msg = wparam.0 as u32;
            if matches!(msg, WM_KEYDOWN | WM_SYSKEYDOWN | WM_KEYUP | WM_SYSKEYUP) {
                let kb = &*(lparam.0 as *const KBDLLHOOKSTRUCT);
                let changed = normalize_vk(kb.vkCode);
                let changed_down = matches!(msg, WM_KEYDOWN | WM_SYSKEYDOWN)
                    || match kb.vkCode {
                        0xA0 => async_down(0xA1),
                        0xA1 => async_down(0xA0),
                        0xA2 => async_down(0xA3),
                        0xA3 => async_down(0xA2),
                        0xA4 => async_down(0xA5),
                        0xA5 => async_down(0xA4),
                        0x5B => async_down(0x5C),
                        0x5C => async_down(0x5B),
                        _ => false,
                    };
                if matches!(changed, 0x10 | 0x11 | 0x12 | 0x5B) {
                    crate::eraser_input::blink_modifier_event(changed, changed_down);
                }
            }
        }
        CallNextHookEx(HHOOK(std::ptr::null_mut()), code, wparam, lparam)
    }
}

#[cfg(target_os = "windows")]
pub use hook::install;

#[cfg(not(target_os = "windows"))]
pub fn install() {}
