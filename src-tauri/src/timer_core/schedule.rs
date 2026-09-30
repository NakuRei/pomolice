use chrono::{DateTime, Local};

use super::{Effect, TimerCore};
use crate::logging_event::{log_info_event, log_warn_event, BackendLogEvent};
use crate::schedule_boundary::{
    collect_crossed_end_boundaries, collect_crossed_start_boundaries, ActiveWindow,
};
use crate::settings::input::SettingsFileStatus;
use crate::settings::UserSettings;
use crate::timer_core::observation::ActiveWindowChangedPayload;
use crate::timer_core::observation::SettingsChangedPayload;
use crate::types::{BoundaryOccurrenceId, ScheduleStopState, TimerStatus, WorkStartOrigin};

/// What the weekly schedule tracks between ticks. Whether it is enabled
/// and its slots are part of the user settings.
pub(super) struct ScheduleState {
    /// How the running session relates to the schedule window it started in.
    pub(super) stop_state: ScheduleStopState,
    pub(super) last_boundary_check_local: DateTime<Local>,
    /// Set when a running session was restored, so start boundaries crossed
    /// while the app was closed do not fire once that session ends.
    pub(super) starts_blocked_until_idle: bool,
    pub(super) last_published_active_window: Option<ActiveWindow>,
}

impl TimerCore {
    pub(super) fn manual_start_origin(&self, now: DateTime<Local>) -> WorkStartOrigin {
        if self.active_window_at(now).is_some() {
            WorkStartOrigin::ManualInsideScheduleWindow
        } else {
            WorkStartOrigin::ManualOutsideScheduleWindow
        }
    }

    pub(super) fn change_settings(
        &mut self,
        settings: UserSettings,
        now: DateTime<Local>,
    ) -> Vec<Effect> {
        self.settings = settings;
        self.settings_file_status = SettingsFileStatus::Ok;

        log_info_event(
            BackendLogEvent::SettingsUpdateApplied,
            &[("timer_status", format!("{:?}", self.status()))],
        );
        let mut effects = vec![
            Effect::EmitSettingsChanged(Box::new(SettingsChangedPayload {
                settings: self.settings.clone().into(),
                settings_file_status: self.settings_file_status,
            })),
            self.publish(now),
        ];
        // The schedule decides which window is active, so a new schedule is
        // announced right away rather than on the next tick.
        effects.extend(self.publish_active_window_if_changed(now));
        effects
    }

    /// Whether the session in progress ends at this boundary because the
    /// schedule window it belongs to is over. A disabled schedule controls
    /// nothing, whatever stop state was recorded before it was disabled.
    pub(super) fn scheduled_stop_is_due(&self) -> bool {
        self.settings.auto_run_feature_enabled
            && self.schedule.stop_state == ScheduleStopState::StopAtNextSessionBoundary
    }

    pub(super) fn cross_end_boundaries(
        &mut self,
        previous_local: DateTime<Local>,
        now: DateTime<Local>,
    ) -> Vec<Effect> {
        if !self.settings.auto_run_feature_enabled {
            return Vec::new();
        }

        let mut effects = Vec::new();
        for end_boundary in collect_crossed_end_boundaries(
            previous_local,
            now,
            &self.settings.weekly_auto_run_config,
        ) {
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
        if !self.settings.auto_run_feature_enabled {
            return Vec::new();
        }

        let mut effects = Vec::new();
        for start_boundary in collect_crossed_start_boundaries(
            previous_local,
            now,
            &self.settings.weekly_auto_run_config,
        ) {
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
            .start_work(WorkStartOrigin::ScheduleStartBoundary, now)
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
