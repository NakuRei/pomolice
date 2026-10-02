use chrono::{DateTime, Utc};

use super::{Effect, TimerCore};
use crate::schedule_boundary::{
    collect_crossed_end_boundaries, collect_crossed_start_boundaries, ActiveWindow,
};
use crate::settings::UserSettings;
use crate::timer_core::observation::ActiveWindowChangedPayload;
use crate::timer_core::observation::SettingsChangedPayload;
use crate::types::{BoundaryOccurrenceId, ScheduleStopState, TimerStatus, WorkStartOrigin};

/// What the weekly schedule tracks between ticks. Whether it is enabled
/// and its slots are part of the user settings.
pub(super) struct ScheduleState {
    /// How the running session relates to the schedule window it started in.
    pub(super) stop_state: ScheduleStopState,
    pub(super) last_boundary_check_at: DateTime<Utc>,
    /// Set when a running session was restored, so start boundaries crossed
    /// while the app was closed do not fire once that session ends.
    pub(super) starts_blocked_until_idle: bool,
    pub(super) last_published_active_window: Option<ActiveWindow>,
}

impl<Tz: chrono::TimeZone> TimerCore<Tz> {
    pub(super) fn manual_start_origin(&self, now: DateTime<Utc>) -> WorkStartOrigin {
        if self.active_window_at(now).is_some() {
            WorkStartOrigin::ManualInsideScheduleWindow
        } else {
            WorkStartOrigin::ManualOutsideScheduleWindow
        }
    }

    pub(super) fn change_settings(
        &mut self,
        settings: UserSettings,
        now: DateTime<Utc>,
    ) -> Vec<Effect> {
        self.settings = settings;

        log::info!(
            "event_id=settings.update.applied timer_status={:?}",
            self.status(),
        );
        let mut effects = vec![
            Effect::EmitSettingsChanged(Box::new(SettingsChangedPayload {
                settings: self.settings.clone().into(),
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
        previous_at: DateTime<Utc>,
        now: DateTime<Utc>,
    ) -> Vec<Effect> {
        if !self.settings.auto_run_feature_enabled {
            return Vec::new();
        }

        let mut effects = Vec::new();
        for end_boundary in collect_crossed_end_boundaries(
            &previous_at.with_timezone(&self.timezone),
            &now.with_timezone(&self.timezone),
            &self.settings.weekly_auto_run_config,
        ) {
            log::info!(
                "event_id=scheduler.crossed_end_boundary boundary_id={}",
                end_boundary.as_str(),
            );
            if self.schedule_stop_at_next_session_boundary() {
                log::info!(
                    "event_id=scheduler.marked_stop_pending boundary_id={}",
                    end_boundary.as_str(),
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
        previous_at: DateTime<Utc>,
        now: DateTime<Utc>,
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
            &previous_at.with_timezone(&self.timezone),
            &now.with_timezone(&self.timezone),
            &self.settings.weekly_auto_run_config,
        ) {
            log::info!(
                "event_id=scheduler.crossed_start_boundary boundary_id={}",
                start_boundary.as_str(),
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
        now: DateTime<Utc>,
    ) -> bool {
        if self.status() != TimerStatus::Idle {
            return false;
        }
        if self
            .start_work(WorkStartOrigin::ScheduleStartBoundary, now)
            .is_err()
        {
            log::warn!(
                "event_id=scheduler.skipped_auto_start_due_to_rejection boundary_id={}",
                start_boundary.as_str(),
            );
            return false;
        }

        log::info!(
            "event_id=scheduler.started_work_from_boundary boundary_id={}",
            start_boundary.as_str(),
        );
        true
    }

    pub(super) fn publish_active_window_if_changed(&mut self, now: DateTime<Utc>) -> Vec<Effect> {
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
