use crate::state::{DaemonState, Phase, SessionOutcome};
use std::sync::{Arc, Mutex};
use std::time::Instant;
use tokio::time::{sleep, Duration};

pub type SharedState = Arc<Mutex<DaemonState>>;

/// Pure function: given current state, return next phase or None.
pub fn next_phase(state: &DaemonState) -> Option<Phase> {
    if let Some(until) = state.paused_until {
        if Instant::now() < until {
            return None;
        }
    }
    if state.is_silenced && state.phase == Phase::Monitoring {
        return None;
    }
    let elapsed = state.phase_elapsed_secs();
    match state.phase {
        Phase::Monitoring if elapsed >= state.config.interval_secs => Some(Phase::Warning),
        Phase::Warning if elapsed >= state.config.warning_secs => Some(Phase::Spreading),
        Phase::Spreading if elapsed >= state.config.spreading_secs => Some(Phase::Holding),
        Phase::Holding if elapsed >= state.config.holding_secs => Some(Phase::Dissolving),
        Phase::Dissolving if elapsed >= state.config.dissolving_secs => Some(Phase::Monitoring),
        _ => None,
    }
}

/// Returns the SessionOutcome to record when user presses escape in a given phase.
pub fn escape_outcome(phase: &Phase) -> SessionOutcome {
    match phase {
        Phase::Warning => SessionOutcome::WarnSkipped,
        Phase::Spreading => SessionOutcome::Skipped,
        Phase::Holding => SessionOutcome::EarlyExit,
        _ => SessionOutcome::WarnSkipped,
    }
}

/// Main daemon loop. Polls every 500ms for phase transitions.
/// `silence_fn`: called each tick to detect silence conditions.
/// `on_phase_change`: called with (new_phase, Option<outcome_to_record>).
pub async fn run<F>(
    shared: SharedState,
    silence_fn: impl Fn() -> bool + Send + 'static,
    mut on_phase_change: F,
) where
    F: FnMut(Phase, Option<SessionOutcome>) + Send + 'static,
{
    loop {
        sleep(Duration::from_millis(500)).await;

        let transition = {
            let mut state = shared.lock().unwrap();
            state.is_silenced = silence_fn();

            next_phase(&state).map(|next| {
                let came_from = state.phase.clone();
                let outcome = if next == Phase::Monitoring && came_from == Phase::Dissolving {
                    Some(SessionOutcome::Completed)
                } else {
                    None
                };
                state.transition_to(next.clone());
                (next, outcome)
            })
        };

        if let Some((phase, outcome)) = transition {
            on_phase_change(phase, outcome);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::{DaemonConfig, DaemonState, Phase};
    use std::time::Duration;

    fn instant_config() -> DaemonConfig {
        DaemonConfig {
            interval_secs: 1,
            warning_secs: 1,
            spreading_secs: 1,
            holding_secs: 1,
            dissolving_secs: 1,
            ..DaemonConfig::default()
        }
    }

    #[test]
    fn monitoring_transitions_to_warning_when_elapsed() {
        let config = instant_config();
        let mut state = DaemonState::new(config);
        // Fake that 2 seconds have passed
        state.phase_start = std::time::Instant::now() - Duration::from_secs(2);
        let next = next_phase(&state);
        assert_eq!(next, Some(Phase::Warning));
    }

    #[test]
    fn silenced_state_stays_in_monitoring() {
        let config = instant_config();
        let mut state = DaemonState::new(config);
        state.is_silenced = true;
        state.phase_start = std::time::Instant::now() - Duration::from_secs(9999);
        let next = next_phase(&state);
        assert_eq!(next, None); // Silenced: no transition
    }

    #[test]
    fn paused_state_returns_none() {
        let config = instant_config();
        let mut state = DaemonState::new(config);
        state.paused_until = Some(std::time::Instant::now() + Duration::from_secs(3600));
        state.phase_start = std::time::Instant::now() - Duration::from_secs(9999);
        let next = next_phase(&state);
        assert_eq!(next, None);
    }

    #[test]
    fn escape_from_warning_is_warn_skipped() {
        use crate::state::SessionOutcome;
        assert_eq!(escape_outcome(&Phase::Warning), SessionOutcome::WarnSkipped);
    }
}
