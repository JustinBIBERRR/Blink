//! Pause is separate from settings so saving settings cannot restore an old pause.
use std::path::{Path, PathBuf};

pub fn path() -> PathBuf {
    dirs::data_local_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("blink-rs")
        .join("pause.json")
}

pub fn wall_now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

pub fn load(path: &Path) -> Option<u64> {
    serde_json::from_str(&std::fs::read_to_string(path).ok()?)
        .ok()
        .flatten()
}
pub fn save(path: &Path, deadline: Option<u64>) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    std::fs::write(
        path,
        serde_json::to_vec(&deadline).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn deadline_survives_restart_and_explicit_resume_clears_it() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("pause.json");
        save(&path, Some(1060)).unwrap();
        assert_eq!(load(&path), Some(1060));
        let now = std::time::Instant::now();
        let mut scheduler =
            crate::scheduler::Scheduler::new(crate::state::DaemonConfig::default(), now);
        scheduler.restore_pause(load(&path), 1030, now);
        assert_eq!(scheduler.pause_deadline(), Some(1060));
        save(&path, None).unwrap();
        assert_eq!(load(&path), None);
        scheduler.restore_pause(Some(1060), 1061, now);
        assert_eq!(scheduler.pause_deadline(), None);
    }
}
