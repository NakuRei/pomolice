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
pub(crate) mod observation;
mod phase;
mod restore;
mod schedule;
mod session;
#[cfg(test)]
mod tests;

use chrono::{DateTime, Local, Utc};

pub(crate) use effect::{BreakLockTarget, Effect};
pub(crate) use event::Event;

use phase::{BreakSessionState, TimerPhase};
use schedule::ScheduleState;

use crate::error::Result;
use crate::schedule_boundary::{is_now_inside_schedule_window, ActiveWindow};
use crate::settings::input::SettingsFileStatus;
use crate::settings::UserSettings;
use crate::timer_core::observation::BootAutoStartOfferPayload;
use crate::timer_core::observation::FullStatePayload;
use crate::timer_core::observation::TimerTickPayload;
use crate::types::{
    PersistedState, SessionDurationSecs, SessionType, TimerStatus, PERSISTED_STATE_SCHEMA_VERSION,
};

pub(crate) struct TimerCore {
    phase: TimerPhase,
    current_cycle: u32,
    completed_pomodoros: u32,
    settings: UserSettings,
    settings_file_status: SettingsFileStatus,
    last_transition_at: Option<DateTime<Local>>,
    boot_auto_start_offer: Option<ActiveWindow>,
    schedule: ScheduleState,
}

impl TimerCore {
    /// `Err` means the event was rejected and the state is unchanged.
    pub(crate) fn apply(&mut self, event: Event, now: DateTime<Local>) -> Result<Vec<Effect>> {
        let had_boot_offer = self.boot_auto_start_offer.is_some();
        let mut effects = match event {
            Event::Started => Ok(self.apply_started(now)),
            Event::Tick => Ok(self.apply_tick(now)),
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

    /// One second of the application clock. The countdown runs before the
    /// schedule boundaries so that a work session ending on the same second
    /// as the schedule still gets its break; the scheduled stop then takes
    /// effect when that break ends.
    fn apply_tick(&mut self, now: DateTime<Local>) -> Vec<Effect> {
        let previous_local = self.schedule.last_boundary_check_local;
        // Never move backwards: after a clock rollback no boundary fires until
        // the wall clock passes the point already checked.
        self.schedule.last_boundary_check_local = previous_local.max(now);

        let mut effects = self.count_down(now);
        effects.extend(self.cross_end_boundaries(previous_local, now));
        effects.extend(self.cross_start_boundaries(previous_local, now));
        effects.extend(self.publish_active_window_if_changed(now));
        effects
    }

    pub(crate) fn status(&self) -> TimerStatus {
        self.phase.status()
    }

    pub(crate) fn settings(&self) -> &UserSettings {
        &self.settings
    }

    pub(crate) fn post_break_decision_pending(&self) -> bool {
        self.phase.post_break_decision_pending()
    }

    fn current_break(&self) -> Option<&BreakSessionState> {
        self.phase.current_break()
    }

    fn active_window_at(&self, now: DateTime<Local>) -> Option<ActiveWindow> {
        if !self.settings.auto_run_feature_enabled {
            return None;
        }
        is_now_inside_schedule_window(now, &self.settings.weekly_auto_run_config)
    }

    pub(crate) fn tick_payload(&self, now: DateTime<Local>) -> TimerTickPayload {
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

    fn publish(&self, now: DateTime<Local>) -> Effect {
        Effect::PublishTimerState(self.tick_payload(now))
    }

    pub(crate) fn full_state_payload(&self, now: DateTime<Local>) -> FullStatePayload {
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

        FullStatePayload {
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
            settings_file_status: self.settings_file_status,
            boot_auto_start_offer,
            active_window_summary,
        }
    }

    pub(crate) fn persisted_state(
        &self,
        now: DateTime<Local>,
        saved_at: DateTime<Utc>,
    ) -> PersistedState {
        PersistedState {
            schema_version: PERSISTED_STATE_SCHEMA_VERSION,
            status: self.status(),
            remaining_seconds: self.remaining_seconds_at(now),
            session_duration_secs: self.session_duration_secs(),
            session_type: self.session_type(),
            current_cycle: self.current_cycle,
            completed_pomodoros: self.completed_pomodoros,
            post_break_decision_pending: self.post_break_decision_pending(),
            saved_at,
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

    fn remaining_seconds_at(&self, now: DateTime<Local>) -> u64 {
        match &self.phase {
            TimerPhase::Idle { .. } => self.settings.work_duration_secs.get(),
            TimerPhase::Working { ends_at, .. } => whole_seconds_until(*ends_at, now),
            TimerPhase::OnBreak(current_break) => current_break.remaining_seconds,
        }
    }
}

/// Rounded up so a tick that runs a few milliseconds late still reports the
/// second that just started.
fn whole_seconds_until(ends_at: DateTime<Local>, now: DateTime<Local>) -> u64 {
    let remaining_ms = (ends_at - now).num_milliseconds();
    if remaining_ms <= 0 {
        return 0;
    }
    u64::try_from((remaining_ms + 999) / 1000).unwrap_or(u64::MAX)
}
