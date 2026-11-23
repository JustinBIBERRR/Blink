use crate::state::DaemonConfig;
use std::path::PathBuf;

fn config_path() -> PathBuf {
    dirs::data_local_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("blink-rs")
        .join("config.json")
}

/// Load persisted config from the default path, or None if absent/corrupt.
pub fn load() -> Option<DaemonConfig> {
    load_from(&config_path())
}

/// Persist config to the default path.
pub fn save(config: &DaemonConfig) -> Result<(), String> {
    save_to(&config_path(), config)
}

fn load_from(path: &PathBuf) -> Option<DaemonConfig> {
    let data = std::fs::read_to_string(path).ok()?;
    // Tolerate a leading UTF-8 BOM (Windows editors like Notepad add one), which
    // serde_json otherwise rejects with "expected value at line 1 column 1".
    let data = data.strip_prefix('\u{feff}').unwrap_or(&data);
    match serde_json::from_str::<DaemonConfig>(data) {
        Ok(cfg) => Some(cfg),
        Err(e) => {
            log::warn!("config.json parse failed, using defaults: {e}");
            None
        }
    }
}

fn save_to(path: &PathBuf, config: &DaemonConfig) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let json = serde_json::to_string_pretty(config).map_err(|e| e.to_string())?;
    std::fs::write(path, json).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shortcuts_are_opt_in_and_existing_custom_shortcut_survives() {
        assert!(DaemonConfig::default().hotkey.is_empty());
        assert!(DaemonConfig::default().eraser_modifier.is_none());
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.json");
        std::fs::write(&path, r#"{"interval_secs":1200,"warning_secs":5,"spreading_secs":20,"holding_secs":20,"dissolving_secs":10,"hotkey":"ctrl+shift+space","auto_start":false}"#).unwrap();
        let mut config = load_from(&path).unwrap();
        assert_eq!(config.hotkey, "ctrl+shift+space");
        assert!(config.eraser_modifier.is_none());
        config.eraser_modifier = Some(crate::state::EraserModifier::Alt);
        save_to(&path, &config).unwrap();
        assert_eq!(load_from(&path).unwrap().eraser_modifier, Some(crate::state::EraserModifier::Alt));
    }

    #[test]
    fn existing_config_without_mode_uses_soft_reminder() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.json");
        std::fs::write(&path, r#"{"interval_secs":1200,"warning_secs":5,"spreading_secs":20,"holding_secs":20,"dissolving_secs":10,"hotkey":"","auto_start":false}"#).unwrap();
        assert!(!load_from(&path).unwrap().eraser_enabled);
        assert_eq!(
            load_from(&path).unwrap().reminder_mode,
            crate::state::ReminderMode::Soft
        );
    }

    #[test]
    fn save_then_load_roundtrips() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.json");
        let mut cfg = DaemonConfig::default();
        cfg.hotkey = "f1+space".to_string();
        cfg.interval_secs = 999;
        cfg.eraser_enabled = true;
        cfg.reminder_mode = crate::state::ReminderMode::Fog;
        save_to(&path, &cfg).unwrap();
        let loaded = load_from(&path).unwrap();
        assert_eq!(loaded.hotkey, "f1+space");
        assert_eq!(loaded.interval_secs, 999);
        assert!(loaded.eraser_enabled);
        assert_eq!(loaded.reminder_mode, crate::state::ReminderMode::Fog);
    }

    #[test]
    fn load_from_missing_file_is_none() {
        let path = PathBuf::from("definitely/does/not/exist/config.json");
        assert!(load_from(&path).is_none());
    }

    #[test]
    fn load_from_tolerates_utf8_bom() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.json");
        let mut cfg = DaemonConfig::default();
        cfg.hotkey = "ctrl+shift+esc".to_string();
        let json = serde_json::to_string_pretty(&cfg).unwrap();
        std::fs::write(&path, format!("\u{feff}{json}")).unwrap();
        let loaded = load_from(&path).expect("BOM-prefixed config should still load");
        assert_eq!(loaded.hotkey, "ctrl+shift+esc");
    }

    #[test]
    fn load_from_garbage_is_none() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.json");
        std::fs::write(&path, "not json {{{").unwrap();
        assert!(load_from(&path).is_none());
    }
}
