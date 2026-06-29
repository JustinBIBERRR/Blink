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
    match serde_json::from_str::<DaemonConfig>(&data) {
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
    fn save_then_load_roundtrips() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.json");
        let mut cfg = DaemonConfig::default();
        cfg.hotkey = "f1+space".to_string();
        cfg.interval_secs = 999;
        save_to(&path, &cfg).unwrap();
        let loaded = load_from(&path).unwrap();
        assert_eq!(loaded.hotkey, "f1+space");
        assert_eq!(loaded.interval_secs, 999);
    }

    #[test]
    fn load_from_missing_file_is_none() {
        let path = PathBuf::from("definitely/does/not/exist/config.json");
        assert!(load_from(&path).is_none());
    }

    #[test]
    fn load_from_garbage_is_none() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.json");
        std::fs::write(&path, "not json {{{").unwrap();
        assert!(load_from(&path).is_none());
    }
}
