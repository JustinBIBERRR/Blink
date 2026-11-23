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

#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReminderMode {
    #[default]
    Soft,
    Fog,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DaemonConfig {
    #[serde(default = "enabled")]
    pub fog_notifications: bool,
    #[serde(default)]
    pub reminder_mode: ReminderMode,
    pub interval_secs: u64,
    pub warning_secs: u64,
    pub spreading_secs: u64,
    pub holding_secs: u64,
    pub dissolving_secs: u64,
    pub hotkey: String,
    pub auto_start: bool,
}

fn enabled() -> bool {
    true
}

impl Default for DaemonConfig {
    fn default() -> Self {
        Self {
            fog_notifications: true,
            reminder_mode: ReminderMode::Soft,
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

/// Desired window region for the overlay this instant.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum OverlayHole {
    /// Window hidden (monitoring) — region irrelevant.
    Hidden,
    /// Rounded-rect hole sized as a fraction (0.0..=0.92) of each window axis,
    /// centered. The hole is the clear, operable area.
    Frame(f64),
    /// No hole — the whole window is solid (full-screen blur, holding).
    Full,
}

/// Largest hole: leaves a ~4% blurred frame on each edge during warning.
const HOLE_MAX: f64 = 0.92;

/// Pure geometry: hole for a phase at `elapsed` seconds into it.
pub fn hole_for(phase: &Phase, elapsed: f64, cfg: &DaemonConfig) -> OverlayHole {
    let ease = |t: f64| -> f64 {
        let t = t.clamp(0.0, 1.0);
        if t < 0.5 {
            4.0 * t * t * t
        } else {
            1.0 - (-2.0 * t + 2.0).powi(3) / 2.0
        }
    };
    match phase {
        Phase::Monitoring => OverlayHole::Hidden,
        Phase::Warning => OverlayHole::Frame(HOLE_MAX),
        Phase::Spreading => {
            let t = elapsed / cfg.spreading_secs.max(1) as f64;
            OverlayHole::Frame(HOLE_MAX * (1.0 - ease(t)))
        }
        Phase::Holding => OverlayHole::Full,
        Phase::Dissolving => {
            let t = elapsed / cfg.dissolving_secs.max(1) as f64;
            OverlayHole::Frame(HOLE_MAX * ease(t))
        }
    }
}

impl DaemonState {
    /// Hole for the current phase and elapsed time.
    pub fn overlay_hole(&self) -> OverlayHole {
        hole_for(
            &self.phase,
            self.phase_start.elapsed().as_secs_f64(),
            &self.config,
        )
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

    fn frac(h: OverlayHole) -> f64 {
        match h {
            OverlayHole::Frame(f) => f,
            other => panic!("expected Frame, got {:?}", other),
        }
    }

    #[test]
    fn hole_hidden_when_monitoring() {
        let cfg = DaemonConfig::default();
        assert_eq!(hole_for(&Phase::Monitoring, 0.0, &cfg), OverlayHole::Hidden);
    }

    #[test]
    fn hole_full_when_holding() {
        let cfg = DaemonConfig::default();
        assert_eq!(hole_for(&Phase::Holding, 5.0, &cfg), OverlayHole::Full);
    }

    #[test]
    fn hole_large_static_during_warning() {
        let cfg = DaemonConfig::default();
        assert!((frac(hole_for(&Phase::Warning, 3.0, &cfg)) - 0.92).abs() < 1e-9);
    }

    #[test]
    fn hole_spreads_closed_over_spreading() {
        let cfg = DaemonConfig::default();
        assert!((frac(hole_for(&Phase::Spreading, 0.0, &cfg)) - 0.92).abs() < 1e-9);
        assert!(frac(hole_for(&Phase::Spreading, cfg.spreading_secs as f64, &cfg)).abs() < 1e-9);
    }

    #[test]
    fn hole_reopens_over_dissolving() {
        let cfg = DaemonConfig::default();
        assert!(frac(hole_for(&Phase::Dissolving, 0.0, &cfg)).abs() < 1e-9);
        assert!(
            (frac(hole_for(
                &Phase::Dissolving,
                cfg.dissolving_secs as f64,
                &cfg
            )) - 0.92)
                .abs()
                < 1e-9
        );
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
