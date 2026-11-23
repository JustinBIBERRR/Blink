//! Reminder lifecycle. All callers supply monotonic time; windows never own it.
use crate::state::{DaemonConfig, Phase, SessionOutcome};
use serde::Serialize;
use std::time::{Duration, Instant};

pub const SOFT_DURATION: Duration = Duration::from_secs(3);

#[derive(Debug, Clone, Serialize)]
pub struct ReminderSnapshot {
    pub generation: u64,
    pub round_id: Option<u64>,
    pub phase: Phase,
    pub elapsed_ms: u64,
}

pub struct Scheduler {
    pub config: DaemonConfig,
    generation: u64,
    next_due: Instant,
    paused_until: Option<Instant>,
    pub is_silenced: bool,
    active: Option<(u64, Instant)>,
}

impl Scheduler {
    pub fn new(config: DaemonConfig, now: Instant) -> Self {
        Self {
            next_due: now + Duration::from_secs(config.interval_secs),
            config,
            generation: 0,
            active: None,
            paused_until: None,
            is_silenced: false,
        }
    }

    pub fn preview(&mut self, now: Instant) -> u64 {
        self.generation += 1;
        self.active = Some((self.generation, now));
        self.next_due = now + Duration::from_secs(self.config.interval_secs);
        self.generation
    }

    pub fn tick(&mut self, now: Instant) -> Option<SessionOutcome> {
        let mut outcome = None;
        if self
            .active
            .is_some_and(|(_, start)| now.saturating_duration_since(start) >= SOFT_DURATION)
        {
            self.active = None;
            self.generation += 1;
            outcome = Some(SessionOutcome::Completed);
        }

        if let Some(until) = self.paused_until {
            if now < until {
                return outcome;
            }
            self.paused_until = None;
            self.next_due = now + Duration::from_secs(self.config.interval_secs);
        }
        if self.active.is_none() && now >= self.next_due && !self.is_silenced {
            self.preview(now);
        }

        outcome
    }

    pub fn pause(&mut self, seconds: u64, now: Instant) {
        if let Some((id, _)) = self.active {
            self.finish(id, now);
        }
        self.paused_until = if seconds == 0 {
            None
        } else {
            Some(now + Duration::from_secs(seconds))
        };
        self.next_due = now + Duration::from_secs(self.config.interval_secs);
    }

    pub fn finish(&mut self, round_id: u64, _now: Instant) -> bool {
        if self.active.map(|(id, _)| id) != Some(round_id) {
            return false;
        }
        self.active = None;
        self.generation += 1;
        true
    }

    pub fn snapshot(&self, now: Instant) -> ReminderSnapshot {
        ReminderSnapshot {
            generation: self.generation,
            round_id: self.active.map(|(id, _)| id),
            phase: if self.active.is_some() {
                Phase::Warning
            } else {
                Phase::Monitoring
            },
            elapsed_ms: self
                .active
                .map(|(_, start)| now.saturating_duration_since(start).as_millis() as u64)
                .unwrap_or(0),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::DaemonConfig;
    use std::time::{Duration, Instant};

    #[test]
    fn preview_finishes_after_three_seconds() {
        let now = Instant::now();
        let mut scheduler = Scheduler::new(DaemonConfig::default(), now);
        let id = scheduler.preview(now);
        assert_eq!(scheduler.snapshot(now).round_id, Some(id));
        scheduler.tick(now + Duration::from_millis(2999));
        assert_eq!(scheduler.snapshot(now).phase, Phase::Warning);
        scheduler.tick(now + Duration::from_secs(3));
        assert_eq!(scheduler.snapshot(now).round_id, None);
        assert_eq!(scheduler.snapshot(now).phase, Phase::Monitoring);
    }
    #[test]
    fn replacing_preview_rejects_old_finish_and_completed_round_stays_ended() {
        let now = Instant::now();
        let mut scheduler = Scheduler::new(DaemonConfig::default(), now);
        let old = scheduler.preview(now);
        let current = scheduler.preview(now + Duration::from_secs(1));
        assert!(!scheduler.finish(old, now + Duration::from_secs(2)));
        assert_eq!(scheduler.snapshot(now).round_id, Some(current));
        assert!(scheduler.finish(current, now + Duration::from_secs(2)));
        assert!(!scheduler.finish(current, now + Duration::from_secs(3)));
        assert_eq!(scheduler.snapshot(now).round_id, None);
        assert!(scheduler.snapshot(now).generation > current);
    }

    #[test]
    fn scheduled_reminder_uses_same_lifecycle_and_pause_ends_preview() {
        let now = Instant::now();
        let mut config = DaemonConfig::default();
        config.interval_secs = 10;
        let mut scheduler = Scheduler::new(config, now);
        scheduler.tick(now + Duration::from_secs(10));
        assert_eq!(scheduler.snapshot(now).phase, Phase::Warning);
        scheduler.pause(60, now + Duration::from_secs(11));
        assert_eq!(scheduler.snapshot(now).round_id, None);
        scheduler.tick(now + Duration::from_secs(20));
        assert_eq!(scheduler.snapshot(now).round_id, None);
        scheduler.pause(0, now + Duration::from_secs(21));
        scheduler.tick(now + Duration::from_secs(30));
        assert_eq!(scheduler.snapshot(now).round_id, None);
        scheduler.tick(now + Duration::from_secs(31));
        assert_eq!(scheduler.snapshot(now).phase, Phase::Warning);
    }

    #[test]
    fn preview_while_paused_still_finishes_on_time() {
        let now = Instant::now();
        let mut scheduler = Scheduler::new(DaemonConfig::default(), now);
        scheduler.pause(3600, now);
        scheduler.preview(now);
        scheduler.tick(now + Duration::from_secs(3));
        assert_eq!(scheduler.snapshot(now).round_id, None);
    }

    #[test]
    fn completion_is_reported_once_for_statistics() {
        let now = Instant::now();
        let mut scheduler = Scheduler::new(DaemonConfig::default(), now);
        scheduler.preview(now);
        assert_eq!(
            scheduler.tick(now + Duration::from_secs(3)),
            Some(crate::state::SessionOutcome::Completed)
        );
        assert_eq!(scheduler.tick(now + Duration::from_secs(4)), None);
    }
}
