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

use global_hotkey::{GlobalHotKeyEvent, GlobalHotKeyManager, HotKeyState};
use global_hotkey::hotkey::{Code, HotKey, Modifiers};

pub struct HotkeyManager {
    _manager: GlobalHotKeyManager,
    pub hotkey_id: u32,
}

impl HotkeyManager {
    pub fn new() -> Result<Self, Box<dyn std::error::Error>> {
        let manager = GlobalHotKeyManager::new()?;
        let hotkey = HotKey::new(
            Some(Modifiers::CONTROL | Modifiers::SHIFT),
            Code::Space,
        );
        let id = hotkey.id();
        manager.register(hotkey)?;
        Ok(Self { _manager: manager, hotkey_id: id })
    }
}

/// Returns true if the event matches a pressed state for the given hotkey id.
pub fn is_escape_event(event: &GlobalHotKeyEvent, id: u32) -> bool {
    event.id == id && event.state == HotKeyState::Pressed
}

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
