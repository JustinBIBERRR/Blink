use std::collections::HashSet;

/// Map a hotkey token (e.g. "ctrl", "space", "f1", "a") to a Windows virtual-key
/// code. Returns None for unknown tokens. Uses numeric VK constants so this stays
/// cross-platform and unit-testable without the `windows` crate.
pub fn token_to_vk(token: &str) -> Option<u32> {
    Some(match token {
        "ctrl" => 0x11,
        "shift" => 0x10,
        "alt" => 0x12,
        "win" => 0x5B,
        "space" => 0x20,
        "esc" => 0x1B,
        "enter" => 0x0D,
        "tab" => 0x09,
        "left" => 0x25,
        "up" => 0x26,
        "right" => 0x27,
        "down" => 0x28,
        t if t.len() == 1 && t.as_bytes()[0].is_ascii_lowercase() => {
            (t.as_bytes()[0] - b'a') as u32 + 0x41
        }
        t if t.len() == 1 && t.as_bytes()[0].is_ascii_digit() => {
            (t.as_bytes()[0] - b'0') as u32 + 0x30
        }
        t if t.starts_with('f') && t.len() >= 2 => {
            let n: u32 = t[1..].parse().ok()?;
            if (1..=12).contains(&n) {
                0x70 + (n - 1)
            } else {
                return None;
            }
        }
        _ => return None,
    })
}

/// Fold left/right modifier VK codes onto their generic equivalent so the hook
/// can compare against a layout-independent combo.
pub fn normalize_vk(vk: u32) -> u32 {
    match vk {
        0xA2 | 0xA3 => 0x11, // L/R Ctrl  -> Ctrl
        0xA0 | 0xA1 => 0x10, // L/R Shift -> Shift
        0xA4 | 0xA5 => 0x12, // L/R Alt   -> Alt
        0x5C => 0x5B,        // R Win     -> Win
        other => other,
    }
}

/// Parse a `+`-joined hotkey string into a deduped, normalized list of VK codes.
/// Tokens are matched case-insensitively; unknown/empty tokens are skipped.
pub fn parse_combo(hotkey: &str) -> Vec<u32> {
    let mut out = Vec::new();
    for tok in hotkey.split('+') {
        let tok = tok.trim().to_lowercase();
        if tok.is_empty() {
            continue;
        }
        if let Some(vk) = token_to_vk(&tok) {
            let n = normalize_vk(vk);
            if !out.contains(&n) {
                out.push(n);
            }
        }
    }
    out
}

/// True when the combo is non-empty and every key in it is currently pressed.
pub fn combo_matches(pressed: &HashSet<u32>, combo: &[u32]) -> bool {
    !combo.is_empty() && combo.iter().all(|k| pressed.contains(k))
}

#[cfg(target_os = "windows")]
mod hook {
    use super::{combo_matches, normalize_vk};
    use std::collections::HashSet;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::mpsc::Sender;
    use std::sync::{Mutex, OnceLock};

    use windows::Win32::Foundation::{HINSTANCE, LPARAM, LRESULT, WPARAM};
    use windows::Win32::System::LibraryLoader::GetModuleHandleW;
    use windows::Win32::UI::Input::KeyboardAndMouse::GetAsyncKeyState;
    use windows::Win32::UI::WindowsAndMessaging::{
        CallNextHookEx, GetMessageW, SetWindowsHookExW, HHOOK, KBDLLHOOKSTRUCT, MSG,
        WH_KEYBOARD_LL, WM_KEYDOWN, WM_KEYUP, WM_SYSKEYDOWN, WM_SYSKEYUP,
    };

    static ACTIVE_COMBO: Mutex<Vec<u32>> = Mutex::new(Vec::new());
    static ARMED: AtomicBool = AtomicBool::new(true);
    static ESCAPE_TX: OnceLock<Sender<u64>> = OnceLock::new();

    /// True if `vk` is physically held right now, per the OS async key-state
    /// table. Used instead of a hook-maintained pressed-set so a KEYUP we never
    /// received (e.g. an elevated window like Task Manager grabbed focus after
    /// Ctrl+Shift+Esc) cannot leave a key stuck "down".
    unsafe fn async_down(vk: u32) -> bool {
        (GetAsyncKeyState(vk as i32) as u16 & 0x8000) != 0
    }

    pub fn set_combo(combo: Vec<u32>) {
        *ACTIVE_COMBO.lock().unwrap() = combo;
        // Re-arm: a combo change (e.g. via save_config) may land while the old
        // combo's keys are still held, which would otherwise leave the hook
        // stuck un-armed until a full key-release cycle on the new combo.
        ARMED.store(true, Ordering::Relaxed);
    }

    /// Install the global low-level keyboard hook on a dedicated message-pump
    /// thread. `tx` receives one `()` each time the active combo is pressed.
    pub fn install(tx: Sender<u64>) {
        let _ = ESCAPE_TX.set(tx);
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
                        0xA0 => async_down(0xA1), 0xA1 => async_down(0xA0),
                        0xA2 => async_down(0xA3), 0xA3 => async_down(0xA2),
                        0xA4 => async_down(0xA5), 0xA5 => async_down(0xA4),
                        0x5B => async_down(0x5C), 0x5C => async_down(0x5B),
                        _ => false,
                    };
                if matches!(changed, 0x10 | 0x11 | 0x12 | 0x5B) {
                    crate::eraser_input::blink_modifier_event(changed, changed_down);
                }
                let combo = ACTIVE_COMBO.lock().unwrap().clone();
                if !combo.is_empty() {
                    // The key in flight isn't in the async table yet, so use the
                    // message's direction for it; read every other combo key from
                    // the real OS state (immune to KEYUPs we never saw).
                    let down: HashSet<u32> = combo
                        .iter()
                        .copied()
                        .filter(|&vk| if vk == changed { changed_down } else { async_down(vk) })
                        .collect();
                    if combo_matches(&down, &combo) {
                        // Edge-trigger: fire once per press, re-arm when the combo
                        // is no longer fully held.
                        if ARMED.swap(false, Ordering::Relaxed) {
                            if let Some(tx) = ESCAPE_TX.get() {
                                let round = crate::eraser_input::current_round();
                                if round != 0 { crate::eraser_input::capture(None); let _ = tx.send(round); }
                            }
                        }
                    } else {
                        ARMED.store(true, Ordering::Relaxed);
                    }
                }
            }
        }
        CallNextHookEx(HHOOK(std::ptr::null_mut()), code, wparam, lparam)
    }
}

#[cfg(target_os = "windows")]
pub use hook::{install, set_combo};

#[cfg(target_os = "macos")]
mod mac {
    use super::*;
    use std::sync::{Mutex, OnceLock, mpsc::Sender};
    static TX: OnceLock<Sender<u64>> = OnceLock::new();
    static COMBO: Mutex<Vec<u32>> = Mutex::new(Vec::new());
    static KEYS: Mutex<Option<(HashSet<u32>, bool)>> = Mutex::new(None);
    pub fn install(tx: Sender<u64>) { let _ = TX.set(tx); }
    pub fn set_combo(combo: Vec<u32>) {
        *COMBO.lock().unwrap() = combo;
        *KEYS.lock().unwrap() = None;
    }
    #[no_mangle]
    pub extern "C" fn blink_keyboard_event(key: u32, down: bool, flags: u32) {
        let combo = COMBO.lock().unwrap();
        let mut state = KEYS.lock().unwrap();
        if key == u32::MAX { *state = None; return; }
        let (pressed, armed) = state.get_or_insert_with(|| (HashSet::new(), true));
        for (bit, vk) in [(1,0x11), (2,0x10), (4,0x12), (8,0x5B)] {
            if flags & bit != 0 { pressed.insert(vk); } else { pressed.remove(&vk); }
        }
        if key != 0 { if down { pressed.insert(key); } else { pressed.remove(&key); } }
        if combo_matches(pressed, &combo) {
            if *armed {
                let round = crate::eraser_input::current_round();
                if round != 0 {
                    crate::eraser_input::capture(None);
                    if let Some(tx) = TX.get() { let _ = tx.send(round); }
                }
            }
            *armed = false;
        } else { *armed = true; }
    }
}
#[cfg(target_os = "macos")]
pub use mac::{install, set_combo};
#[cfg(not(any(target_os = "windows", target_os = "macos")))]
pub fn install(_tx: std::sync::mpsc::Sender<u64>) {}
#[cfg(not(any(target_os = "windows", target_os = "macos")))]
pub fn set_combo(_combo: Vec<u32>) {}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn token_to_vk_known_keys() {
        assert_eq!(token_to_vk("ctrl"), Some(0x11));
        assert_eq!(token_to_vk("shift"), Some(0x10));
        assert_eq!(token_to_vk("alt"), Some(0x12));
        assert_eq!(token_to_vk("win"), Some(0x5B));
        assert_eq!(token_to_vk("space"), Some(0x20));
        assert_eq!(token_to_vk("esc"), Some(0x1B));
        assert_eq!(token_to_vk("a"), Some(0x41));
        assert_eq!(token_to_vk("z"), Some(0x5A));
        assert_eq!(token_to_vk("0"), Some(0x30));
        assert_eq!(token_to_vk("9"), Some(0x39));
        assert_eq!(token_to_vk("f1"), Some(0x70));
        assert_eq!(token_to_vk("f12"), Some(0x7B));
    }

    #[test]
    fn token_to_vk_unknown_is_none() {
        assert_eq!(token_to_vk(""), None);
        assert_eq!(token_to_vk("f13"), None);
        assert_eq!(token_to_vk("ab"), None);
        assert_eq!(token_to_vk("ctrlx"), None);
    }

    #[test]
    fn normalize_vk_folds_left_right() {
        assert_eq!(normalize_vk(0xA2), 0x11); // LCtrl -> Ctrl
        assert_eq!(normalize_vk(0xA3), 0x11); // RCtrl -> Ctrl
        assert_eq!(normalize_vk(0xA0), 0x10); // LShift -> Shift
        assert_eq!(normalize_vk(0xA1), 0x10); // RShift -> Shift
        assert_eq!(normalize_vk(0xA4), 0x12); // LAlt -> Alt
        assert_eq!(normalize_vk(0xA5), 0x12); // RAlt -> Alt
        assert_eq!(normalize_vk(0x5C), 0x5B); // RWin -> Win
        assert_eq!(normalize_vk(0x70), 0x70); // F1 unchanged
    }

    #[test]
    fn parse_combo_dedupes_and_normalizes() {
        assert_eq!(parse_combo("ctrl+shift+space"), vec![0x11, 0x10, 0x20]);
        assert_eq!(parse_combo("f1+space"), vec![0x70, 0x20]);
        assert_eq!(parse_combo("esc"), vec![0x1B]);
        assert_eq!(parse_combo(""), Vec::<u32>::new());
        assert_eq!(parse_combo("ctrl + ctrl"), vec![0x11]);
        assert_eq!(parse_combo("ctrl+bogus+space"), vec![0x11, 0x20]);
        // Case-insensitive: hand-edited config.json with capitals still works.
        assert_eq!(parse_combo("Ctrl+Space"), vec![0x11, 0x20]);
        assert_eq!(parse_combo("F1"), vec![0x70]);
    }

    #[test]
    fn combo_matches_requires_all_keys_and_nonempty() {
        let mut pressed = HashSet::new();
        pressed.insert(0x70u32); // F1
        pressed.insert(0x20u32); // Space
        assert!(combo_matches(&pressed, &[0x70, 0x20]));
        assert!(combo_matches(&pressed, &[0x70]));
        assert!(!combo_matches(&pressed, &[0x70, 0x11])); // Ctrl not pressed
        assert!(!combo_matches(&pressed, &[])); // empty combo never matches
        // Subset semantics: extra held keys do not prevent a match.
        pressed.insert(0x11u32); // Ctrl also held
        assert!(combo_matches(&pressed, &[0x70, 0x20]));
    }
}
