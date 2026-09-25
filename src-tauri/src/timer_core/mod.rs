// The single owner of timer state. Every decision is made inside `apply`
// from the state it holds and the `now` it is given; nothing here reads a
// clock, awaits, or touches anything outside its own state. Logging is the
// one side effect it performs directly.
//
// The submodules are topical views over this one state, not components:
// each `impl TimerCore` block may read and write any field.

mod break_session;
mod effect;
mod event;
mod phase;
mod restore;
mod schedule;
mod session;
#[cfg(test)]
mod tests;

use chrono::{DateTime, Local, Utc};

pub(crate) use effect::Effect;
pub(crate) use event::Event;

use phase::{BreakSessionState, TimerPhase};
use schedule::{BootPromptGate, ScheduleState};

use crate::error::Result;
use crate::schedule_boundary::{is_now_inside_schedule_window, ActiveWindow};
use crate::types::{
    FullStatePayload, PersistedState, SessionType, TimerSettings, TimerStatus, TimerTickPayload,
    PERSISTED_STATE_SCHEMA_VERSION,
};

pub(crate) struct TimerCore {
    phase: TimerPhase,
    current_cycle: u32,
    completed_pomodoros: u32,
    settings: TimerSettings,
    last_transition_at: Option<DateTime<Local>>,
    schedule: ScheduleState,
}

impl TimerCore {
    /// `Err` means the event was rejected and the state is unchanged.
    pub(crate) fn apply(&mut self, event: Event, now: DateTime<Local>) -> Result<Vec<Effect>> {
        match event {
            Event::Started => Ok(self.apply_started(now)),
            Event::Tick => Ok(self.apply_tick(now)),
            Event::StartWorkRequested { settings } => self.start_work_requested(settings, now),
            Event::FirstFullStateObserved => {
                self.observe_first_full_state(now);
                Ok(Vec::new())
            }
            Event::BootPromptResolved { start_now } => self.resolve_boot_prompt(start_now, now),
            Event::PostBreakDecisionResolved { action } => {
                self.resolve_post_break_decision(action, now)
            }
            Event::BreakAttendanceSet { attendance } => self.set_break_attendance(attendance, now),
            Event::EmergencyStopRequested => Ok(self.emergency_stop(now)),
            Event::TimerDurationSettingsChanged {
                work_duration_secs,
                short_break_duration_secs,
                long_break_duration_secs,
                long_break_interval,
            } => self.change_timer_durations(
                work_duration_secs,
                short_break_duration_secs,
                long_break_duration_secs,
                long_break_interval,
                now,
            ),
            Event::AutomationPoliciesChanged {
                auto_run_feature_enabled,
                weekly_auto_run_config,
                post_break_transition_policy,
            } => Ok(self.change_automation_policies(
                auto_run_feature_enabled,
                weekly_auto_run_config,
                post_break_transition_policy,
                now,
            )),
            Event::InputDetected => Ok(self.apply_input_penalty(now)),
            Event::EscapeComboPressed => Ok(self.escape_combo_pressed()),
            Event::BreakLockEntered => Ok(self.break_lock_entered(now)),
            Event::BreakLockEnterFailed => Ok(self.break_lock_enter_failed(now)),
            Event::BreakLockReleased => Ok(self.break_lock_released(now)),
            Event::BreakLockReleaseFailed => Ok(self.break_lock_release_failed(now)),
        }
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

    pub(crate) fn post_break_decision_pending(&self) -> bool {
        self.phase.post_break_decision_pending()
    }

    fn current_break(&self) -> Option<&BreakSessionState> {
        self.phase.current_break()
    }

    fn active_window_at(&self, now: DateTime<Local>) -> Option<ActiveWindow> {
        if !self.schedule.enabled {
            return None;
        }
        is_now_inside_schedule_window(now, &self.schedule.config)
    }

    pub(crate) fn tick_payload(&self, now: DateTime<Local>) -> TimerTickPayload {
        let current_break = self.current_break();
        TimerTickPayload {
            status: self.status(),
            remaining_seconds: self.remaining_seconds_at(now),
            session_type: self.session_type(),
            current_cycle: self.current_cycle,
            completed_pomodoros: self.completed_pomodoros,
            break_interaction_count: current_break.map_or(0, |b| b.interaction_count),
            is_fullscreen_break: current_break.is_some_and(|b| b.is_fullscreen),
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
        let boot_auto_start_prompt_window =
            if self.schedule.boot_prompt_gate == BootPromptGate::WaitingForDecision {
                active_window_summary.clone()
            } else {
                None
            };
        let tick = self.tick_payload(now);

        FullStatePayload {
            status: tick.status,
            remaining_seconds: tick.remaining_seconds,
            session_type: tick.session_type,
            current_cycle: tick.current_cycle,
            completed_pomodoros: tick.completed_pomodoros,
            break_interaction_count: tick.break_interaction_count,
            is_fullscreen_break: tick.is_fullscreen_break,
            away_during_break: tick.away_during_break,
            post_break_decision_pending: self.post_break_decision_pending(),
            settings: self.settings.clone(),
            boot_auto_start_prompt_window,
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
            session_type: self.session_type(),
            current_cycle: self.current_cycle,
            completed_pomodoros: self.completed_pomodoros,
            post_break_decision_pending: self.post_break_decision_pending(),
            settings: self.settings.clone(),
            saved_at,
            auto_run_feature_enabled: self.schedule.enabled,
            weekly_auto_run_config: self.schedule.config.clone(),
            schedule_stop_state: self.schedule.stop_state,
        }
    }

    fn session_type(&self) -> SessionType {
        match &self.phase {
            TimerPhase::Idle { .. } | TimerPhase::Working { .. } => SessionType::Work,
            TimerPhase::OnBreak(current_break) => current_break.kind.session_type(),
        }
    }

    fn remaining_seconds_at(&self, now: DateTime<Local>) -> u64 {
        match &self.phase {
            TimerPhase::Idle { .. } => self.settings.work_duration_secs.get(),
            TimerPhase::Working { ends_at } => whole_seconds_until(*ends_at, now),
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
