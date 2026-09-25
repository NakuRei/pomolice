use chrono::{DateTime, Local};

use super::{Effect, TimerCore};
use crate::error::Result;
use crate::logging_event::{log_info_event, log_warn_event, BackendLogEvent};
use crate::schedule_boundary::{
    collect_crossed_end_boundaries, collect_crossed_start_boundaries, ActiveWindow,
};
use crate::types::{
    ActiveWindowChangedPayload, BoundaryOccurrenceId, PostBreakTransitionPolicy, ScheduleStopState,
    TimerStatus, WeeklyAutoRunConfig, WorkStartOrigin,
};

/// Gate that keeps schedule starts closed until the frontend has had the
/// chance to show the boot auto-start prompt.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum BootPromptGate {
    WaitingForFirstFullState,
    WaitingForDecision,
    Open,
}

/// Everything the weekly schedule needs to decide on a tick.
pub(super) struct ScheduleState {
    pub(super) enabled: bool,
    pub(super) config: WeeklyAutoRunConfig,
    /// How the running session relates to the schedule window it started in.
    pub(super) stop_state: ScheduleStopState,
    pub(super) last_boundary_check_local: DateTime<Local>,
    /// Set when a running session was restored, so start boundaries crossed
    /// while the app was closed do not fire once that session ends.
    pub(super) starts_blocked_until_idle: bool,
    pub(super) last_published_active_window: Option<ActiveWindow>,
    pub(super) boot_prompt_gate: BootPromptGate,
}

impl TimerCore {
    pub(super) fn observe_first_full_state(&mut self, now: DateTime<Local>) {
        if self.schedule.boot_prompt_gate != BootPromptGate::WaitingForFirstFullState {
            return;
        }
        let should_prompt =
            self.status() == TimerStatus::Idle && self.active_window_at(now).is_some();
        self.schedule.boot_prompt_gate = if should_prompt {
            BootPromptGate::WaitingForDecision
        } else {
            BootPromptGate::Open
        };
    }

    pub(super) fn resolve_boot_prompt(
        &mut self,
        start_now: bool,
        now: DateTime<Local>,
    ) -> Result<Vec<Effect>> {
        if self.schedule.boot_prompt_gate != BootPromptGate::WaitingForDecision {
            return Ok(Vec::new());
        }
        self.schedule.boot_prompt_gate = BootPromptGate::Open;
        if !start_now {
            return Ok(Vec::new());
        }

        self.start_work(self.settings.clone(), self.manual_start_origin(now), now)?;
        Ok(vec![self.publish(now), Effect::Persist])
    }

    pub(super) fn manual_start_origin(&self, now: DateTime<Local>) -> WorkStartOrigin {
        if self.active_window_at(now).is_some() {
            WorkStartOrigin::ManualInsideScheduleWindow
        } else {
            WorkStartOrigin::ManualOutsideScheduleWindow
        }
    }

    pub(super) fn change_automation_policies(
        &mut self,
        auto_run_feature_enabled: bool,
        weekly_auto_run_config: WeeklyAutoRunConfig,
        post_break_transition_policy: PostBreakTransitionPolicy,
        now: DateTime<Local>,
    ) -> Vec<Effect> {
        self.schedule.enabled = auto_run_feature_enabled;
        self.schedule.config = weekly_auto_run_config;
        self.settings.post_break_transition_policy = post_break_transition_policy;
        if !auto_run_feature_enabled {
            self.schedule.stop_state = ScheduleStopState::NotControlledBySchedule;
        }
        // Boundaries that passed under the previous schedule must not replay
        // under the new one.
        self.schedule.last_boundary_check_local = self.schedule.last_boundary_check_local.max(now);

        log_info_event(
            BackendLogEvent::SettingsSyncApplied,
            &[("timer_status", format!("{:?}", self.status()))],
        );
        vec![self.publish(now), Effect::Persist]
    }

    pub(super) fn cross_end_boundaries(
        &mut self,
        previous_local: DateTime<Local>,
        now: DateTime<Local>,
    ) -> Vec<Effect> {
        if !self.schedule.enabled {
            return Vec::new();
        }

        let mut effects = Vec::new();
        for end_boundary in
            collect_crossed_end_boundaries(previous_local, now, &self.schedule.config)
        {
            log_info_event(
                BackendLogEvent::SchedulerCrossedEndBoundary,
                &[("boundary_id", end_boundary.as_str().to_string())],
            );
            if self.schedule_stop_at_next_session_boundary() {
                log_info_event(
                    BackendLogEvent::SchedulerMarkedStopPending,
                    &[("boundary_id", end_boundary.as_str().to_string())],
                );
                effects.push(Effect::EmitAutoRunStopPending);
                effects.push(Effect::Persist);
            }
        }
        effects
    }

    fn schedule_stop_at_next_session_boundary(&mut self) -> bool {
        let is_running = matches!(self.status(), TimerStatus::Working | TimerStatus::OnBreak);
        if !is_running || self.schedule.stop_state != ScheduleStopState::WaitingForScheduleEnd {
            return false;
        }
        self.schedule.stop_state = ScheduleStopState::StopAtNextSessionBoundary;
        true
    }

    pub(super) fn cross_start_boundaries(
        &mut self,
        previous_local: DateTime<Local>,
        now: DateTime<Local>,
    ) -> Vec<Effect> {
        if self.schedule.starts_blocked_until_idle {
            if self.status() != TimerStatus::Idle {
                return Vec::new();
            }
            // The tick that observes idle only opens the gate; boundaries
            // crossed up to now belong to the session that just ended.
            self.schedule.starts_blocked_until_idle = false;
            return Vec::new();
        }
        if !self.schedule.enabled || self.schedule.boot_prompt_gate != BootPromptGate::Open {
            return Vec::new();
        }

        let mut effects = Vec::new();
        for start_boundary in
            collect_crossed_start_boundaries(previous_local, now, &self.schedule.config)
        {
            log_info_event(
                BackendLogEvent::SchedulerCrossedStartBoundary,
                &[("boundary_id", start_boundary.as_str().to_string())],
            );
            if self.start_work_from_boundary(&start_boundary, now) {
                effects.push(self.publish(now));
                effects.push(Effect::Persist);
            }
        }
        effects
    }

    fn start_work_from_boundary(
        &mut self,
        start_boundary: &BoundaryOccurrenceId,
        now: DateTime<Local>,
    ) -> bool {
        if self.status() != TimerStatus::Idle {
            return false;
        }
        if self
            .start_work(
                self.settings.clone(),
                WorkStartOrigin::ScheduleStartBoundary,
                now,
            )
            .is_err()
        {
            log_warn_event(
                BackendLogEvent::SchedulerSkippedAutoStartDueToRejection,
                &[("boundary_id", start_boundary.as_str().to_string())],
            );
            return false;
        }

        log_info_event(
            BackendLogEvent::SchedulerStartedWorkFromBoundary,
            &[("boundary_id", start_boundary.as_str().to_string())],
        );
        true
    }

    pub(super) fn publish_active_window_if_changed(&mut self, now: DateTime<Local>) -> Vec<Effect> {
        let active_window = self.active_window_at(now);
        if active_window == self.schedule.last_published_active_window {
            return Vec::new();
        }
        let payload = ActiveWindowChangedPayload {
            active_window_summary: active_window.as_ref().map(ActiveWindow::to_summary_payload),
        };
        self.schedule.last_published_active_window = active_window;
        vec![Effect::EmitActiveWindowChanged(payload)]
    }
}
