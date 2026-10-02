// The single owner of timer state. Every decision is made inside `apply`
// from the state it holds and the `now` it is given; nothing here reads a
// clock, awaits, or touches anything outside its own state. Logging is the
// one side effect it performs directly.
//
// The submodules are topical views over this one state, not components:
// each `impl TimerCore` block may read and write any field.

mod boot_auto_start;
mod break_session;
mod effect;
mod event;
pub mod observation;
mod phase;
mod restore;
mod schedule;
mod session;
#[cfg(test)]
mod tests;

use chrono::{DateTime, Utc};

pub use effect::{BreakLockTarget, Effect};
pub use event::Event;

use phase::{BreakSessionState, TimerPhase};
use schedule::ScheduleState;

use crate::error::Result;
use crate::schedule_boundary::{is_now_inside_schedule_window, ActiveWindow};
use crate::settings::UserSettings;
use crate::timer_core::observation::BootAutoStartOfferPayload;
use crate::timer_core::observation::TimerState;
use crate::timer_core::observation::TimerTickPayload;
use crate::types::{SessionDurationSecs, SessionType, TimerCheckpoint, TimerStatus};

pub struct TimerCore<Tz: chrono::TimeZone> {
    timezone: Tz,
    phase: TimerPhase,
    current_cycle: u32,
    completed_pomodoros: u32,
    settings: UserSettings,
    last_transition_at: Option<DateTime<Utc>>,
    boot_auto_start_offer: Option<ActiveWindow>,
    schedule: ScheduleState,
}

impl<Tz: chrono::TimeZone> TimerCore<Tz> {
    /// Applies an event at an explicit wall-clock instant.
    ///
    /// # Errors
    /// Rejects invalid transitions or rate-limited starts without changing state.
    pub fn apply(&mut self, event: Event, now: DateTime<Utc>) -> Result<Vec<Effect>> {
        let had_boot_offer = self.boot_auto_start_offer.is_some();
        let mut effects = match event {
            Event::TimeObserved {
                credited_break_seconds,
            } => Ok(self.advance_time(now, credited_break_seconds)),
            Event::StartWorkRequested => self.start_work_requested(now),
            Event::BootPromptResolved { start_now } => self.resolve_boot_prompt(start_now, now),
            Event::PostBreakDecisionResolved { action } => {
                self.resolve_post_break_decision(action, now)
            }
            Event::BreakAttendanceSet { attendance } => self.set_break_attendance(attendance, now),
            Event::EmergencyStopRequested => Ok(self.emergency_stop(now)),
            Event::SettingsChanged { settings } => Ok(self.change_settings(settings, now)),
            Event::InputDetected => Ok(self.apply_input_penalty(now)),
            Event::EscapeComboPressed => Ok(self.escape_combo_pressed()),
        }?;
        self.clear_invalid_boot_auto_start_offer(now);
        if had_boot_offer && self.boot_auto_start_offer.is_none() {
            effects.push(Effect::EmitBootAutoStartOfferClosed);
        }
        Ok(effects)
    }

    /// The countdown runs before schedule boundaries so that a work session ending on the same second
    /// as the schedule still gets its break; the scheduled stop then takes
    /// effect when that break ends.
    fn advance_time(&mut self, now: DateTime<Utc>, credited_break_seconds: u64) -> Vec<Effect> {
        let previous_at = self.schedule.last_boundary_check_at;
        // Never move backwards: after a clock rollback no boundary fires until
        // the wall clock passes the point already checked.
        self.schedule.last_boundary_check_at = previous_at.max(now);

        let mut effects = self.count_down(now, credited_break_seconds);
        effects.extend(self.cross_end_boundaries(previous_at, now));
        effects.extend(self.cross_start_boundaries(previous_at, now));
        effects.extend(self.publish_active_window_if_changed(now));
        effects
    }

    #[must_use]
    pub fn status(&self) -> TimerStatus {
        self.phase.status()
    }

    #[must_use]
    pub fn settings(&self) -> &UserSettings {
        &self.settings
    }

    #[must_use]
    fn post_break_decision_pending(&self) -> bool {
        self.phase.post_break_decision_pending()
    }

    fn current_break(&self) -> Option<&BreakSessionState> {
        self.phase.current_break()
    }

    fn active_window_at(&self, now: DateTime<Utc>) -> Option<ActiveWindow> {
        if !self.settings.auto_run_feature_enabled {
            return None;
        }
        is_now_inside_schedule_window(
            &now.with_timezone(&self.timezone),
            &self.settings.weekly_auto_run_config,
        )
    }

    #[must_use]
    pub(crate) fn tick_payload(&self, now: DateTime<Utc>) -> TimerTickPayload {
        let current_break = self.current_break();
        TimerTickPayload {
            status: self.status(),
            remaining_seconds: self.remaining_seconds_at(now),
            session_duration_secs: self.session_duration_secs().get(),
            session_type: self.session_type(),
            current_cycle: self.current_cycle,
            completed_pomodoros: self.completed_pomodoros,
            break_interaction_count: current_break.map_or(0, |b| b.interaction_count),
            away_during_break: current_break.is_some_and(|b| b.attendance.is_away()),
        }
    }

    fn publish(&self, now: DateTime<Utc>) -> Effect {
        Effect::PublishTimerState(self.tick_payload(now))
    }

    #[must_use]
    pub fn snapshot(&self, now: DateTime<Utc>) -> TimerState {
        let active_window_summary = self
            .active_window_at(now)
            .as_ref()
            .map(ActiveWindow::to_summary_payload);
        let boot_auto_start_offer =
            self.boot_auto_start_offer
                .as_ref()
                .map(|window| BootAutoStartOfferPayload {
                    anchor_local_date: window.anchor_local_date,
                    window: window.to_summary_payload(),
                });
        let tick = self.tick_payload(now);

        TimerState {
            status: tick.status,
            remaining_seconds: tick.remaining_seconds,
            session_duration_secs: tick.session_duration_secs,
            session_type: tick.session_type,
            current_cycle: tick.current_cycle,
            completed_pomodoros: tick.completed_pomodoros,
            break_interaction_count: tick.break_interaction_count,
            away_during_break: tick.away_during_break,
            post_break_decision_pending: self.post_break_decision_pending(),
            settings: self.settings.clone().into(),
            boot_auto_start_offer,
            active_window_summary,
        }
    }

    #[must_use]
    pub fn checkpoint(&self, now: DateTime<Utc>) -> TimerCheckpoint {
        TimerCheckpoint {
            status: self.status(),
            remaining_seconds: self.remaining_seconds_at(now),
            session_duration_secs: self.session_duration_secs(),
            session_type: self.session_type(),
            current_cycle: self.current_cycle,
            completed_pomodoros: self.completed_pomodoros,
            post_break_decision_pending: self.post_break_decision_pending(),
            saved_at: now,
            schedule_stop_state: self.schedule.stop_state,
        }
    }

    fn session_type(&self) -> SessionType {
        match &self.phase {
            TimerPhase::Idle { .. } | TimerPhase::Working { .. } => SessionType::Work,
            TimerPhase::OnBreak(current_break) => current_break.kind.session_type(),
        }
    }

    fn session_duration_secs(&self) -> SessionDurationSecs {
        match &self.phase {
            TimerPhase::Idle { .. } => self.settings.work_duration_secs.into(),
            TimerPhase::Working { duration_secs, .. } => *duration_secs,
            TimerPhase::OnBreak(current_break) => current_break.duration_secs,
        }
    }

    fn remaining_seconds_at(&self, now: DateTime<Utc>) -> u64 {
        match &self.phase {
            TimerPhase::Idle { .. } => self.settings.work_duration_secs.get(),
            TimerPhase::Working { ends_at, .. } => whole_seconds_until(*ends_at, now),
            TimerPhase::OnBreak(current_break) => current_break.remaining_seconds,
        }
    }
}

/// Rounded up so a tick that runs a few milliseconds late still reports the
/// second that just started.
fn whole_seconds_until(ends_at: DateTime<Utc>, now: DateTime<Utc>) -> u64 {
    let remaining_ms = (ends_at - now).num_milliseconds();
    if remaining_ms <= 0 {
        return 0;
    }
    u64::try_from((remaining_ms + 999) / 1000).unwrap_or(u64::MAX)
}
