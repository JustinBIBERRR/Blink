//! Reminder lifecycle. All callers supply monotonic time; windows never own it.
use crate::eraser_input::InputEvent;
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
    pub eraser: EraserSnapshot,
}

#[derive(Debug, Clone, Copy)]
pub enum SystemEvent {
    Locked(bool),
    Sleeping(bool),
    Silenced(bool),
    Modifier { key: u32, down: bool },
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct EraserSnapshot {
    pub enabled: bool,
    pub held: bool,
    pub radius: f64,
    pub cursor: Option<[f64; 2]>,
    pub strokes: Vec<[f64; 5]>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NotificationIntent {
    FogStarting,
    FogCleared,
}

pub struct Scheduler {
    acceptance_interval: Option<Duration>,
    pending_display: Option<(u64, bool)>,
    notifications: Vec<NotificationIntent>,
    eraser: EraserSnapshot,
    eraser_last: Option<[f64; 2]>,
    temporary_eraser: bool,
    holder_pickup: Option<Instant>,
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
            acceptance_interval: None,
            pending_display: None,
            notifications: Vec::new(),
            eraser: EraserSnapshot {
                radius: 38.0,
                ..Default::default()
            },
            eraser_last: None,
            temporary_eraser: false,
            holder_pickup: None,
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

    pub fn set_acceptance_interval(&mut self, seconds: Option<u64>) {
        self.acceptance_interval = seconds.map(Duration::from_secs);
    }

    /// Drain native submissions once, independently of per-monitor rendering.
    pub fn take_notifications(&mut self) -> Vec<NotificationIntent> {
        std::mem::take(&mut self.notifications)
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
        if self.config.eraser_modifier != config.eraser_modifier && self.temporary_eraser {
            if let Some((id, _, _)) = self.active {
                self.set_eraser_held(id, false, now);
            }
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
            SystemEvent::Modifier { key, down } => {
                if self.config.eraser_modifier.map(|m| m.key()) == Some(key) {
                    if let Some((id, _, _)) = self.active {
                        if down && !self.eraser.held {
                            self.temporary_eraser = self.set_eraser_held(id, true, now);
                            if self.temporary_eraser {
                                crate::eraser_input::temporary_key(key);
                            }
                        } else if !down && self.temporary_eraser {
                            self.set_eraser_held(id, false, now);
                        }
                    }
                }
                return;
            }
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

    /// Reserve a new request before asynchronous window preparation begins.
    pub fn request_preview(&mut self) -> u64 {
        self.generation += 1;
        self.pending_display = Some((self.generation, false));
        self.generation
    }

    pub fn cancel_presentation(&mut self, request: u64) {
        if self.pending_display.is_some_and(|(id, _)| id == request) {
            self.pending_display = None;
            self.generation += 1;
        }
    }

    pub fn pending_presentation(&self, now: Instant) -> Option<u64> {
        let (generation, automatic) = self.pending_display?;
        if generation != self.generation || self.locked || self.sleeping || self.is_silenced {
            return None;
        }
        if automatic
            && (self.active.is_some()
                || now < self.next_due
                || self.paused_until.is_some_and(|until| now < until)
                || self.idle_deferred
                || self.return_grace_until.is_some_and(|until| now < until))
        {
            return None;
        }
        Some(generation)
    }

    /// Called only after every participating webview has installed its listener.
    pub fn present_if_current(&mut self, generation: u64, now: Instant) -> bool {
        if self.pending_presentation(now) != Some(generation) {
            return false;
        }
        self.preview(now);
        true
    }

    fn preview(&mut self, now: Instant) -> u64 {
        self.advance_clock(now);
        if self.locked || self.sleeping || self.is_silenced {
            return self.generation;
        }

        self.pending_display = None;
        self.reset_eraser();
        self.eraser.enabled =
            self.config.eraser_enabled && self.config.reminder_mode == ReminderMode::Fog;
        self.generation += 1;
        self.notifications
            .retain(|message| *message != NotificationIntent::FogStarting);
        self.active = Some((self.generation, now, self.config.reminder_mode));
        crate::eraser_input::active_round(Some(self.generation));
        if self.config.fog_notifications && self.config.reminder_mode == ReminderMode::Fog {
            self.notifications.push(NotificationIntent::FogStarting);
        }
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
            if self.config.fog_notifications
                && self
                    .active
                    .is_some_and(|(_, _, mode)| mode == ReminderMode::Fog)
            {
                self.notifications.push(NotificationIntent::FogCleared);
            }
            self.reset_eraser();
            self.active = None;
            self.generation += 1;
            if let Some(interval) = self.acceptance_interval {
                self.next_due = now + interval;
            }
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
            if self
                .pending_display
                .is_none_or(|(id, _)| id != self.generation)
            {
                self.pending_display = Some((self.generation, true));
            }
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

    pub fn finish(&mut self, round_id: u64, now: Instant) -> bool {
        if self.active.map(|(id, _, _)| id) != Some(round_id) {
            return false;
        }
        self.notifications
            .retain(|message| *message != NotificationIntent::FogStarting);
        self.reset_eraser();
        self.active = None;
        self.generation += 1;
        if let Some(interval) = self.acceptance_interval {
            self.next_due = now + interval;
        }
        true
    }

    fn reset_eraser(&mut self) {
        crate::eraser_input::capture(None);
        crate::eraser_input::active_round(None);
        crate::eraser_input::temporary_key(0);
        self.eraser = EraserSnapshot {
            radius: 38.0,
            ..Default::default()
        };
        self.eraser_last = None;
        self.temporary_eraser = false;
        self.holder_pickup = None;
    }

    /// Click count comes from the OS/webview; remember the docked first click even
    /// though the first click already picked up the shared eraser.
    pub fn holder_click(&mut self, round_id: u64, double: bool, now: Instant) -> bool {
        if self.active.map(|a| a.0) != Some(round_id) {
            return false;
        }
        if double
            && self.eraser.enabled
            && (!self.eraser.held
                || self
                    .holder_pickup
                    .is_some_and(|at| now.saturating_duration_since(at) <= Duration::from_secs(2)))
        {
            return self.finish(round_id, now);
        }
        let pickup = !self.eraser.held;
        if !self.set_eraser_held(round_id, pickup, now) {
            return false;
        }
        self.holder_pickup = if pickup { Some(now) } else { None };
        true
    }

    pub fn set_eraser_held(&mut self, round_id: u64, held: bool, now: Instant) -> bool {
        if !self.eraser.enabled
            || !self.active.is_some_and(|(id, start, _)| {
                id == round_id && now.saturating_duration_since(start) >= SOFT_DURATION
            })
        {
            return false;
        }
        self.temporary_eraser = false;
        crate::eraser_input::temporary_key(0);
        self.eraser.held = held;
        self.eraser_last = None;
        crate::eraser_input::capture(if held { Some(round_id) } else { None });
        true
    }

    /// Desktop coordinates: macOS points, Windows physical pixels. UI converts per monitor.
    pub fn eraser_input(&mut self, round_id: u64, event: InputEvent, now: Instant) {
        if self.active.map(|a| a.0) != Some(round_id) {
            return;
        }
        if let InputEvent::Modifier { key, down } = event {
            self.system_event(SystemEvent::Modifier { key, down }, now);
            return;
        }
        if !self.eraser.held {
            return;
        }
        let position = match event {
            InputEvent::CapabilityLost => {
                self.set_eraser_held(round_id, false, now);
                return;
            }
            InputEvent::Wheel { delta } => {
                if delta.is_finite() {
                    self.eraser.radius = (self.eraser.radius + delta * 4.0).clamp(12.0, 160.0);
                }
                return;
            }
            InputEvent::Move { position }
            | InputEvent::Press { position }
            | InputEvent::Release { position } => position,
            InputEvent::Modifier { .. } => return,
        };
        let [x, y] = position;
        self.eraser.cursor = Some(position);
        if matches!(event, InputEvent::Press { .. }) {
            self.eraser_last = Some(position);
        }
        if matches!(event, InputEvent::Release { .. }) {
            self.eraser_last = None;
            return;
        }
        if let Some([px, py]) = self.eraser_last {
            self.eraser.strokes.push([px, py, x, y, self.eraser.radius]);
            self.eraser_last = Some(position);
        }
    }

    pub fn snapshot(&self, now: Instant) -> ReminderSnapshot {
        ReminderSnapshot {
            eraser: self.eraser.clone(),
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
    fn tick_ready(scheduler: &mut Scheduler, now: Instant) -> Option<SessionOutcome> {
        let outcome = scheduler.tick(now);
        if let Some(request) = scheduler.pending_presentation(now) {
            scheduler.present_if_current(request, now);
        }
        outcome
    }

    use super::*;
    use crate::state::DaemonConfig;
    use std::time::{Duration, Instant};

    #[test]
    fn short_acceptance_interval_leaves_five_clear_seconds_after_each_end() {
        let now = Instant::now();
        let mut config = DaemonConfig::default();
        config.reminder_mode = ReminderMode::Fog;
        config.interval_secs = 5;
        let mut scheduler = Scheduler::new(config, now);
        scheduler.set_acceptance_interval(Some(5));
        let id = scheduler.preview(now);
        scheduler.finish(id, now + Duration::from_secs(15));
        scheduler.tick(now + Duration::from_secs(19));
        assert!(scheduler
            .pending_presentation(now + Duration::from_secs(19))
            .is_none());
        tick_ready(&mut scheduler, now + Duration::from_secs(20));
        let next = scheduler.snapshot(now).round_id.unwrap();
        scheduler.tick(now + Duration::from_secs(42));
        assert_eq!(scheduler.snapshot(now).round_id, Some(next));
        scheduler.tick(now + Duration::from_secs(43));
        assert!(scheduler.snapshot(now).round_id.is_none());
        scheduler.tick(now + Duration::from_secs(47));
        assert!(scheduler
            .pending_presentation(now + Duration::from_secs(47))
            .is_none());
        scheduler.tick(now + Duration::from_secs(48));
        assert!(scheduler
            .pending_presentation(now + Duration::from_secs(48))
            .is_some());
    }

    #[test]
    fn automatic_reminder_waits_for_display_before_clock_and_notification_start() {
        let now = Instant::now();
        let mut config = DaemonConfig::default();
        config.interval_secs = 10;
        config.reminder_mode = ReminderMode::Fog;
        let mut scheduler = Scheduler::new(config, now);
        scheduler.tick(now + Duration::from_secs(10));
        assert!(scheduler.snapshot(now).round_id.is_none());
        assert!(scheduler.take_notifications().is_empty());
        let request = scheduler
            .pending_presentation(now + Duration::from_secs(10))
            .unwrap();
        scheduler.tick(now + Duration::from_secs(15));
        assert!(scheduler.snapshot(now).round_id.is_none());
        assert!(scheduler.present_if_current(request, now + Duration::from_secs(15)));
        assert_eq!(
            scheduler.snapshot(now + Duration::from_secs(15)).elapsed_ms,
            0
        );
        assert_eq!(
            scheduler.take_notifications(),
            vec![NotificationIntent::FogStarting]
        );
        let id = scheduler.snapshot(now).round_id.unwrap();
        scheduler.finish(id, now + Duration::from_secs(16));
        scheduler.tick(now + Duration::from_secs(24));
        assert!(scheduler
            .pending_presentation(now + Duration::from_secs(24))
            .is_none());
        scheduler.tick(now + Duration::from_secs(25));
        assert!(scheduler
            .pending_presentation(now + Duration::from_secs(25))
            .is_some());
    }

    #[test]
    fn pending_display_cannot_survive_pause_system_interrupt_or_newer_preview() {
        let now = Instant::now();
        for interruption in 0..4 {
            let mut scheduler = Scheduler::new(DaemonConfig::default(), now);
            let old = scheduler.request_preview();
            match interruption {
                0 => scheduler.pause(60, now),
                1 => {
                    scheduler.system_event(SystemEvent::Locked(true), now);
                    scheduler.system_event(SystemEvent::Locked(false), now);
                }
                2 => {
                    scheduler.system_event(SystemEvent::Silenced(true), now);
                    scheduler.system_event(SystemEvent::Silenced(false), now);
                }
                _ => {
                    scheduler.request_preview();
                }
            }
            assert!(!scheduler.present_if_current(old, now));
            assert!(scheduler.snapshot(now).round_id.is_none());
            assert!(scheduler.take_notifications().is_empty());
        }
    }

    #[test]
    fn docked_double_click_finishes_after_first_click_has_picked_up() {
        let now = Instant::now();
        let mut config = DaemonConfig::default();
        config.reminder_mode = ReminderMode::Fog;
        config.eraser_enabled = true;
        let mut scheduler = Scheduler::new(config, now);
        let id = scheduler.preview(now);
        let fog = now + Duration::from_secs(3);
        scheduler.holder_click(id, false, fog);
        assert!(scheduler.snapshot(fog).eraser.held);
        scheduler.holder_click(id, true, fog + Duration::from_millis(120));
        assert!(scheduler.snapshot(fog).round_id.is_none());
        assert!(!scheduler.snapshot(fog).eraser.held);
        assert!(!scheduler.holder_click(id, false, fog));
    }

    #[test]
    fn configured_modifier_temporarily_picks_up_and_returns_without_erasing_marks() {
        let now = Instant::now();
        let mut config = DaemonConfig::default();
        config.reminder_mode = ReminderMode::Fog;
        config.eraser_enabled = true;
        config.eraser_modifier = Some(crate::state::EraserModifier::Alt);
        let mut scheduler = Scheduler::new(config, now);
        let id = scheduler.preview(now);
        let fog = now + Duration::from_secs(3);
        scheduler.system_event(
            SystemEvent::Modifier {
                key: 0x10,
                down: true,
            },
            fog,
        );
        assert!(!scheduler.snapshot(fog).eraser.held);
        scheduler.system_event(
            SystemEvent::Modifier {
                key: 0x12,
                down: true,
            },
            fog,
        );
        assert!(scheduler.snapshot(fog).eraser.held);
        scheduler.eraser_input(
            id,
            InputEvent::Press {
                position: [5.0, 10.0],
            },
            fog,
        );
        scheduler.system_event(
            SystemEvent::Modifier {
                key: 0x12,
                down: false,
            },
            fog,
        );
        assert!(!scheduler.snapshot(fog).eraser.held);
        assert_eq!(scheduler.snapshot(fog).eraser.strokes.len(), 1);
        scheduler.eraser_input(
            id,
            InputEvent::Move {
                position: [100.0, 100.0],
            },
            fog,
        );
        assert_eq!(scheduler.snapshot(fog).eraser.strokes.len(), 1);
        tick_ready(&mut scheduler, now + Duration::from_secs(23));
        assert!(scheduler.snapshot(now).round_id.is_none());
    }

    #[test]
    fn wheel_changes_only_future_strokes_without_extending_round() {
        let now = Instant::now();
        let mut config = DaemonConfig::default();
        config.reminder_mode = ReminderMode::Fog;
        config.eraser_enabled = true;
        let mut scheduler = Scheduler::new(config, now);
        let id = scheduler.preview(now);
        let fog = now + Duration::from_secs(3);
        scheduler.set_eraser_held(id, true, fog);
        scheduler.eraser_input(
            id,
            InputEvent::Press {
                position: [10.0, 10.0],
            },
            fog,
        );
        scheduler.eraser_input(
            id,
            InputEvent::Release {
                position: [10.0, 10.0],
            },
            fog,
        );
        scheduler.eraser_input(id, InputEvent::Wheel { delta: 1.0 }, fog);
        scheduler.eraser_input(
            id,
            InputEvent::Press {
                position: [20.0, 20.0],
            },
            fog,
        );
        let strokes = scheduler.snapshot(fog).eraser.strokes;
        assert_eq!(strokes[0][4], 38.0);
        assert_eq!(strokes[1][4], 42.0);
        tick_ready(&mut scheduler, now + Duration::from_secs(23));
        assert!(scheduler.snapshot(now).round_id.is_none());
    }

    #[test]
    fn fog_submits_once_at_prelude_and_after_natural_fade() {
        let now = Instant::now();
        let mut config = DaemonConfig::default();
        config.reminder_mode = ReminderMode::Fog;
        let mut scheduler = Scheduler::new(config, now);
        scheduler.preview(now);
        assert_eq!(
            scheduler.take_notifications(),
            vec![NotificationIntent::FogStarting]
        );
        assert!(scheduler.take_notifications().is_empty());
        tick_ready(&mut scheduler, now + Duration::from_secs(22));
        assert!(scheduler.take_notifications().is_empty());
        tick_ready(&mut scheduler, now + Duration::from_secs(23));
        assert_eq!(
            scheduler.take_notifications(),
            vec![NotificationIntent::FogCleared]
        );
        tick_ready(&mut scheduler, now + Duration::from_secs(24));
        assert!(scheduler.take_notifications().is_empty());
    }

    #[test]
    fn notifications_can_be_disabled_without_stopping_visuals() {
        let now = Instant::now();
        let mut config = DaemonConfig::default();
        config.reminder_mode = ReminderMode::Fog;
        config.fog_notifications = false;
        let mut scheduler = Scheduler::new(config, now);
        scheduler.preview(now);
        assert_eq!(scheduler.snapshot(now).phase, Phase::Warning);
        assert!(scheduler.take_notifications().is_empty());
        tick_ready(&mut scheduler, now + Duration::from_secs(23));
        assert_eq!(scheduler.snapshot(now).phase, Phase::Monitoring);
        assert!(scheduler.take_notifications().is_empty());
    }

    #[test]
    fn interrupted_round_cancels_unsubmitted_prelude() {
        let now = Instant::now();
        let mut config = DaemonConfig::default();
        config.reminder_mode = ReminderMode::Fog;
        let mut scheduler = Scheduler::new(config, now);
        let id = scheduler.preview(now);
        scheduler.finish(id, now);
        assert!(scheduler.take_notifications().is_empty());
    }

    #[test]
    fn every_round_exit_discards_held_eraser_and_rejects_queued_strokes() {
        for exit in 0..5 {
            let now = Instant::now();
            let mut config = DaemonConfig::default();
            config.reminder_mode = ReminderMode::Fog;
            config.eraser_enabled = true;
            let mut scheduler = Scheduler::new(config, now);
            let id = scheduler.preview(now);
            let fog = now + Duration::from_secs(4);
            assert!(!scheduler.set_eraser_held(id, true, now));
            assert!(scheduler.set_eraser_held(id, true, fog));
            scheduler.eraser_input(
                id,
                InputEvent::Press {
                    position: [20.0, 30.0],
                },
                fog,
            );
            match exit {
                0 => {
                    scheduler.finish(id, fog);
                }
                1 => {
                    tick_ready(&mut scheduler, now + Duration::from_secs(23));
                }
                2 => scheduler.pause(60, fog),
                3 => scheduler.system_event(SystemEvent::Locked(true), fog),
                _ => scheduler.system_event(SystemEvent::Silenced(true), fog),
            }
            scheduler.eraser_input(
                id,
                InputEvent::Move {
                    position: [40.0, 50.0],
                },
                fog,
            );
            assert!(!scheduler.snapshot(fog).eraser.held);
            assert!(scheduler.snapshot(fog).eraser.strokes.is_empty());
        }
    }

    #[test]
    fn eraser_is_shared_retains_multiple_strokes_and_releases_on_interrupt() {
        let now = Instant::now();
        let mut config = DaemonConfig::default();
        config.reminder_mode = ReminderMode::Fog;
        config.eraser_enabled = true;
        let mut scheduler = Scheduler::new(config, now);
        let id = scheduler.preview(now);
        let fog = now + Duration::from_secs(4);
        assert!(scheduler.set_eraser_held(id, true, fog));
        scheduler.eraser_input(
            id,
            InputEvent::Press {
                position: [-100.0, 10.0],
            },
            fog,
        );
        scheduler.eraser_input(
            id,
            InputEvent::Move {
                position: [100.0, 20.0],
            },
            fog,
        );
        scheduler.eraser_input(
            id,
            InputEvent::Release {
                position: [100.0, 20.0],
            },
            fog,
        );
        scheduler.eraser_input(
            id,
            InputEvent::Press {
                position: [300.0, 40.0],
            },
            fog,
        );
        scheduler.eraser_input(
            id,
            InputEvent::Release {
                position: [300.0, 40.0],
            },
            fog,
        );
        assert_eq!(scheduler.snapshot(fog).eraser.strokes.len(), 3);
        assert!(scheduler.set_eraser_held(id, false, fog));
        assert_eq!(scheduler.snapshot(fog).eraser.strokes.len(), 3);
        assert!(!scheduler.snapshot(fog).eraser.held);
        scheduler.system_event(SystemEvent::Sleeping(true), fog);
        assert!(scheduler.snapshot(fog).eraser.strokes.is_empty());
        assert!(!scheduler.set_eraser_held(id, true, fog));
    }

    #[test]
    fn fullscreen_defers_one_reminder_and_respects_return_grace_and_display_interval() {
        let now = Instant::now();
        let mut config = DaemonConfig::default();
        config.interval_secs = 60;
        let mut scheduler = Scheduler::new(config, now);
        scheduler.system_event(SystemEvent::Silenced(true), now);
        tick_ready(&mut scheduler, now + Duration::from_secs(60));
        scheduler.idle_for(Duration::from_secs(300), now + Duration::from_secs(600));
        tick_ready(&mut scheduler, now + Duration::from_secs(600));
        assert_eq!(scheduler.snapshot(now).round_id, None);
        scheduler.system_event(SystemEvent::Silenced(false), now + Duration::from_secs(600));
        scheduler.idle_for(Duration::ZERO, now + Duration::from_secs(601));
        tick_ready(&mut scheduler, now + Duration::from_secs(630));
        assert_eq!(scheduler.snapshot(now).round_id, None);
        tick_ready(&mut scheduler, now + Duration::from_secs(631));
        assert!(scheduler.snapshot(now).round_id.is_some());
        tick_ready(&mut scheduler, now + Duration::from_secs(634));
        tick_ready(&mut scheduler, now + Duration::from_secs(690));
        assert_eq!(scheduler.snapshot(now).round_id, None);
        tick_ready(&mut scheduler, now + Duration::from_secs(691));
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
        tick_ready(&mut scheduler, now + Duration::from_secs(59));
        assert_eq!(scheduler.snapshot(now).round_id, None);
        tick_ready(&mut scheduler, now + Duration::from_secs(60));
        assert!(scheduler.snapshot(now).round_id.is_some());
    }

    #[test]
    fn silence_interrupts_round_and_rejects_delayed_preview() {
        let now = Instant::now();
        let mut scheduler = Scheduler::new(DaemonConfig::default(), now);
        let id = scheduler.preview(now);
        let generation = scheduler.request_preview();
        scheduler.system_event(SystemEvent::Silenced(true), now);
        assert_eq!(scheduler.snapshot(now).round_id, None);
        assert!(!scheduler.present_if_current(generation, now));
        scheduler.preview(now);
        assert_eq!(scheduler.snapshot(now).round_id, None);
        scheduler.system_event(SystemEvent::Silenced(false), now);
        assert!(!scheduler.present_if_current(generation, now));
        assert!(!scheduler.finish(id, now));
        assert_eq!(
            tick_ready(&mut scheduler, now + Duration::from_secs(3)),
            None
        );
    }

    #[test]
    fn soft_and_interrupted_fog_do_not_submit_natural_end() {
        let now = Instant::now();
        let mut soft = Scheduler::new(DaemonConfig::default(), now);
        soft.preview(now);
        tick_ready(&mut soft, now + Duration::from_secs(3));
        assert!(soft.take_notifications().is_empty());
        for interrupt in 0..5 {
            let mut config = DaemonConfig::default();
            config.reminder_mode = ReminderMode::Fog;
            let mut scheduler = Scheduler::new(config, now);
            let id = scheduler.preview(now);
            assert_eq!(
                scheduler.take_notifications(),
                vec![NotificationIntent::FogStarting]
            );
            match interrupt {
                0 => {
                    scheduler.finish(id, now);
                }
                1 => scheduler.system_event(SystemEvent::Locked(true), now),
                2 => scheduler.system_event(SystemEvent::Sleeping(true), now),
                3 => scheduler.system_event(SystemEvent::Silenced(true), now),
                _ => scheduler.pause(60, now),
            }
            tick_ready(&mut scheduler, now + Duration::from_secs(23));
            assert!(scheduler.take_notifications().is_empty());
            assert_eq!(scheduler.snapshot(now).round_id, None);
        }
    }

    #[test]
    fn explicit_resume_discards_old_deferred_round_and_return_grace() {
        let now = Instant::now();
        let mut config = DaemonConfig::default();
        config.interval_secs = 10;
        let mut scheduler = Scheduler::new(config, now);
        scheduler.idle_for(Duration::from_secs(300), now + Duration::from_secs(300));
        tick_ready(&mut scheduler, now + Duration::from_secs(300));
        scheduler.pause(60, now + Duration::from_secs(300));
        scheduler.idle_for(Duration::ZERO, now + Duration::from_secs(301));
        scheduler.pause(0, now + Duration::from_secs(302));
        tick_ready(&mut scheduler, now + Duration::from_secs(311));
        assert_eq!(scheduler.snapshot(now).round_id, None);
        tick_ready(&mut scheduler, now + Duration::from_secs(312));
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
        tick_ready(&mut scheduler, now + Duration::from_secs(9));
        assert_eq!(scheduler.snapshot(now).round_id, None);
        tick_ready(&mut scheduler, now + Duration::from_secs(10));
        assert!(scheduler.snapshot(now).round_id.is_some());
    }

    #[test]
    fn five_minutes_idle_defers_due_reminder_but_reading_before_threshold_counts() {
        let now = Instant::now();
        let mut config = DaemonConfig::default();
        config.interval_secs = 299;
        let mut reading = Scheduler::new(config.clone(), now);
        reading.idle_for(Duration::from_secs(299), now + Duration::from_secs(299));
        tick_ready(&mut reading, now + Duration::from_secs(299));
        assert!(reading.snapshot(now).round_id.is_some());
        config.interval_secs = 300;
        let mut away = Scheduler::new(config, now);
        away.idle_for(Duration::from_secs(300), now + Duration::from_secs(300));
        tick_ready(&mut away, now + Duration::from_secs(300));
        assert_eq!(away.snapshot(now).round_id, None);
    }

    #[test]
    fn missed_intervals_coalesce_and_return_grace_resets_interval_only_when_shown() {
        let now = Instant::now();
        let mut config = DaemonConfig::default();
        config.interval_secs = 300;
        let mut scheduler = Scheduler::new(config, now);
        scheduler.idle_for(Duration::from_secs(300), now + Duration::from_secs(300));
        tick_ready(&mut scheduler, now + Duration::from_secs(300));
        scheduler.idle_for(Duration::from_secs(3600), now + Duration::from_secs(3600));
        tick_ready(&mut scheduler, now + Duration::from_secs(3600));
        assert_eq!(scheduler.snapshot(now).round_id, None);
        scheduler.idle_for(Duration::ZERO, now + Duration::from_secs(3601));
        tick_ready(&mut scheduler, now + Duration::from_secs(3601));
        assert_eq!(scheduler.snapshot(now).round_id, None);
        scheduler.idle_for(Duration::ZERO, now + Duration::from_secs(3630));
        tick_ready(&mut scheduler, now + Duration::from_secs(3630));
        assert_eq!(scheduler.snapshot(now).round_id, None);
        tick_ready(&mut scheduler, now + Duration::from_secs(3631));
        assert!(scheduler.snapshot(now).round_id.is_some());
        tick_ready(&mut scheduler, now + Duration::from_secs(3634));
        assert_eq!(scheduler.snapshot(now).round_id, None);
        tick_ready(&mut scheduler, now + Duration::from_secs(3930));
        assert_eq!(scheduler.snapshot(now).round_id, None);
        tick_ready(&mut scheduler, now + Duration::from_secs(3931));
        assert!(scheduler.snapshot(now).round_id.is_some());
    }

    #[test]
    fn pause_invalidates_prepared_preview_and_restores_until_deadline() {
        let now = Instant::now();
        let mut config = DaemonConfig::default();
        config.interval_secs = 10;
        let mut scheduler = Scheduler::new(config.clone(), now);
        let generation = scheduler.request_preview();
        scheduler.pause(60, now);
        assert!(!scheduler.present_if_current(generation, now));
        let mut restarted = Scheduler::new(config, now);
        restarted.restore_pause(Some(1060), 1030, now);
        assert_eq!(restarted.pause_deadline(), Some(1060));
        tick_ready(&mut restarted, now + Duration::from_secs(29));
        assert_eq!(restarted.snapshot(now).round_id, None);
        tick_ready(&mut restarted, now + Duration::from_secs(30));
        assert_eq!(restarted.pause_deadline(), None);
        tick_ready(&mut restarted, now + Duration::from_secs(39));
        assert_eq!(restarted.snapshot(now).round_id, None);
        tick_ready(&mut restarted, now + Duration::from_secs(40));
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
        assert_eq!(
            tick_ready(&mut scheduler, now + Duration::from_secs(23)),
            None
        );
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
        tick_ready(&mut scheduler, now + Duration::from_secs(3));
        assert_eq!(
            scheduler.snapshot(now + Duration::from_secs(3)).phase,
            Phase::Spreading
        );
        tick_ready(&mut scheduler, now + Duration::from_secs(20));
        assert_eq!(
            scheduler.snapshot(now + Duration::from_secs(20)).phase,
            Phase::Dissolving
        );
        assert_eq!(scheduler.snapshot(now).round_id, Some(id));
        assert_eq!(
            tick_ready(&mut scheduler, now + Duration::from_secs(23)),
            Some(SessionOutcome::Completed)
        );
        assert!(!scheduler.finish(id, now + Duration::from_secs(24)));
        assert_eq!(scheduler.snapshot(now).round_id, None);
    }

    #[test]
    fn interrupt_invalidates_pending_preview_even_after_unlock() {
        let now = Instant::now();
        let mut scheduler = Scheduler::new(DaemonConfig::default(), now);
        let requested_generation = scheduler.request_preview();
        scheduler.system_event(SystemEvent::Locked(true), now);
        scheduler.system_event(SystemEvent::Locked(false), now);
        assert!(!scheduler.present_if_current(requested_generation, now));
        assert_eq!(scheduler.snapshot(now).round_id, None);
    }

    #[test]
    fn changed_interval_counts_reading_and_restart_begins_fresh() {
        let now = Instant::now();
        let mut scheduler = Scheduler::new(DaemonConfig::default(), now);
        let mut config = DaemonConfig::default();
        config.interval_secs = 10;
        scheduler.update_config(config.clone(), now);
        tick_ready(&mut scheduler, now + Duration::from_secs(9));
        assert_eq!(scheduler.snapshot(now).round_id, None);
        tick_ready(&mut scheduler, now + Duration::from_secs(10));
        assert!(scheduler.snapshot(now).round_id.is_some());
        let mut restarted = Scheduler::new(config, now + Duration::from_secs(11));
        tick_ready(&mut restarted, now + Duration::from_secs(20));
        assert_eq!(restarted.snapshot(now).round_id, None);
        tick_ready(&mut restarted, now + Duration::from_secs(21));
        assert!(restarted.snapshot(now).round_id.is_some());
    }

    #[test]
    fn lock_and_sleep_suspend_independently_and_reject_late_rounds() {
        let now = Instant::now();
        let mut config = DaemonConfig::default();
        config.interval_secs = 10;
        let mut scheduler = Scheduler::new(config, now);
        tick_ready(&mut scheduler, now + Duration::from_secs(4));
        scheduler.system_event(SystemEvent::Locked(true), now + Duration::from_secs(4));
        scheduler.system_event(SystemEvent::Sleeping(true), now + Duration::from_secs(5));
        scheduler.system_event(SystemEvent::Sleeping(false), now + Duration::from_secs(100));
        tick_ready(&mut scheduler, now + Duration::from_secs(200));
        assert_eq!(scheduler.snapshot(now).round_id, None);
        scheduler.system_event(SystemEvent::Locked(false), now + Duration::from_secs(200));
        tick_ready(&mut scheduler, now + Duration::from_secs(205));
        assert_eq!(scheduler.snapshot(now).round_id, None);
        tick_ready(&mut scheduler, now + Duration::from_secs(206));
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
        tick_ready(&mut scheduler, now + Duration::from_millis(2999));
        assert_eq!(scheduler.snapshot(now).phase, Phase::Warning);
        tick_ready(&mut scheduler, now + Duration::from_secs(3));
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
        tick_ready(&mut scheduler, now + Duration::from_secs(10));
        assert_eq!(scheduler.snapshot(now).phase, Phase::Warning);
        scheduler.pause(60, now + Duration::from_secs(11));
        assert_eq!(scheduler.snapshot(now).round_id, None);
        tick_ready(&mut scheduler, now + Duration::from_secs(20));
        assert_eq!(scheduler.snapshot(now).round_id, None);
        scheduler.pause(0, now + Duration::from_secs(21));
        tick_ready(&mut scheduler, now + Duration::from_secs(30));
        assert_eq!(scheduler.snapshot(now).round_id, None);
        tick_ready(&mut scheduler, now + Duration::from_secs(31));
        assert_eq!(scheduler.snapshot(now).phase, Phase::Warning);
    }

    #[test]
    fn preview_while_paused_still_finishes_on_time() {
        let now = Instant::now();
        let mut scheduler = Scheduler::new(DaemonConfig::default(), now);
        scheduler.pause(3600, now);
        scheduler.preview(now);
        tick_ready(&mut scheduler, now + Duration::from_secs(3));
        assert_eq!(scheduler.snapshot(now).round_id, None);
    }

    #[test]
    fn completion_is_reported_once_for_statistics() {
        let now = Instant::now();
        let mut scheduler = Scheduler::new(DaemonConfig::default(), now);
        scheduler.preview(now);
        assert_eq!(
            tick_ready(&mut scheduler, now + Duration::from_secs(3)),
            Some(crate::state::SessionOutcome::Completed)
        );
        assert_eq!(
            tick_ready(&mut scheduler, now + Duration::from_secs(4)),
            None
        );
    }
}
