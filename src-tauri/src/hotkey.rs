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
    #[test]
    fn hotkey_module_compiles() {}
}
