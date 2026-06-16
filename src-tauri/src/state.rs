use serde::{Deserialize, Serialize};
use std::time::Instant;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Phase {
    Monitoring,
    Warning,
    Spreading,
    Holding,
    Dissolving,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SessionOutcome {
    Completed,
    EarlyExit,
    Skipped,
    WarnSkipped,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DaemonConfig {
    pub interval_secs: u64,
    pub warning_secs: u64,
    pub spreading_secs: u64,
    pub holding_secs: u64,
    pub dissolving_secs: u64,
    pub hotkey: String,
    pub auto_start: bool,
}

impl Default for DaemonConfig {
    fn default() -> Self {
        Self {
            interval_secs: 1200,
            warning_secs: 5,
            spreading_secs: 20,
            holding_secs: 20,
            dissolving_secs: 10,
            hotkey: "ctrl+shift+space".to_string(),
            auto_start: true,
        }
    }
}

#[derive(Debug)]
pub struct DaemonState {
    pub phase: Phase,
    pub phase_start: Instant,
    pub config: DaemonConfig,
    pub paused_until: Option<Instant>,
    pub is_silenced: bool,
}

impl DaemonState {
    pub fn new(config: DaemonConfig) -> Self {
        Self {
            phase: Phase::Monitoring,
            phase_start: Instant::now(),
            config,
            paused_until: None,
            is_silenced: false,
        }
    }

    pub fn phase_elapsed_secs(&self) -> u64 {
        self.phase_start.elapsed().as_secs()
    }

    pub fn transition_to(&mut self, phase: Phase) {
        self.phase = phase;
        self.phase_start = Instant::now();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn phase_has_five_variants() {
        let phases = vec![
            Phase::Monitoring,
            Phase::Warning,
            Phase::Spreading,
            Phase::Holding,
            Phase::Dissolving,
        ];
        assert_eq!(phases.len(), 5);
    }

    #[test]
    fn session_outcome_serializes_to_snake_case() {
        let outcome = SessionOutcome::WarnSkipped;
        let s = serde_json::to_string(&outcome).unwrap();
        assert_eq!(s, "\"warn_skipped\"");
    }

    #[test]
    fn daemon_config_defaults_match_20_20_20_rule() {
        let cfg = DaemonConfig::default();
        assert_eq!(cfg.interval_secs, 1200);
        assert_eq!(cfg.warning_secs, 5);
        assert_eq!(cfg.spreading_secs, 20);
        assert_eq!(cfg.holding_secs, 20);
        assert_eq!(cfg.dissolving_secs, 10);
        assert!(cfg.auto_start);
    }
}
