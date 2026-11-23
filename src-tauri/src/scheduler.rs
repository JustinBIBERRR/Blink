//! Reminder lifecycle. All callers supply monotonic time; windows never own it.
use crate::state::{DaemonConfig, Phase, ReminderMode, SessionOutcome};
use serde::Serialize;
use std::time::{Duration, Instant};

pub const SOFT_DURATION: Duration = Duration::from_secs(3);

#[derive(Debug, Clone, Serialize)]
pub struct ReminderSnapshot {
    pub generation: u64,
    pub round_id: Option<u64>,
    pub phase: Phase,
    pub elapsed_ms: u64,
    pub mode: ReminderMode,
}

#[derive(Debug, Clone, Copy)]
pub enum SystemEvent {
    Locked(bool),
    Sleeping(bool),
    Silenced(bool),
}

pub struct Scheduler {
    idle_deferred: bool,
    return_grace_until: Option<Instant>,
    locked: bool,
    sleeping: bool,
    last_clock: Instant,
    pub config: DaemonConfig,
    generation: u64,
    next_due: Instant,
    paused_until: Option<Instant>,
    pause_deadline: Option<u64>,
    is_silenced: bool,
    active: Option<(u64, Instant, ReminderMode)>,
}

impl Scheduler {
    pub fn new(config: DaemonConfig, now: Instant) -> Self {
        Self {
            idle_deferred: false,
            return_grace_until: None,
            locked: false,
            sleeping: false,
            last_clock: now,
            next_due: now + Duration::from_secs(config.interval_secs),
            config,
            generation: 0,
            active: None,
            paused_until: None,
            pause_deadline: None,
            is_silenced: false,
        }
    }

    /// Idle input controls display eligibility, never accumulated use time.
    pub fn idle_for(&mut self, idle: Duration, now: Instant) {
        if idle >= Duration::from_secs(300) {
            self.idle_deferred = true;
            self.return_grace_until = None;
        } else if self.idle_deferred {
            self.idle_deferred = false;
            // Start once at the observed return input, not on each subsequent keystroke.
            self.return_grace_until = Some(now - idle + Duration::from_secs(30));
        }
    }

    pub fn update_config(&mut self, config: DaemonConfig, now: Instant) {
        self.advance_clock(now);
        if self.config.interval_secs != config.interval_secs {
            self.next_due = now + Duration::from_secs(config.interval_secs);
        }
        self.config = config;
    }

    fn advance_clock(&mut self, now: Instant) {
        if self.locked || self.sleeping {
            self.next_due += now.saturating_duration_since(self.last_clock);
        }
        self.last_clock = now;
    }

    pub fn system_event(&mut self, event: SystemEvent, now: Instant) {
        self.advance_clock(now);
        match event {
            SystemEvent::Locked(value) => self.locked = value,
            SystemEvent::Sleeping(value) => self.sleeping = value,
            SystemEvent::Silenced(value) => {
                if self.is_silenced == value {
                    return;
                }
                self.is_silenced = value;
            }
        }
        if self.locked || self.sleeping || self.is_silenced {
            if let Some((id, _, _)) = self.active {
                self.finish(id, now);
            } else {
                self.generation += 1;
            }
        }
    }

    pub fn preview_if_current(&mut self, generation: u64, now: Instant) -> bool {
        if self.generation != generation || self.locked || self.sleeping || self.is_silenced {
            return false;
        }
        self.preview(now);
        true
    }

    pub fn preview(&mut self, now: Instant) -> u64 {
        self.advance_clock(now);
        if self.locked || self.sleeping || self.is_silenced {
            return self.generation;
        }

        self.generation += 1;
        self.active = Some((self.generation, now, self.config.reminder_mode));
        self.next_due = now + Duration::from_secs(self.config.interval_secs);
        self.generation
    }

    pub fn tick(&mut self, now: Instant) -> Option<SessionOutcome> {
        self.advance_clock(now);
        let mut outcome = None;
        if self.active.is_some_and(|(_, start, mode)| {
            now.saturating_duration_since(start)
                >= if mode == ReminderMode::Fog {
                    Duration::from_secs(23)
                } else {
                    SOFT_DURATION
                }
        }) {
            self.active = None;
            self.generation += 1;
            outcome = Some(SessionOutcome::Completed);
        }

        if let Some(until) = self.paused_until {
            if now < until {
                return outcome;
            }
            self.paused_until = None;
            self.pause_deadline = None;
            self.next_due = now + Duration::from_secs(self.config.interval_secs);
        }
        if self.active.is_none()
            && now >= self.next_due
            && !self.idle_deferred
            && self.return_grace_until.is_none_or(|until| now >= until)
            && !self.is_silenced
            && !self.locked
            && !self.sleeping
        {
            self.preview(now);
        }

        outcome
    }

    /// Wall time remains authoritative even when the monotonic clock stops in sleep.
    pub fn refresh_pause(&mut self, wall_now: u64, now: Instant) {
        if let Some(deadline) = self.pause_deadline {
            if deadline <= wall_now {
                self.pause(0, now);
            } else {
                self.paused_until = Some(now + Duration::from_secs(deadline - wall_now));
            }
        }
    }

    pub fn pause_deadline(&self) -> Option<u64> {
        self.pause_deadline
    }

    /// Restore a persisted Unix deadline using the current wall and monotonic clocks.
    pub fn restore_pause(&mut self, deadline: Option<u64>, wall_now: u64, now: Instant) {
        let deadline = deadline.filter(|until| *until > wall_now);
        self.pause(deadline.map(|until| until - wall_now).unwrap_or(0), now);
        self.pause_deadline = deadline;
    }

    pub fn pause(&mut self, seconds: u64, now: Instant) {
        self.advance_clock(now);
        // A resumed interval replaces any previously deferred reminder.
        self.return_grace_until = None;
        self.idle_deferred = false;
        // Invalidate preview requests that are still preparing native windows.
        self.generation += 1;
        self.pause_deadline = None;
        if let Some((id, _, _)) = self.active {
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
        if self.active.map(|(id, _, _)| id) != Some(round_id) {
            return false;
        }
        self.active = None;
        self.generation += 1;
        true
    }

    pub fn snapshot(&self, now: Instant) -> ReminderSnapshot {
        ReminderSnapshot {
            generation: self.generation,
            round_id: self.active.map(|(id, _, _)| id),
            mode: self
                .active
                .map(|(_, _, mode)| mode)
                .unwrap_or(self.config.reminder_mode),
            phase: if let Some((_, start, mode)) = self.active {
                let elapsed = now.saturating_duration_since(start).as_secs_f64();
                if mode == ReminderMode::Soft || elapsed < 3.0 {
                    Phase::Warning
                } else if elapsed < 8.0 {
                    Phase::Spreading
                } else if elapsed < 20.0 {
                    Phase::Holding
                } else {
                    Phase::Dissolving
                }
            } else {
                Phase::Monitoring
            },
            elapsed_ms: self
                .active
                .map(|(_, start, _)| now.saturating_duration_since(start).as_millis() as u64)
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
    fn fullscreen_defers_one_reminder_and_respects_return_grace_and_display_interval() {
        let now = Instant::now();
        let mut config = DaemonConfig::default();
        config.interval_secs = 60;
        let mut scheduler = Scheduler::new(config, now);
        scheduler.system_event(SystemEvent::Silenced(true), now);
        scheduler.tick(now + Duration::from_secs(60));
        scheduler.idle_for(Duration::from_secs(300), now + Duration::from_secs(600));
        scheduler.tick(now + Duration::from_secs(600));
        assert_eq!(scheduler.snapshot(now).round_id, None);
        scheduler.system_event(SystemEvent::Silenced(false), now + Duration::from_secs(600));
        scheduler.idle_for(Duration::ZERO, now + Duration::from_secs(601));
        scheduler.tick(now + Duration::from_secs(630));
        assert_eq!(scheduler.snapshot(now).round_id, None);
        scheduler.tick(now + Duration::from_secs(631));
        assert!(scheduler.snapshot(now).round_id.is_some());
        scheduler.tick(now + Duration::from_secs(634));
        scheduler.tick(now + Duration::from_secs(690));
        assert_eq!(scheduler.snapshot(now).round_id, None);
        scheduler.tick(now + Duration::from_secs(691));
        assert!(scheduler.snapshot(now).round_id.is_some());
    }

    #[test]
    fn exiting_presentation_continues_use_time_without_a_new_interval() {
        let now = Instant::now();
        let mut config = DaemonConfig::default();
        config.interval_secs = 60;
        let mut scheduler = Scheduler::new(config, now);
        scheduler.system_event(SystemEvent::Silenced(true), now + Duration::from_secs(10));
        scheduler.system_event(SystemEvent::Silenced(false), now + Duration::from_secs(40));
        scheduler.tick(now + Duration::from_secs(59));
        assert_eq!(scheduler.snapshot(now).round_id, None);
        scheduler.tick(now + Duration::from_secs(60));
        assert!(scheduler.snapshot(now).round_id.is_some());
    }

    #[test]
    fn silence_interrupts_round_and_rejects_delayed_preview() {
        let now = Instant::now();
        let mut scheduler = Scheduler::new(DaemonConfig::default(), now);
        let id = scheduler.preview(now);
        let generation = scheduler.snapshot(now).generation;
        scheduler.system_event(SystemEvent::Silenced(true), now);
        assert_eq!(scheduler.snapshot(now).round_id, None);
        assert!(!scheduler.preview_if_current(generation, now));
        scheduler.preview(now);
        assert_eq!(scheduler.snapshot(now).round_id, None);
        scheduler.system_event(SystemEvent::Silenced(false), now);
        assert!(!scheduler.preview_if_current(generation, now));
        assert!(!scheduler.finish(id, now));
        assert_eq!(scheduler.tick(now + Duration::from_secs(3)), None);
    }

    #[test]
    fn explicit_resume_discards_old_deferred_round_and_return_grace() {
        let now = Instant::now();
        let mut config = DaemonConfig::default();
        config.interval_secs = 10;
        let mut scheduler = Scheduler::new(config, now);
        scheduler.idle_for(Duration::from_secs(300), now + Duration::from_secs(300));
        scheduler.tick(now + Duration::from_secs(300));
        scheduler.pause(60, now + Duration::from_secs(300));
        scheduler.idle_for(Duration::ZERO, now + Duration::from_secs(301));
        scheduler.pause(0, now + Duration::from_secs(302));
        scheduler.tick(now + Duration::from_secs(311));
        assert_eq!(scheduler.snapshot(now).round_id, None);
        scheduler.tick(now + Duration::from_secs(312));
        assert!(scheduler.snapshot(now).round_id.is_some());
    }

    #[test]
    fn pause_deadline_expires_when_wall_clock_advances_during_sleep() {
        let now = Instant::now();
        let mut config = DaemonConfig::default();
        config.interval_secs = 10;
        let mut scheduler = Scheduler::new(config, now);
        scheduler.restore_pause(Some(1060), 1000, now);
        scheduler.refresh_pause(1061, now);
        assert_eq!(scheduler.pause_deadline(), None);
        scheduler.tick(now + Duration::from_secs(9));
        assert_eq!(scheduler.snapshot(now).round_id, None);
        scheduler.tick(now + Duration::from_secs(10));
        assert!(scheduler.snapshot(now).round_id.is_some());
    }

    #[test]
    fn five_minutes_idle_defers_due_reminder_but_reading_before_threshold_counts() {
        let now = Instant::now();
        let mut config = DaemonConfig::default();
        config.interval_secs = 299;
        let mut reading = Scheduler::new(config.clone(), now);
        reading.idle_for(Duration::from_secs(299), now + Duration::from_secs(299));
        reading.tick(now + Duration::from_secs(299));
        assert!(reading.snapshot(now).round_id.is_some());
        config.interval_secs = 300;
        let mut away = Scheduler::new(config, now);
        away.idle_for(Duration::from_secs(300), now + Duration::from_secs(300));
        away.tick(now + Duration::from_secs(300));
        assert_eq!(away.snapshot(now).round_id, None);
    }

    #[test]
    fn missed_intervals_coalesce_and_return_grace_resets_interval_only_when_shown() {
        let now = Instant::now();
        let mut config = DaemonConfig::default();
        config.interval_secs = 300;
        let mut scheduler = Scheduler::new(config, now);
        scheduler.idle_for(Duration::from_secs(300), now + Duration::from_secs(300));
        scheduler.tick(now + Duration::from_secs(300));
        scheduler.idle_for(Duration::from_secs(3600), now + Duration::from_secs(3600));
        scheduler.tick(now + Duration::from_secs(3600));
        assert_eq!(scheduler.snapshot(now).round_id, None);
        scheduler.idle_for(Duration::ZERO, now + Duration::from_secs(3601));
        scheduler.tick(now + Duration::from_secs(3601));
        assert_eq!(scheduler.snapshot(now).round_id, None);
        scheduler.idle_for(Duration::ZERO, now + Duration::from_secs(3630));
        scheduler.tick(now + Duration::from_secs(3630));
        assert_eq!(scheduler.snapshot(now).round_id, None);
        scheduler.tick(now + Duration::from_secs(3631));
        assert!(scheduler.snapshot(now).round_id.is_some());
        scheduler.tick(now + Duration::from_secs(3634));
        assert_eq!(scheduler.snapshot(now).round_id, None);
        scheduler.tick(now + Duration::from_secs(3930));
        assert_eq!(scheduler.snapshot(now).round_id, None);
        scheduler.tick(now + Duration::from_secs(3931));
        assert!(scheduler.snapshot(now).round_id.is_some());
    }

    #[test]
    fn pause_invalidates_prepared_preview_and_restores_until_deadline() {
        let now = Instant::now();
        let mut config = DaemonConfig::default();
        config.interval_secs = 10;
        let mut scheduler = Scheduler::new(config.clone(), now);
        let generation = scheduler.snapshot(now).generation;
        scheduler.pause(60, now);
        assert!(!scheduler.preview_if_current(generation, now));
        let mut restarted = Scheduler::new(config, now);
        restarted.restore_pause(Some(1060), 1030, now);
        assert_eq!(restarted.pause_deadline(), Some(1060));
        restarted.tick(now + Duration::from_secs(29));
        assert_eq!(restarted.snapshot(now).round_id, None);
        restarted.tick(now + Duration::from_secs(30));
        assert_eq!(restarted.pause_deadline(), None);
        restarted.tick(now + Duration::from_secs(39));
        assert_eq!(restarted.snapshot(now).round_id, None);
        restarted.tick(now + Duration::from_secs(40));
        assert!(restarted.snapshot(now).round_id.is_some());
    }

    #[test]
    fn closing_fog_ends_current_round_and_rejects_delayed_close() {
        let now = Instant::now();
        let mut config = DaemonConfig::default();
        config.reminder_mode = ReminderMode::Fog;
        let mut scheduler = Scheduler::new(config, now);
        let first = scheduler.preview(now);
        // Saving another mode must not change a round already on screen.
        scheduler.config.reminder_mode = ReminderMode::Soft;
        assert_eq!(
            scheduler.snapshot(now + Duration::from_secs(5)).phase,
            Phase::Spreading
        );
        assert!(scheduler.finish(first, now + Duration::from_secs(5)));
        assert_eq!(scheduler.snapshot(now).round_id, None);
        assert_eq!(scheduler.tick(now + Duration::from_secs(23)), None);
        let next = scheduler.preview(now + Duration::from_secs(24));
        assert!(!scheduler.finish(first, now + Duration::from_secs(25)));
        assert_eq!(scheduler.snapshot(now).round_id, Some(next));
    }

    #[test]
    fn selected_fog_previews_three_second_cue_and_twenty_second_fog() {
        let now = Instant::now();
        let mut config = DaemonConfig::default();
        config.reminder_mode = crate::state::ReminderMode::Fog;
        let mut scheduler = Scheduler::new(config, now);
        let id = scheduler.preview(now);
        assert_eq!(
            scheduler.snapshot(now + Duration::from_millis(2999)).phase,
            Phase::Warning
        );
        scheduler.tick(now + Duration::from_secs(3));
        assert_eq!(
            scheduler.snapshot(now + Duration::from_secs(3)).phase,
            Phase::Spreading
        );
        scheduler.tick(now + Duration::from_secs(20));
        assert_eq!(
            scheduler.snapshot(now + Duration::from_secs(20)).phase,
            Phase::Dissolving
        );
        assert_eq!(scheduler.snapshot(now).round_id, Some(id));
        assert_eq!(
            scheduler.tick(now + Duration::from_secs(23)),
            Some(SessionOutcome::Completed)
        );
        assert!(!scheduler.finish(id, now + Duration::from_secs(24)));
        assert_eq!(scheduler.snapshot(now).round_id, None);
    }

    #[test]
    fn interrupt_invalidates_pending_preview_even_after_unlock() {
        let now = Instant::now();
        let mut scheduler = Scheduler::new(DaemonConfig::default(), now);
        let requested_generation = scheduler.snapshot(now).generation;
        scheduler.system_event(SystemEvent::Locked(true), now);
        scheduler.system_event(SystemEvent::Locked(false), now);
        assert!(!scheduler.preview_if_current(requested_generation, now));
        assert_eq!(scheduler.snapshot(now).round_id, None);
    }

    #[test]
    fn changed_interval_counts_reading_and_restart_begins_fresh() {
        let now = Instant::now();
        let mut scheduler = Scheduler::new(DaemonConfig::default(), now);
        let mut config = DaemonConfig::default();
        config.interval_secs = 10;
        scheduler.update_config(config.clone(), now);
        scheduler.tick(now + Duration::from_secs(9));
        assert_eq!(scheduler.snapshot(now).round_id, None);
        scheduler.tick(now + Duration::from_secs(10));
        assert!(scheduler.snapshot(now).round_id.is_some());
        let mut restarted = Scheduler::new(config, now + Duration::from_secs(11));
        restarted.tick(now + Duration::from_secs(20));
        assert_eq!(restarted.snapshot(now).round_id, None);
        restarted.tick(now + Duration::from_secs(21));
        assert!(restarted.snapshot(now).round_id.is_some());
    }

    #[test]
    fn lock_and_sleep_suspend_independently_and_reject_late_rounds() {
        let now = Instant::now();
        let mut config = DaemonConfig::default();
        config.interval_secs = 10;
        let mut scheduler = Scheduler::new(config, now);
        scheduler.tick(now + Duration::from_secs(4));
        scheduler.system_event(SystemEvent::Locked(true), now + Duration::from_secs(4));
        scheduler.system_event(SystemEvent::Sleeping(true), now + Duration::from_secs(5));
        scheduler.system_event(SystemEvent::Sleeping(false), now + Duration::from_secs(100));
        scheduler.tick(now + Duration::from_secs(200));
        assert_eq!(scheduler.snapshot(now).round_id, None);
        scheduler.system_event(SystemEvent::Locked(false), now + Duration::from_secs(200));
        scheduler.tick(now + Duration::from_secs(205));
        assert_eq!(scheduler.snapshot(now).round_id, None);
        scheduler.tick(now + Duration::from_secs(206));
        let id = scheduler.snapshot(now).round_id.unwrap();
        scheduler.system_event(SystemEvent::Sleeping(true), now + Duration::from_secs(207));
        assert_eq!(scheduler.snapshot(now).round_id, None);
        assert!(!scheduler.finish(id, now + Duration::from_secs(208)));
        scheduler.preview(now + Duration::from_secs(208));
        assert_eq!(scheduler.snapshot(now).round_id, None);
    }

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
