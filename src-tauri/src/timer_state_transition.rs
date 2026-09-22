// Timer State Manager
// Manages timer state and session lifecycle without global statics

use chrono::{DateTime, Local};
use std::time::{Duration, Instant};
use tokio_util::sync::CancellationToken;

use crate::error::{PomoliceError, Result};
use crate::logging_event::{log_info_event, log_warn_event, BackendLogEvent};
use crate::schedule_boundary::{is_now_inside_schedule_window, ActiveWindow};
use crate::types::{
    BreakAttendance, FullStatePayload, LongBreakDurationSecs, LongBreakIntervalCount,
    PostBreakDecisionAction, PostBreakTransitionPolicy, ScheduleStopState, SessionCompletedPayload,
    SessionTransitionOutcome, SessionTransitionReason, SessionType, ShortBreakDurationSecs,
    TimerSettings, TimerState, TimerStatus, TimerTickPayload, WeeklyAutoRunConfig,
    WorkDurationSecs, WorkStartOrigin,
};

const INPUT_PENALTY_DEBOUNCE_DURATION: Duration = Duration::from_millis(100);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ScheduleEndBoundaryResult {
    StopScheduledAtSessionBoundary,
    Ignored,
}

#[derive(Clone, PartialEq, Eq)]
struct AutoRunBoundaryScope {
    auto_run_feature_enabled: bool,
    weekly_auto_run_config: WeeklyAutoRunConfig,
}

impl AutoRunBoundaryScope {
    fn from_timer_state(timer_state: &TimerState) -> Self {
        Self {
            auto_run_feature_enabled: timer_state.auto_run_feature_enabled,
            weekly_auto_run_config: timer_state.weekly_auto_run_config.clone(),
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) struct AutoRunBoundaryChangeIdentity(u64);

impl AutoRunBoundaryChangeIdentity {
    fn initial() -> Self {
        Self(0)
    }

    fn advance(&mut self) {
        self.0 = match self.0.checked_add(1) {
            Some(next) => next,
            None => panic!("auto-run boundary change identity overflowed"),
        };
    }
}

impl std::fmt::Display for AutoRunBoundaryChangeIdentity {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(formatter)
    }
}

#[derive(Clone)]
pub(crate) struct AutoRunBoundaryObservation {
    pub(crate) auto_run_feature_enabled: bool,
    pub(crate) weekly_auto_run_config: WeeklyAutoRunConfig,
    pub(crate) change_identity: AutoRunBoundaryChangeIdentity,
}

impl AutoRunBoundaryObservation {
    pub(crate) fn active_window_at(&self, now_local: DateTime<Local>) -> Option<ActiveWindow> {
        if !self.auto_run_feature_enabled {
            return None;
        }
        is_now_inside_schedule_window(now_local, &self.weekly_auto_run_config)
    }
}

struct AutoRunBoundaryPublication {
    scope: AutoRunBoundaryScope,
    change_identity: AutoRunBoundaryChangeIdentity,
}

impl AutoRunBoundaryPublication {
    fn from_timer_state(timer_state: &TimerState) -> Self {
        Self {
            scope: AutoRunBoundaryScope::from_timer_state(timer_state),
            change_identity: AutoRunBoundaryChangeIdentity::initial(),
        }
    }

    fn observe(&self) -> AutoRunBoundaryObservation {
        AutoRunBoundaryObservation {
            auto_run_feature_enabled: self.scope.auto_run_feature_enabled,
            weekly_auto_run_config: self.scope.weekly_auto_run_config.clone(),
            change_identity: self.change_identity,
        }
    }

    fn publish_if_scope_changed(&mut self, scope: AutoRunBoundaryScope) {
        if self.scope == scope {
            return;
        }

        self.scope = scope;
        self.change_identity.advance();
    }
}

pub(crate) struct SavedTimerSession {
    pub(crate) status: TimerStatus,
    pub(crate) session_type: SessionType,
    pub(crate) remaining_seconds: u64,
    pub(crate) current_cycle: u32,
    pub(crate) completed_pomodoros: u32,
    pub(crate) post_break_decision_pending: bool,
    pub(crate) settings: TimerSettings,
    pub(crate) auto_run_feature_enabled: bool,
    pub(crate) weekly_auto_run_config: WeeklyAutoRunConfig,
    pub(crate) schedule_stop_state: ScheduleStopState,
}

pub(crate) struct RestoreNormalizationSnapshot {
    pub(crate) status: TimerStatus,
    pub(crate) session_type: SessionType,
    pub(crate) remaining_seconds: u64,
    pub(crate) completed_pomodoros: u32,
}

pub(crate) struct RestoredTimerSession {
    pub(crate) timer_state: TimerState,
    pub(crate) pending_decision_normalization: Option<RestoreNormalizationSnapshot>,
}

pub(crate) fn restore_saved_timer_session(
    mut session: SavedTimerSession,
    elapsed_seconds_since_save: u64,
) -> RestoredTimerSession {
    let pending_decision_normalization =
        normalize_restored_pending_post_break_decision(&mut session);
    let remaining_seconds = restored_remaining_seconds(&session, elapsed_seconds_since_save);
    let schedule_stop_state =
        restored_schedule_stop_state(session.status, session.schedule_stop_state);

    RestoredTimerSession {
        timer_state: TimerState {
            status: session.status,
            remaining_seconds,
            session_type: session.session_type,
            current_cycle: session.current_cycle,
            completed_pomodoros: session.completed_pomodoros,
            break_interaction_count: 0,
            last_penalty_time: None,
            settings: session.settings,
            auto_run_feature_enabled: session.auto_run_feature_enabled,
            weekly_auto_run_config: session.weekly_auto_run_config,
            schedule_stop_state,
            post_break_decision_pending: session.post_break_decision_pending,
            is_fullscreen_break: false,
            break_attendance: BreakAttendance::AtDesk,
            last_transition: None,
        },
        pending_decision_normalization,
    }
}

fn normalize_restored_pending_post_break_decision(
    session: &mut SavedTimerSession,
) -> Option<RestoreNormalizationSnapshot> {
    if !session.post_break_decision_pending {
        return None;
    }

    let normalization_snapshot = (session.status != TimerStatus::Idle
        || session.session_type != SessionType::Work
        || session.remaining_seconds != session.settings.work_duration_secs.get())
    .then_some(RestoreNormalizationSnapshot {
        status: session.status,
        session_type: session.session_type,
        remaining_seconds: session.remaining_seconds,
        completed_pomodoros: session.completed_pomodoros,
    });

    session.status = TimerStatus::Idle;
    session.session_type = SessionType::Work;
    session.remaining_seconds = session.settings.work_duration_secs.get();
    session.schedule_stop_state = ScheduleStopState::NotControlledBySchedule;

    normalization_snapshot
}

fn restored_remaining_seconds(session: &SavedTimerSession, elapsed_seconds_since_save: u64) -> u64 {
    // The persisted state file is user-writable, so a restored countdown can
    // carry an arbitrary value. Legitimate saves never exceed the configured
    // session duration; cap anything larger instead of trusting it.
    let configured_session_duration_secs = match session.session_type {
        SessionType::Work => session.settings.work_duration_secs.get(),
        SessionType::ShortBreak => session.settings.short_break_duration_secs.get(),
        SessionType::LongBreak => session.settings.long_break_duration_secs.get(),
    };
    let bounded_remaining_seconds = session
        .remaining_seconds
        .min(configured_session_duration_secs);

    if session.status == TimerStatus::Working {
        bounded_remaining_seconds.saturating_sub(elapsed_seconds_since_save)
    } else {
        bounded_remaining_seconds
    }
}

fn restored_schedule_stop_state(
    status: TimerStatus,
    schedule_stop_state: ScheduleStopState,
) -> ScheduleStopState {
    match status {
        TimerStatus::Idle => ScheduleStopState::NotControlledBySchedule,
        TimerStatus::Working | TimerStatus::OnBreak => schedule_stop_state,
    }
}

fn schedule_stop_state_for_work_start_origin(origin: WorkStartOrigin) -> ScheduleStopState {
    match origin {
        WorkStartOrigin::ManualOutsideScheduleWindow => ScheduleStopState::NotControlledBySchedule,
        WorkStartOrigin::ManualInsideScheduleWindow | WorkStartOrigin::ScheduleStartBoundary => {
            ScheduleStopState::WaitingForScheduleEnd
        }
    }
}

/// Manager for timer state and lifecycle
///
/// This struct holds only pure timer state and task lifecycle handles.
/// All side effects (event emission to frontend) are the caller's responsibility.
pub(crate) struct TimerStateManager {
    state: TimerState,
    auto_run_boundary_publication: AutoRunBoundaryPublication,
    /// Token used to signal the timer task to stop.
    /// Cancelling this token causes the task's `select!` to resolve immediately,
    /// preventing any in-flight tick from being spawned after the task exits.
    cancel_token: Option<CancellationToken>,
    timer_task: Option<tokio::task::JoinHandle<()>>,
}

impl TimerStateManager {
    /// Create a new timer state manager
    pub(crate) fn new(persisted_state: Option<TimerState>) -> Self {
        let state = persisted_state.unwrap_or_default();
        let auto_run_boundary_publication = AutoRunBoundaryPublication::from_timer_state(&state);

        Self {
            state,
            auto_run_boundary_publication,
            cancel_token: None,
            timer_task: None,
        }
    }

    /// Apply persisted state during startup initialization.
    ///
    /// Returns the tick payload to emit if the restored timer was running,
    /// or `None` if the restored timer is idle.
    pub(crate) fn apply_persisted_state(
        &mut self,
        persisted_state: TimerState,
    ) -> Option<TimerTickPayload> {
        self.state = persisted_state;
        self.auto_run_boundary_publication
            .publish_if_scope_changed(AutoRunBoundaryScope::from_timer_state(&self.state));
        if self.state.status != TimerStatus::Idle {
            return Some(self.state.to_tick_payload());
        }
        None
    }

    /// Get current timer status
    pub(crate) fn status(&self) -> TimerStatus {
        self.state.status
    }

    /// Get full state for frontend sync
    pub(crate) fn get_full_state(&self) -> FullStatePayload {
        self.state.to_full_payload()
    }

    /// Get mutable reference to internal state (for persistence)
    pub(crate) fn get_state(&self) -> &TimerState {
        &self.state
    }

    pub(crate) fn observe_auto_run_boundary_scope(&self) -> AutoRunBoundaryObservation {
        self.auto_run_boundary_publication.observe()
    }

    /// Start a work session with an explicit start request origin.
    pub(crate) fn start_work(
        &mut self,
        settings: TimerSettings,
        work_start_origin: WorkStartOrigin,
    ) -> Result<(TimerTickPayload, u64)> {
        if self.state.status != TimerStatus::Idle {
            log_warn_event(
                BackendLogEvent::TimerStartRejected,
                &[
                    ("reason", "invalid_state_transition".to_string()),
                    ("current_state", format!("{:?}", self.state.status)),
                ],
            );
            return Err(PomoliceError::InvalidStateTransition {
                action: "start work".to_string(),
                current_state: self.state.status,
            });
        }

        if work_start_origin != WorkStartOrigin::ScheduleStartBoundary {
            if let Some(remaining_ms) = self.rate_limit_remaining_ms() {
                log_warn_event(
                    BackendLogEvent::TimerStartRejected,
                    &[
                        ("reason", "rate_limited".to_string()),
                        ("current_state", format!("{:?}", self.state.status)),
                    ],
                );
                return Err(PomoliceError::RateLimited { remaining_ms });
            }
        }

        let work_duration_secs = settings.work_duration_secs.get();

        // Update settings
        self.state.settings = settings;

        // Initialize work session
        self.state.status = TimerStatus::Working;
        self.state.session_type = SessionType::Work;
        self.state.remaining_seconds = work_duration_secs;
        self.state.post_break_decision_pending = false;
        self.state.schedule_stop_state =
            schedule_stop_state_for_work_start_origin(work_start_origin);

        // Initialize cycle on first start
        if self.state.current_cycle == 0 {
            self.state.current_cycle = 1;
        }

        self.state.last_transition = Some(Instant::now());

        log_info_event(
            BackendLogEvent::TimerWorkSessionStarted,
            &[
                (
                    "duration_secs",
                    self.state.settings.work_duration_secs.to_string(),
                ),
                ("work_start_origin", format!("{work_start_origin:?}")),
            ],
        );

        let payload = self.state.to_tick_payload();
        Ok((payload, work_duration_secs))
    }

    pub(crate) fn resolve_post_break_decision(
        &mut self,
        action: PostBreakDecisionAction,
    ) -> Result<TimerTickPayload> {
        if self.state.status != TimerStatus::Idle || !self.state.post_break_decision_pending {
            log_warn_event(
                BackendLogEvent::TimerTransitionRejected,
                &[
                    ("action", "resolve_post_break_decision".to_string()),
                    ("current_state", format!("{:?}", self.state.status)),
                    (
                        "pending",
                        self.state.post_break_decision_pending.to_string(),
                    ),
                ],
            );
            return Err(PomoliceError::InvalidStateTransition {
                action: "resolve post-break decision".to_string(),
                current_state: self.state.status,
            });
        }

        match action {
            PostBreakDecisionAction::Continue => {
                self.state.current_cycle += 1;
                self.state.status = TimerStatus::Working;
            }
            PostBreakDecisionAction::StopCycle => {
                self.state.status = TimerStatus::Idle;
            }
        }

        self.state.session_type = SessionType::Work;
        self.state.remaining_seconds = self.state.settings.work_duration_secs.get();
        self.state.post_break_decision_pending = false;
        self.state.is_fullscreen_break = false;
        self.state.schedule_stop_state = ScheduleStopState::NotControlledBySchedule;
        self.state.last_transition = Some(Instant::now());

        self.log_manual_decision_snapshot(action);

        Ok(self.state.to_tick_payload())
    }

    /// Record whether the user is at the desk during the current break.
    pub(crate) fn set_break_attendance(
        &mut self,
        attendance: BreakAttendance,
    ) -> Result<TimerTickPayload> {
        if self.state.status != TimerStatus::OnBreak {
            log_warn_event(
                BackendLogEvent::TimerTransitionRejected,
                &[
                    ("action", "set_break_attendance".to_string()),
                    ("current_state", format!("{:?}", self.state.status)),
                ],
            );
            return Err(PomoliceError::InvalidStateTransition {
                action: "set break attendance".to_string(),
                current_state: self.state.status,
            });
        }

        self.state.break_attendance = attendance;
        Ok(self.state.to_tick_payload())
    }

    /// Emergency stop - reset all state
    ///
    /// Returns `(was_fullscreen_break, tick_payload)`.
    /// The caller is responsible for emitting `"timer_tick"` and exiting fullscreen if needed.
    pub(crate) fn emergency_stop(&mut self) -> (bool, TimerTickPayload) {
        let was_fullscreen_break = self.state.is_fullscreen_break;

        // Reset state
        self.state.status = TimerStatus::Idle;
        self.state.session_type = SessionType::Work;
        self.state.remaining_seconds = self.state.settings.work_duration_secs.get();
        self.state.post_break_decision_pending = false;
        self.state.is_fullscreen_break = false;
        self.state.break_attendance = BreakAttendance::AtDesk;
        self.state.break_interaction_count = 0;
        self.state.last_penalty_time = None;
        self.state.schedule_stop_state = ScheduleStopState::NotControlledBySchedule;
        self.state.last_transition = Some(Instant::now());

        (was_fullscreen_break, self.state.to_tick_payload())
    }

    /// Sync only timer duration settings from frontend.
    ///
    /// This method is restricted to idle state to avoid mid-session duration changes.
    pub(crate) fn sync_timer_duration_settings(
        &mut self,
        work_duration_secs: WorkDurationSecs,
        short_break_duration_secs: ShortBreakDurationSecs,
        long_break_duration_secs: LongBreakDurationSecs,
        long_break_interval: LongBreakIntervalCount,
    ) -> Result<TimerTickPayload> {
        self.ensure_idle_for_duration_setting_sync("sync timer duration settings")?;

        self.state.settings.work_duration_secs = work_duration_secs;
        self.state.settings.short_break_duration_secs = short_break_duration_secs;
        self.state.settings.long_break_duration_secs = long_break_duration_secs;
        self.state.settings.long_break_interval = long_break_interval;

        // Keep idle work preview in sync with updated work duration.
        if self.state.session_type == SessionType::Work {
            self.state.remaining_seconds = self.state.settings.work_duration_secs.get();
        }

        log_info_event(
            BackendLogEvent::SettingsSyncApplied,
            &[("timer_status", format!("{:?}", self.state.status))],
        );

        Ok(self.state.to_tick_payload())
    }

    /// Sync automation policies from frontend.
    ///
    /// This method is allowed in every timer state. It only clears
    /// schedule-stop behavior when the feature is disabled.
    pub(crate) fn sync_automation_policies(
        &mut self,
        auto_run_feature_enabled: bool,
        weekly_auto_run_config: WeeklyAutoRunConfig,
        post_break_transition_policy: PostBreakTransitionPolicy,
    ) -> TimerTickPayload {
        self.apply_auto_run_settings_update(
            auto_run_feature_enabled,
            weekly_auto_run_config,
            post_break_transition_policy,
        );
        self.state.to_tick_payload()
    }

    /// Apply input penalty during break
    ///
    /// Returns the tick payload for the caller to emit as `"timer_tick"`,
    /// or `None` if the penalty was skipped (not on break, or debounced).
    pub(crate) fn apply_input_penalty(&mut self) -> Option<TimerTickPayload> {
        self.apply_input_penalty_at(Instant::now())
    }

    fn apply_input_penalty_at(&mut self, now: Instant) -> Option<TimerTickPayload> {
        // Only apply penalty if on break
        if self.state.status != TimerStatus::OnBreak {
            return None;
        }

        // Debounce: 100ms between penalties
        if let Some(last) = self.state.last_penalty_time {
            let elapsed_since_last_penalty = now.checked_duration_since(last)?;
            if elapsed_since_last_penalty < INPUT_PENALTY_DEBOUNCE_DURATION {
                return None;
            }
        }

        // Increment interaction count
        self.state.break_interaction_count += 1;
        self.state.last_penalty_time = Some(now);

        // Reset break timer to full duration
        self.state.remaining_seconds = self.state.get_break_duration();

        Some(self.state.to_tick_payload())
    }

    /// Set fullscreen break state
    pub(crate) fn set_fullscreen_break(&mut self, is_fullscreen: bool) {
        self.state.is_fullscreen_break = is_fullscreen;
    }

    /// Handle a timer tick (called every second)
    /// Returns `(should_complete_session, tick_payload)`.
    pub(crate) fn tick(&mut self) -> (bool, TimerTickPayload) {
        // Decrement timer
        if self.state.remaining_seconds > 0 {
            self.state.remaining_seconds -= 1;
        }

        let payload = self.state.to_tick_payload();
        let should_complete = self.state.remaining_seconds == 0;

        (should_complete, payload)
    }

    /// Transition from work session to break
    /// Returns `(break_type, session_completed_payload)`.
    pub(crate) fn transition_to_break(&mut self) -> (SessionType, SessionCompletedPayload) {
        use crate::types::SessionType;

        // Work session completed
        self.state.completed_pomodoros += 1;

        // Determine break type
        let break_type = get_break_type(
            self.state.current_cycle,
            self.state.settings.long_break_interval.get(),
        );

        self.state.session_type = break_type;

        // Set break duration
        self.state.remaining_seconds = match break_type {
            SessionType::ShortBreak => self.state.settings.short_break_duration_secs.get(),
            SessionType::LongBreak => self.state.settings.long_break_duration_secs.get(),
            SessionType::Work => unreachable!(),
        };

        // Transition to break
        self.state.status = TimerStatus::OnBreak;
        self.state.post_break_decision_pending = false;
        self.state.break_attendance = BreakAttendance::AtDesk;
        self.state.break_interaction_count = 0;
        self.state.last_transition = Some(Instant::now());

        let payload = SessionCompletedPayload::from_transition_outcome(
            &SessionTransitionOutcome::work_to_break(break_type),
        );
        self.log_transition_snapshot("transition_to_break", payload.transition_reason);

        (break_type, payload)
    }

    /// Complete a work-session boundary transition.
    ///
    /// Scheduled stop has higher priority than normal work->break transition.
    pub(crate) fn complete_work_session_transition(&mut self) -> SessionCompletedPayload {
        if self.state.schedule_stop_state == ScheduleStopState::StopAtNextSessionBoundary {
            return self.execute_scheduled_stop_transition(SessionType::Work);
        }

        let (_, payload) = self.transition_to_break();
        payload
    }

    /// Complete a break-session boundary transition using the latest policy in state.
    ///
    /// Scheduled stop has higher priority than post-break policy.
    pub(crate) fn complete_break_session_transition(&mut self) -> SessionCompletedPayload {
        let completed_session_type = self.state.session_type;

        if self.state.schedule_stop_state == ScheduleStopState::StopAtNextSessionBoundary {
            return self.execute_scheduled_stop_transition(completed_session_type);
        }

        if self.state.break_attendance.is_away() {
            return self.transition_to_idle_after_break();
        }

        match self.state.settings.post_break_transition_policy {
            PostBreakTransitionPolicy::RequireManualContinue => {
                self.transition_to_idle_after_break()
            }
            PostBreakTransitionPolicy::StartNextWorkAutomatically => {
                self.transition_break_to_work_automatically(completed_session_type)
            }
        }
    }

    /// Transition from break session to idle
    /// Returns `session_completed_payload`.
    pub(crate) fn transition_to_idle_after_break(&mut self) -> SessionCompletedPayload {
        let is_long_break = self.state.session_type == SessionType::LongBreak;
        let completed_type = self.state.session_type;

        // Break finished. Enter manual decision waiting state with work session preview.
        self.state.status = TimerStatus::Idle;
        self.state.session_type = SessionType::Work;
        self.state.remaining_seconds = self.state.settings.work_duration_secs.get();
        self.state.post_break_decision_pending = true;
        self.state.is_fullscreen_break = false;
        self.state.break_attendance = BreakAttendance::AtDesk;
        self.state.schedule_stop_state = ScheduleStopState::NotControlledBySchedule;
        self.state.last_transition = Some(Instant::now());

        // If long break completed, reset cycle
        if is_long_break {
            self.state.current_cycle = 0;
        }

        // Note: caller must cancel and await the old task outside the lock.

        let payload = SessionCompletedPayload::from_transition_outcome(
            &SessionTransitionOutcome::break_waiting_decision(completed_type),
        );
        self.log_transition_snapshot("transition_to_idle_after_break", payload.transition_reason);
        payload
    }

    fn transition_break_to_work_automatically(
        &mut self,
        completed_session_type: SessionType,
    ) -> SessionCompletedPayload {
        if completed_session_type == SessionType::LongBreak {
            self.state.current_cycle = 0;
        }
        self.state.current_cycle += 1;

        // Keep schedule control across automatic break-to-work transitions.
        self.state.status = TimerStatus::Working;
        self.state.session_type = SessionType::Work;
        self.state.remaining_seconds = self.state.settings.work_duration_secs.get();
        self.state.post_break_decision_pending = false;
        self.state.is_fullscreen_break = false;
        self.state.break_attendance = BreakAttendance::AtDesk;
        self.state.last_transition = Some(Instant::now());

        let payload = SessionCompletedPayload::from_transition_outcome(
            &SessionTransitionOutcome::break_auto_started(completed_session_type),
        );
        self.log_transition_snapshot(
            "transition_break_to_work_automatically",
            payload.transition_reason,
        );
        payload
    }

    fn execute_scheduled_stop_transition(
        &mut self,
        completed_session_type: SessionType,
    ) -> SessionCompletedPayload {
        if completed_session_type == SessionType::Work {
            self.state.completed_pomodoros += 1;
        }

        if completed_session_type == SessionType::LongBreak {
            self.state.current_cycle = 0;
        }

        self.state.status = TimerStatus::Idle;
        self.state.session_type = SessionType::Work;
        self.state.remaining_seconds = self.state.settings.work_duration_secs.get();
        self.state.post_break_decision_pending = false;
        self.state.is_fullscreen_break = false;
        self.state.break_attendance = BreakAttendance::AtDesk;
        self.state.schedule_stop_state = ScheduleStopState::NotControlledBySchedule;
        self.state.last_transition = Some(Instant::now());

        let payload = SessionCompletedPayload::from_transition_outcome(
            &SessionTransitionOutcome::scheduled_stop_executed(completed_session_type),
        );
        self.log_transition_snapshot(
            "execute_scheduled_stop_transition",
            payload.transition_reason,
        );
        payload
    }

    fn log_transition_snapshot(
        &self,
        transition_stage: &'static str,
        transition_reason: SessionTransitionReason,
    ) {
        log_info_event(
            BackendLogEvent::TimerTransitionCompleted,
            &[
                ("transition_stage", transition_stage.to_string()),
                ("transition_reason", format!("{transition_reason:?}")),
                (
                    "policy",
                    format!("{:?}", self.state.settings.post_break_transition_policy),
                ),
                (
                    "post_break_decision_pending",
                    self.state.post_break_decision_pending.to_string(),
                ),
                (
                    "schedule_stop_state",
                    format!("{:?}", self.state.schedule_stop_state),
                ),
            ],
        );
    }

    fn log_manual_decision_snapshot(&self, action: PostBreakDecisionAction) {
        log_info_event(
            BackendLogEvent::TimerTransitionCompleted,
            &[
                (
                    "transition_stage",
                    "resolve_post_break_decision".to_string(),
                ),
                (
                    "transition_reason",
                    "manual_post_break_decision".to_string(),
                ),
                ("action", format!("{action:?}")),
                (
                    "policy",
                    format!("{:?}", self.state.settings.post_break_transition_policy),
                ),
                (
                    "post_break_decision_pending",
                    self.state.post_break_decision_pending.to_string(),
                ),
                (
                    "schedule_stop_state",
                    format!("{:?}", self.state.schedule_stop_state),
                ),
            ],
        );
    }

    /// Returns the remaining rate-limit duration in milliseconds, or None if not rate-limited.
    fn rate_limit_remaining_ms(&self) -> Option<u64> {
        if let Some(t) = self.state.last_transition {
            let elapsed = t.elapsed();
            let limit = Duration::from_millis(300);
            if elapsed < limit {
                let remaining = limit.checked_sub(elapsed)?;
                return u64::try_from(remaining.as_millis()).ok();
            }
        }
        None
    }

    fn ensure_idle_for_duration_setting_sync(&self, action_name: &str) -> Result<()> {
        if self.state.status != TimerStatus::Idle {
            return Err(PomoliceError::InvalidStateTransition {
                action: action_name.to_string(),
                current_state: self.state.status,
            });
        }

        Ok(())
    }

    fn apply_auto_run_settings_update(
        &mut self,
        auto_run_feature_enabled: bool,
        weekly_auto_run_config: WeeklyAutoRunConfig,
        post_break_transition_policy: PostBreakTransitionPolicy,
    ) {
        let auto_run_boundary_scope = AutoRunBoundaryScope {
            auto_run_feature_enabled,
            weekly_auto_run_config: weekly_auto_run_config.clone(),
        };
        self.state.auto_run_feature_enabled = auto_run_feature_enabled;
        self.state.weekly_auto_run_config = weekly_auto_run_config;
        self.state.settings.post_break_transition_policy = post_break_transition_policy;
        self.auto_run_boundary_publication
            .publish_if_scope_changed(auto_run_boundary_scope);

        log_info_event(
            BackendLogEvent::SettingsSyncApplied,
            &[("timer_status", format!("{:?}", self.state.status))],
        );

        if !self.state.auto_run_feature_enabled {
            self.state.schedule_stop_state = ScheduleStopState::NotControlledBySchedule;
        }
    }

    pub(crate) fn apply_schedule_end_boundary_to_current_session(
        &mut self,
    ) -> ScheduleEndBoundaryResult {
        if !matches!(
            self.state.status,
            TimerStatus::Working | TimerStatus::OnBreak
        ) {
            return ScheduleEndBoundaryResult::Ignored;
        }

        if !self.state.auto_run_feature_enabled {
            return ScheduleEndBoundaryResult::Ignored;
        }

        if self.state.schedule_stop_state != ScheduleStopState::WaitingForScheduleEnd {
            return ScheduleEndBoundaryResult::Ignored;
        }

        self.state.schedule_stop_state = ScheduleStopState::StopAtNextSessionBoundary;
        ScheduleEndBoundaryResult::StopScheduledAtSessionBoundary
    }

    /// Cancel the running timer task and extract its `JoinHandle`.
    ///
    /// This is intentionally split into two steps so the caller can release
    /// the Mutex lock before awaiting the handle, avoiding a deadlock:
    ///
    /// ```text
    /// // Inside the Mutex lock:
    /// let old_handle = timer.cancel_and_take_task();
    /// // Release the lock here (end of scope / drop)
    ///
    /// // Outside the Mutex lock:
    /// if let Some(h) = old_handle { h.await; }
    /// ```
    ///
    /// The cancelled token causes the task's `select!` to resolve immediately,
    /// so the await completes quickly without dispatching another tick.
    pub(crate) fn cancel_and_take_task(&mut self) -> Option<tokio::task::JoinHandle<()>> {
        if let Some(token) = self.cancel_token.take() {
            token.cancel();
        }
        self.timer_task.take()
    }

    /// Spawn a new timer task.
    ///
    /// Must be called **after** the old task has been awaited (outside the lock).
    /// See `cancel_and_take_task` for the required call sequence.
    pub(crate) fn spawn_timer_task<F, Fut>(&mut self, on_tick: F)
    where
        F: Fn() -> Fut + Send + 'static,
        Fut: std::future::Future<Output = ()> + Send + 'static,
    {
        let token = CancellationToken::new();
        self.cancel_token = Some(token.clone());

        let handle = tokio::task::spawn(async move {
            loop {
                tokio::select! {
                    // Cancel wins immediately with no side-effect
                    () = token.cancelled() => break,
                    // Sleep for one second, then dispatch a tick
                    () = tokio::time::sleep(Duration::from_secs(1)) => {
                        // Re-check cancellation after waking so we never dispatch
                        // a tick that races with a newly started session
                        if token.is_cancelled() {
                            break;
                        }
                        on_tick().await;
                    }
                }
            }
        });

        self.timer_task = Some(handle);
    }
}

impl Drop for TimerStateManager {
    fn drop(&mut self) {
        // Cancel the token so the task exits on its next select! poll.
        // We cannot await the JoinHandle here (Drop is sync), but the task
        // will observe the cancellation and terminate on its own.
        if let Some(token) = self.cancel_token.take() {
            token.cancel();
        }
    }
}

/// Determine break type based on current cycle
fn get_break_type(cycle: u32, interval: u32) -> SessionType {
    if cycle > 0 && cycle.is_multiple_of(interval) {
        SessionType::LongBreak
    } else {
        SessionType::ShortBreak
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::DayAutoRunSlot;

    fn make_manager() -> TimerStateManager {
        TimerStateManager::new(None)
    }

    fn make_auto_run_enabled_manager() -> TimerStateManager {
        TimerStateManager::new(Some(TimerState {
            auto_run_feature_enabled: true,
            ..TimerState::default()
        }))
    }

    fn work_duration_secs(value: u64) -> WorkDurationSecs {
        WorkDurationSecs::try_new(value).expect("work duration should be valid")
    }

    fn short_break_duration_secs(value: u64) -> ShortBreakDurationSecs {
        ShortBreakDurationSecs::try_new(value).expect("short break duration should be valid")
    }

    fn long_break_duration_secs(value: u64) -> LongBreakDurationSecs {
        LongBreakDurationSecs::try_new(value).expect("long break duration should be valid")
    }

    fn long_break_interval(value: u32) -> LongBreakIntervalCount {
        LongBreakIntervalCount::try_new(value).expect("long break interval should be valid")
    }

    fn make_settings(
        work_duration_secs: u64,
        short_break_duration_secs: u64,
        long_break_duration_secs: u64,
        long_break_interval: u32,
    ) -> TimerSettings {
        TimerSettings::try_from_raw(
            work_duration_secs,
            short_break_duration_secs,
            long_break_duration_secs,
            long_break_interval,
            PostBreakTransitionPolicy::RequireManualContinue,
        )
        .expect("timer settings should be valid")
    }

    fn make_auto_start_settings(
        work_duration_secs: u64,
        short_break_duration_secs: u64,
        long_break_duration_secs: u64,
        long_break_interval: u32,
    ) -> TimerSettings {
        TimerSettings::try_from_raw(
            work_duration_secs,
            short_break_duration_secs,
            long_break_duration_secs,
            long_break_interval,
            PostBreakTransitionPolicy::StartNextWorkAutomatically,
        )
        .expect("timer settings should be valid")
    }

    fn enabled_slot(start_time: &str, end_time: &str) -> crate::types::DayAutoRunSlot {
        crate::types::DayAutoRunSlot::try_new(
            true,
            crate::types::HhMm::try_from(start_time.to_string())
                .expect("start time should be valid"),
            crate::types::HhMm::try_from(end_time.to_string()).expect("end time should be valid"),
        )
        .expect("slot should be valid")
    }

    #[test]
    fn test_get_break_type() {
        assert_eq!(get_break_type(1, 4), SessionType::ShortBreak);
        assert_eq!(get_break_type(2, 4), SessionType::ShortBreak);
        assert_eq!(get_break_type(3, 4), SessionType::ShortBreak);
        assert_eq!(get_break_type(4, 4), SessionType::LongBreak);
        assert_eq!(get_break_type(5, 4), SessionType::ShortBreak);
        assert_eq!(get_break_type(8, 4), SessionType::LongBreak);
    }

    #[test]
    fn test_get_break_type_different_intervals() {
        // Test with interval = 2
        assert_eq!(get_break_type(1, 2), SessionType::ShortBreak);
        assert_eq!(get_break_type(2, 2), SessionType::LongBreak);
        assert_eq!(get_break_type(3, 2), SessionType::ShortBreak);
        assert_eq!(get_break_type(4, 2), SessionType::LongBreak);

        // Test with interval = 3
        assert_eq!(get_break_type(1, 3), SessionType::ShortBreak);
        assert_eq!(get_break_type(2, 3), SessionType::ShortBreak);
        assert_eq!(get_break_type(3, 3), SessionType::LongBreak);
        assert_eq!(get_break_type(6, 3), SessionType::LongBreak);
    }

    #[test]
    fn test_timer_state_initialization() {
        let state = TimerState::default();
        assert_eq!(state.status, TimerStatus::Idle);
        assert_eq!(state.session_type, SessionType::Work);
        assert_eq!(state.current_cycle, 0);
        assert_eq!(state.completed_pomodoros, 0);
        assert_eq!(state.break_interaction_count, 0);
        assert!(!state.post_break_decision_pending);
        assert!(!state.is_fullscreen_break);
    }

    #[test]
    fn test_timer_state_get_break_duration() {
        let mut state = TimerState::default();
        state.settings.short_break_duration_secs = short_break_duration_secs(300);
        state.settings.long_break_duration_secs = long_break_duration_secs(900);

        state.session_type = SessionType::ShortBreak;
        assert_eq!(state.get_break_duration(), 300);

        state.session_type = SessionType::LongBreak;
        assert_eq!(state.get_break_duration(), 900);

        state.session_type = SessionType::Work;
        assert_eq!(state.get_break_duration(), 0); // Work session returns 0
    }

    #[test]
    fn test_timer_state_to_tick_payload() {
        let state = TimerState {
            status: TimerStatus::Working,
            remaining_seconds: 1500,
            session_type: SessionType::Work,
            current_cycle: 2,
            ..TimerState::default()
        };

        let payload = state.to_tick_payload();
        assert_eq!(payload.status, TimerStatus::Working);
        assert_eq!(payload.remaining_seconds, 1500);
        assert_eq!(payload.session_type, SessionType::Work);
        assert_eq!(payload.current_cycle, 2);
    }

    #[test]
    fn test_timer_state_to_full_payload() {
        let state = TimerState {
            status: TimerStatus::OnBreak,
            remaining_seconds: 300,
            session_type: SessionType::ShortBreak,
            current_cycle: 1,
            completed_pomodoros: 1,
            post_break_decision_pending: false,
            ..TimerState::default()
        };

        let payload = state.to_full_payload();
        assert_eq!(payload.status, TimerStatus::OnBreak);
        assert_eq!(payload.remaining_seconds, 300);
        assert_eq!(payload.session_type, SessionType::ShortBreak);
        assert_eq!(payload.current_cycle, 1);
        assert_eq!(payload.completed_pomodoros, 1);
        assert!(!payload.post_break_decision_pending);
    }

    #[test]
    fn test_start_work_transitions_to_working() {
        let mut mgr = make_manager();
        let settings = make_settings(1500, 300, 900, 4);

        let (payload, duration) = mgr
            .start_work(settings, WorkStartOrigin::ManualOutsideScheduleWindow)
            .unwrap();

        assert_eq!(mgr.status(), TimerStatus::Working);
        assert_eq!(duration, 1500);
        assert_eq!(payload.remaining_seconds, 1500);
        assert_eq!(payload.status, TimerStatus::Working);
    }

    #[test]
    fn test_manual_inside_window_start_stops_at_schedule_end_boundary() {
        let mut mgr = make_auto_run_enabled_manager();
        let settings = make_settings(1500, 300, 900, 4);

        let _ = mgr
            .start_work(settings, WorkStartOrigin::ManualInsideScheduleWindow)
            .unwrap();

        assert_eq!(
            mgr.apply_schedule_end_boundary_to_current_session(),
            ScheduleEndBoundaryResult::StopScheduledAtSessionBoundary
        );
        let payload = mgr.complete_work_session_transition();
        assert_eq!(
            payload.transition_reason,
            crate::types::SessionTransitionReason::ScheduledStopExecuted
        );
        assert_eq!(payload.next_status, TimerStatus::Idle);
    }

    #[test]
    fn test_manual_outside_window_start_ignores_schedule_end_boundary() {
        let mut mgr = make_auto_run_enabled_manager();
        let settings = make_settings(1500, 300, 900, 4);

        let _ = mgr
            .start_work(settings, WorkStartOrigin::ManualOutsideScheduleWindow)
            .unwrap();

        assert_eq!(
            mgr.apply_schedule_end_boundary_to_current_session(),
            ScheduleEndBoundaryResult::Ignored
        );
        let payload = mgr.complete_work_session_transition();
        assert_eq!(
            payload.transition_reason,
            crate::types::SessionTransitionReason::WorkToBreak
        );
        assert_eq!(payload.next_status, TimerStatus::OnBreak);
    }

    #[test]
    fn test_start_work_fails_when_not_idle() {
        let mut mgr = make_manager();
        let settings = make_settings(1500, 300, 900, 4);

        mgr.start_work(
            settings.clone(),
            WorkStartOrigin::ManualOutsideScheduleWindow,
        )
        .unwrap();

        // Second call must fail
        let err = mgr
            .start_work(settings, WorkStartOrigin::ManualOutsideScheduleWindow)
            .unwrap_err();
        assert!(matches!(err, PomoliceError::InvalidStateTransition { .. }));
    }

    #[test]
    fn test_start_work_rejects_on_break_for_all_start_sources() {
        for start_source in [
            WorkStartOrigin::ManualOutsideScheduleWindow,
            WorkStartOrigin::ManualInsideScheduleWindow,
            WorkStartOrigin::ScheduleStartBoundary,
        ] {
            let mut mgr = make_manager();
            let settings = make_settings(1500, 300, 900, 4);

            mgr.start_work(
                settings.clone(),
                WorkStartOrigin::ManualOutsideScheduleWindow,
            )
            .expect("initial work session should start");
            mgr.transition_to_break();
            let before = mgr.get_full_state();

            let err = mgr
                .start_work(settings, start_source)
                .expect_err("work must not start during break");

            assert!(matches!(
                err,
                PomoliceError::InvalidStateTransition {
                    current_state: TimerStatus::OnBreak,
                    ..
                }
            ));

            let after = mgr.get_full_state();
            assert_eq!(after.status, before.status);
            assert_eq!(after.remaining_seconds, before.remaining_seconds);
            assert_eq!(after.session_type, before.session_type);
            assert_eq!(after.current_cycle, before.current_cycle);
            assert_eq!(after.completed_pomodoros, before.completed_pomodoros);
            assert_eq!(
                after.break_interaction_count,
                before.break_interaction_count
            );
            assert_eq!(after.is_fullscreen_break, before.is_fullscreen_break);
            assert_eq!(
                after.post_break_decision_pending,
                before.post_break_decision_pending
            );
        }
    }

    #[test]
    fn test_tick_decrements_remaining_seconds() {
        let mut mgr = make_manager();
        let settings = make_settings(5, 300, 900, 4);
        mgr.start_work(settings, WorkStartOrigin::ManualOutsideScheduleWindow)
            .unwrap();

        let (should_complete, payload) = mgr.tick();
        assert!(!should_complete);
        assert_eq!(payload.remaining_seconds, 4);
    }

    #[test]
    fn test_tick_signals_completion_at_zero() {
        let mut mgr = make_manager();
        let settings = make_settings(1, 300, 900, 4);
        mgr.start_work(settings, WorkStartOrigin::ManualOutsideScheduleWindow)
            .unwrap();

        let (should_complete, _) = mgr.tick();
        assert!(should_complete);
    }

    #[test]
    fn test_tick_keeps_completed_work_session_payload_at_zero() {
        let mut mgr = make_manager();
        let settings = make_settings(1, 300, 900, 4);
        mgr.start_work(settings, WorkStartOrigin::ManualOutsideScheduleWindow)
            .unwrap();

        let (should_complete, payload) = mgr.tick();
        assert!(should_complete);
        assert_eq!(payload.remaining_seconds, 0);

        let (should_complete_after_extra_tick, payload_after_extra_tick) = mgr.tick();
        assert!(should_complete_after_extra_tick);
        assert_eq!(payload_after_extra_tick.remaining_seconds, 0);
        assert_eq!(payload_after_extra_tick.status, TimerStatus::Working);
        assert_eq!(payload_after_extra_tick.session_type, SessionType::Work);
    }

    #[test]
    fn test_tick_keeps_completed_break_session_payload_at_zero() {
        let mut mgr = make_manager();
        let settings = make_settings(1, 1, 900, 4);
        mgr.start_work(settings, WorkStartOrigin::ManualOutsideScheduleWindow)
            .unwrap();
        mgr.transition_to_break();

        let (should_complete, payload) = mgr.tick();
        assert!(should_complete);
        assert_eq!(payload.remaining_seconds, 0);

        let (should_complete_after_extra_tick, payload_after_extra_tick) = mgr.tick();
        assert!(should_complete_after_extra_tick);
        assert_eq!(payload_after_extra_tick.remaining_seconds, 0);
        assert_eq!(payload_after_extra_tick.status, TimerStatus::OnBreak);
        assert_eq!(
            payload_after_extra_tick.session_type,
            SessionType::ShortBreak
        );
    }

    #[test]
    fn test_transition_to_break_emits_work_to_break_contract_fields() {
        let mut mgr = make_manager();
        let settings = make_settings(1500, 300, 900, 4);
        mgr.start_work(settings, WorkStartOrigin::ManualOutsideScheduleWindow)
            .unwrap();

        let (break_type, payload) = mgr.transition_to_break();

        assert!(matches!(
            break_type,
            SessionType::ShortBreak | SessionType::LongBreak
        ));
        assert_eq!(
            payload.transition_reason,
            crate::types::SessionTransitionReason::WorkToBreak
        );
        assert_eq!(payload.completed_session_type, SessionType::Work);
        assert_eq!(payload.next_status, TimerStatus::OnBreak);
        assert_eq!(payload.next_session_type, break_type);
        assert!(!payload.requires_manual_continue_decision);
        assert!(!payload.post_break_decision_pending);
    }

    #[test]
    fn test_transition_to_idle_after_break_sets_work_preview_and_waiting_decision_contract() {
        let mut mgr = make_manager();
        let expected_work_preview_secs = 1800;
        let settings = make_settings(expected_work_preview_secs, 300, 900, 4);
        mgr.start_work(settings, WorkStartOrigin::ManualOutsideScheduleWindow)
            .unwrap();
        mgr.transition_to_break();
        mgr.set_fullscreen_break(true);

        let payload = mgr.transition_to_idle_after_break();

        assert_eq!(
            payload.transition_reason,
            crate::types::SessionTransitionReason::BreakWaitingDecision
        );
        assert_eq!(payload.next_status, TimerStatus::Idle);
        assert_eq!(payload.next_session_type, SessionType::Work);
        assert!(payload.requires_manual_continue_decision);
        assert!(payload.post_break_decision_pending);
        let full_state = mgr.get_full_state();
        assert_eq!(full_state.status, TimerStatus::Idle);
        assert_eq!(full_state.session_type, SessionType::Work);
        assert_eq!(full_state.remaining_seconds, expected_work_preview_secs);
        assert!(full_state.post_break_decision_pending);
        assert!(!full_state.is_fullscreen_break);
    }

    #[test]
    fn test_complete_work_session_transition_prioritizes_scheduled_stop() {
        let mut mgr = TimerStateManager::new(Some(TimerState {
            status: TimerStatus::Working,
            session_type: SessionType::Work,
            completed_pomodoros: 3,
            current_cycle: 2,
            schedule_stop_state: ScheduleStopState::StopAtNextSessionBoundary,
            ..TimerState::default()
        }));

        let payload = mgr.complete_work_session_transition();

        assert_eq!(
            payload.transition_reason,
            crate::types::SessionTransitionReason::ScheduledStopExecuted
        );
        assert_eq!(payload.completed_session_type, SessionType::Work);
        assert_eq!(payload.next_status, TimerStatus::Idle);
        assert_eq!(payload.next_session_type, SessionType::Work);
        assert!(!payload.requires_manual_continue_decision);
        let full_state = mgr.get_full_state();
        assert_eq!(full_state.status, TimerStatus::Idle);
        assert_eq!(full_state.completed_pomodoros, 4);
        assert_eq!(
            mgr.get_state().schedule_stop_state,
            ScheduleStopState::NotControlledBySchedule
        );
    }

    #[test]
    fn test_complete_break_session_transition_waits_for_manual_decision_when_policy_requires() {
        let expected_work_preview_secs = 1800;
        let mut mgr = TimerStateManager::new(Some(TimerState {
            status: TimerStatus::OnBreak,
            session_type: SessionType::ShortBreak,
            current_cycle: 2,
            settings: make_settings(expected_work_preview_secs, 300, 900, 4),
            ..TimerState::default()
        }));

        let payload = mgr.complete_break_session_transition();

        assert_eq!(
            payload.transition_reason,
            crate::types::SessionTransitionReason::BreakWaitingDecision
        );
        assert_eq!(payload.next_status, TimerStatus::Idle);
        assert_eq!(payload.next_session_type, SessionType::Work);
        assert!(payload.requires_manual_continue_decision);
        let full_state = mgr.get_full_state();
        assert_eq!(full_state.status, TimerStatus::Idle);
        assert_eq!(full_state.session_type, SessionType::Work);
        assert_eq!(full_state.remaining_seconds, expected_work_preview_secs);
        assert!(full_state.post_break_decision_pending);
    }

    #[test]
    fn test_complete_break_session_transition_auto_starts_work_and_resets_long_break_cycle() {
        let expected_work_preview_secs = 1800;
        let mut mgr = TimerStateManager::new(Some(TimerState {
            status: TimerStatus::OnBreak,
            session_type: SessionType::LongBreak,
            current_cycle: 4,
            settings: make_auto_start_settings(expected_work_preview_secs, 300, 900, 4),
            post_break_decision_pending: true,
            is_fullscreen_break: true,
            ..TimerState::default()
        }));

        let payload = mgr.complete_break_session_transition();

        assert_eq!(
            payload.transition_reason,
            crate::types::SessionTransitionReason::BreakAutoStarted
        );
        assert_eq!(payload.completed_session_type, SessionType::LongBreak);
        assert_eq!(payload.next_status, TimerStatus::Working);
        assert_eq!(payload.next_session_type, SessionType::Work);
        assert!(!payload.requires_manual_continue_decision);
        let full_state = mgr.get_full_state();
        assert_eq!(full_state.status, TimerStatus::Working);
        assert_eq!(full_state.session_type, SessionType::Work);
        assert_eq!(full_state.remaining_seconds, expected_work_preview_secs);
        assert_eq!(full_state.current_cycle, 1);
        assert!(!full_state.post_break_decision_pending);
        assert!(!full_state.is_fullscreen_break);
    }

    #[test]
    fn test_break_auto_start_remains_controlled_by_schedule_end_boundary() {
        let mut mgr = TimerStateManager::new(Some(TimerState {
            status: TimerStatus::OnBreak,
            session_type: SessionType::ShortBreak,
            auto_run_feature_enabled: true,
            schedule_stop_state: ScheduleStopState::WaitingForScheduleEnd,
            settings: make_auto_start_settings(1500, 300, 900, 4),
            ..TimerState::default()
        }));

        let payload = mgr.complete_break_session_transition();

        assert_eq!(
            payload.transition_reason,
            crate::types::SessionTransitionReason::BreakAutoStarted
        );
        assert_eq!(
            mgr.apply_schedule_end_boundary_to_current_session(),
            ScheduleEndBoundaryResult::StopScheduledAtSessionBoundary
        );
        let payload = mgr.complete_work_session_transition();
        assert_eq!(
            payload.transition_reason,
            crate::types::SessionTransitionReason::ScheduledStopExecuted
        );
    }

    #[test]
    fn test_break_auto_start_progresses_on_next_work_tick() {
        let work_duration_secs = 1800;
        let mut mgr = TimerStateManager::new(Some(TimerState {
            status: TimerStatus::OnBreak,
            session_type: SessionType::ShortBreak,
            current_cycle: 2,
            completed_pomodoros: 5,
            remaining_seconds: 0,
            settings: make_auto_start_settings(work_duration_secs, 300, 900, 4),
            post_break_decision_pending: true,
            is_fullscreen_break: true,
            ..TimerState::default()
        }));

        let payload = mgr.complete_break_session_transition();

        assert_eq!(
            payload.transition_reason,
            crate::types::SessionTransitionReason::BreakAutoStarted
        );
        assert_eq!(payload.completed_session_type, SessionType::ShortBreak);
        assert_eq!(payload.next_status, TimerStatus::Working);
        assert_eq!(payload.next_session_type, SessionType::Work);
        assert!(!payload.requires_manual_continue_decision);
        let full_state = mgr.get_full_state();
        assert_eq!(full_state.status, TimerStatus::Working);
        assert_eq!(full_state.session_type, SessionType::Work);
        assert_eq!(full_state.remaining_seconds, work_duration_secs);
        assert_eq!(full_state.current_cycle, 3);
        assert_eq!(full_state.completed_pomodoros, 5);
        assert!(!full_state.post_break_decision_pending);
        assert!(!full_state.is_fullscreen_break);

        let (should_complete, tick_payload) = mgr.tick();

        assert!(!should_complete);
        assert_eq!(tick_payload.status, TimerStatus::Working);
        assert_eq!(tick_payload.session_type, SessionType::Work);
        assert_eq!(tick_payload.remaining_seconds, work_duration_secs - 1);
        assert_eq!(
            mgr.get_full_state().remaining_seconds,
            work_duration_secs - 1
        );
    }

    #[test]
    fn test_complete_break_session_transition_prioritizes_scheduled_stop_over_auto_policy() {
        let mut mgr = TimerStateManager::new(Some(TimerState {
            status: TimerStatus::OnBreak,
            session_type: SessionType::ShortBreak,
            settings: make_auto_start_settings(1500, 300, 900, 4),
            schedule_stop_state: ScheduleStopState::StopAtNextSessionBoundary,
            ..TimerState::default()
        }));

        let payload = mgr.complete_break_session_transition();

        assert_eq!(
            payload.transition_reason,
            crate::types::SessionTransitionReason::ScheduledStopExecuted
        );
        assert_eq!(payload.next_status, TimerStatus::Idle);
        assert_eq!(payload.next_session_type, SessionType::Work);
        let full_state = mgr.get_full_state();
        assert_eq!(full_state.status, TimerStatus::Idle);
        assert!(!full_state.post_break_decision_pending);
        assert_eq!(
            mgr.get_state().schedule_stop_state,
            ScheduleStopState::NotControlledBySchedule
        );
    }

    #[test]
    fn test_resolve_post_break_decision_continue_starts_work() {
        let mut mgr = TimerStateManager::new(Some(TimerState {
            status: TimerStatus::Idle,
            post_break_decision_pending: true,
            current_cycle: 0,
            ..TimerState::default()
        }));

        let payload = mgr
            .resolve_post_break_decision(PostBreakDecisionAction::Continue)
            .unwrap();

        assert_eq!(payload.status, TimerStatus::Working);
        let full_state = mgr.get_full_state();
        assert_eq!(full_state.status, TimerStatus::Working);
        assert_eq!(full_state.session_type, SessionType::Work);
        assert_eq!(full_state.current_cycle, 1);
        assert!(!full_state.post_break_decision_pending);
    }

    #[test]
    fn test_resolve_post_break_decision_stop_cycle_returns_idle_preview() {
        let expected_work_preview_secs = 1800;
        let mut mgr = TimerStateManager::new(Some(TimerState {
            status: TimerStatus::Idle,
            post_break_decision_pending: true,
            current_cycle: 2,
            settings: make_settings(expected_work_preview_secs, 300, 900, 4),
            ..TimerState::default()
        }));

        let payload = mgr
            .resolve_post_break_decision(PostBreakDecisionAction::StopCycle)
            .unwrap();

        assert_eq!(payload.status, TimerStatus::Idle);
        let full_state = mgr.get_full_state();
        assert_eq!(full_state.status, TimerStatus::Idle);
        assert_eq!(full_state.session_type, SessionType::Work);
        assert_eq!(full_state.current_cycle, 2);
        assert_eq!(full_state.remaining_seconds, expected_work_preview_secs);
        assert!(!full_state.post_break_decision_pending);
    }

    #[test]
    fn test_resolve_post_break_decision_rejects_when_not_pending() {
        let mut mgr = TimerStateManager::new(Some(TimerState {
            status: TimerStatus::Idle,
            post_break_decision_pending: false,
            ..TimerState::default()
        }));

        let err = mgr
            .resolve_post_break_decision(PostBreakDecisionAction::Continue)
            .unwrap_err();

        assert!(matches!(err, PomoliceError::InvalidStateTransition { .. }));
    }

    #[test]
    fn test_apply_input_penalty_skipped_when_not_on_break() {
        let mut mgr = make_manager();
        let result = mgr.apply_input_penalty();
        assert!(result.is_none());
    }

    #[test]
    fn test_apply_input_penalty_resets_break_timer() {
        // Restore a break with part of its time already spent.
        let mut mgr = TimerStateManager::new(Some(TimerState {
            status: TimerStatus::OnBreak,
            session_type: SessionType::ShortBreak,
            remaining_seconds: 100,
            settings: make_settings(1500, 300, 900, 4),
            ..TimerState::default()
        }));

        let result = mgr.apply_input_penalty();
        let payload = result.expect("input penalty should apply during break");
        // Timer reset to full short break duration
        assert_eq!(payload.remaining_seconds, 300);
        assert_eq!(payload.break_interaction_count, 1);
    }

    #[test]
    fn test_apply_input_penalty_ignores_repeated_input_before_debounce_boundary() {
        let mut mgr = make_manager();
        let settings = make_settings(1500, 300, 900, 4);
        mgr.start_work(settings, WorkStartOrigin::ManualOutsideScheduleWindow)
            .unwrap();
        mgr.transition_to_break();

        let base_time = Instant::now();
        let first_result = mgr.apply_input_penalty_at(base_time);
        let first_payload = first_result.expect("first input penalty should apply");
        assert_eq!(first_payload.break_interaction_count, 1);
        assert_eq!(first_payload.remaining_seconds, 300);

        // Let the break timer run down so an accepted penalty would be visible.
        mgr.tick();
        mgr.tick();

        let Some(just_before_debounce_boundary) =
            INPUT_PENALTY_DEBOUNCE_DURATION.checked_sub(Duration::from_millis(1))
        else {
            panic!("input penalty debounce duration should be at least 1ms");
        };
        let debounced_result =
            mgr.apply_input_penalty_at(base_time + just_before_debounce_boundary);

        assert!(debounced_result.is_none());
        let full_state = mgr.get_full_state();
        assert_eq!(full_state.break_interaction_count, 1);
        assert_eq!(full_state.remaining_seconds, 298);
    }

    #[test]
    fn test_apply_input_penalty_accepts_repeated_input_at_debounce_boundary() {
        let mut mgr = make_manager();
        let settings = make_settings(1500, 300, 900, 4);
        mgr.start_work(settings, WorkStartOrigin::ManualOutsideScheduleWindow)
            .unwrap();
        mgr.transition_to_break();

        let base_time = Instant::now();
        let first_result = mgr.apply_input_penalty_at(base_time);
        assert!(first_result.is_some());

        // Let the break timer run down so the accepted penalty reset is visible.
        mgr.tick();
        mgr.tick();

        let boundary_result =
            mgr.apply_input_penalty_at(base_time + INPUT_PENALTY_DEBOUNCE_DURATION);

        let boundary_payload =
            boundary_result.expect("input at the debounce boundary should apply a penalty");
        assert_eq!(boundary_payload.break_interaction_count, 2);
        assert_eq!(boundary_payload.remaining_seconds, 300);
    }

    #[test]
    fn test_apply_input_penalty_ignores_timestamp_before_last_penalty() {
        let mut mgr = make_manager();
        let settings = make_settings(1500, 300, 900, 4);
        mgr.start_work(settings, WorkStartOrigin::ManualOutsideScheduleWindow)
            .unwrap();
        mgr.transition_to_break();

        let earlier_time = Instant::now();
        let first_result = mgr.apply_input_penalty_at(earlier_time + Duration::from_millis(1));
        assert!(first_result.is_some());

        // Let the break timer run down so an accepted penalty would be visible.
        mgr.tick();
        mgr.tick();

        let earlier_result = mgr.apply_input_penalty_at(earlier_time);

        assert!(earlier_result.is_none());
        let full_state = mgr.get_full_state();
        assert_eq!(full_state.break_interaction_count, 1);
        assert_eq!(full_state.remaining_seconds, 298);
    }

    #[test]
    fn test_emergency_stop_resets_to_idle() {
        let mut mgr = make_manager();
        let settings = make_settings(1500, 300, 900, 4);
        mgr.start_work(settings, WorkStartOrigin::ManualOutsideScheduleWindow)
            .unwrap();

        let (was_fullscreen, payload) = mgr.emergency_stop();
        assert!(!was_fullscreen);
        assert_eq!(mgr.status(), TimerStatus::Idle);
        assert_eq!(payload.status, TimerStatus::Idle);
    }

    #[test]
    fn test_sync_timer_duration_settings_updates_duration_fields_when_idle() {
        let mut mgr = make_manager();
        let payload = mgr
            .sync_timer_duration_settings(
                work_duration_secs(1800),
                short_break_duration_secs(420),
                long_break_duration_secs(1200),
                long_break_interval(5),
            )
            .unwrap();

        assert_eq!(payload.status, TimerStatus::Idle);
        let full_state = mgr.get_full_state();
        assert_eq!(full_state.settings.work_duration_secs.get(), 1800);
        assert_eq!(full_state.settings.short_break_duration_secs.get(), 420);
        assert_eq!(full_state.settings.long_break_duration_secs.get(), 1200);
        assert_eq!(full_state.settings.long_break_interval.get(), 5);
        assert_eq!(full_state.remaining_seconds, 1800);
    }

    #[test]
    fn test_sync_timer_duration_settings_rejects_non_idle_state() {
        let mut mgr = TimerStateManager::new(Some(TimerState {
            status: TimerStatus::Working,
            ..TimerState::default()
        }));

        let err = mgr
            .sync_timer_duration_settings(
                work_duration_secs(1800),
                short_break_duration_secs(420),
                long_break_duration_secs(1200),
                long_break_interval(5),
            )
            .unwrap_err();

        assert!(matches!(err, PomoliceError::InvalidStateTransition { .. }));
    }

    #[test]
    fn test_sync_automation_policies_keeps_pending_stop_state_when_feature_remains_enabled() {
        let mut mgr = make_auto_run_enabled_manager();
        let settings = make_settings(1500, 300, 900, 4);
        mgr.start_work(settings, WorkStartOrigin::ManualInsideScheduleWindow)
            .expect("work session should start");
        assert_eq!(
            mgr.apply_schedule_end_boundary_to_current_session(),
            ScheduleEndBoundaryResult::StopScheduledAtSessionBoundary
        );

        let weekly_auto_run_config = WeeklyAutoRunConfig::try_new(
            DayAutoRunSlot::default(),
            enabled_slot("10:00", "19:00"),
            DayAutoRunSlot::default(),
            DayAutoRunSlot::default(),
            DayAutoRunSlot::default(),
            DayAutoRunSlot::default(),
            DayAutoRunSlot::default(),
        )
        .expect("weekly config should be valid");

        let payload = mgr.sync_automation_policies(
            true,
            weekly_auto_run_config.clone(),
            PostBreakTransitionPolicy::RequireManualContinue,
        );

        assert_eq!(payload.status, TimerStatus::Working);
        assert!(mgr.get_state().auto_run_feature_enabled);
        assert_eq!(
            mgr.get_state().weekly_auto_run_config,
            weekly_auto_run_config
        );
        let completion = mgr.complete_work_session_transition();
        assert_eq!(
            completion.transition_reason,
            crate::types::SessionTransitionReason::ScheduledStopExecuted
        );
    }

    #[test]
    fn test_sync_automation_policies_clears_pending_stop_state_when_feature_is_disabled() {
        let mut mgr = make_auto_run_enabled_manager();
        let settings = make_settings(1500, 300, 900, 4);
        mgr.start_work(settings, WorkStartOrigin::ManualInsideScheduleWindow)
            .expect("work session should start");
        assert_eq!(
            mgr.apply_schedule_end_boundary_to_current_session(),
            ScheduleEndBoundaryResult::StopScheduledAtSessionBoundary
        );

        let payload = mgr.sync_automation_policies(
            false,
            WeeklyAutoRunConfig::default(),
            PostBreakTransitionPolicy::RequireManualContinue,
        );

        assert_eq!(payload.status, TimerStatus::Working);
        let completion = mgr.complete_work_session_transition();
        assert_eq!(
            completion.transition_reason,
            crate::types::SessionTransitionReason::WorkToBreak
        );
    }

    #[test]
    fn test_manual_inside_window_start_accepts_schedule_end_boundary() {
        let mut mgr = make_auto_run_enabled_manager();
        let settings = make_settings(1500, 300, 900, 4);
        mgr.start_work(settings, WorkStartOrigin::ManualInsideScheduleWindow)
            .expect("work session should start");

        assert_eq!(
            mgr.apply_schedule_end_boundary_to_current_session(),
            ScheduleEndBoundaryResult::StopScheduledAtSessionBoundary
        );
        let payload = mgr.complete_work_session_transition();
        assert_eq!(
            payload.transition_reason,
            crate::types::SessionTransitionReason::ScheduledStopExecuted
        );
    }

    #[test]
    fn test_scheduled_auto_start_accepts_schedule_end_boundary() {
        let mut mgr = make_auto_run_enabled_manager();
        let settings = make_settings(1500, 300, 900, 4);
        mgr.start_work(settings, WorkStartOrigin::ScheduleStartBoundary)
            .expect("work session should start");

        assert_eq!(
            mgr.apply_schedule_end_boundary_to_current_session(),
            ScheduleEndBoundaryResult::StopScheduledAtSessionBoundary
        );
        let payload = mgr.complete_work_session_transition();
        assert_eq!(
            payload.transition_reason,
            crate::types::SessionTransitionReason::ScheduledStopExecuted
        );
    }

    #[test]
    fn test_manual_outside_window_start_ignores_schedule_end_boundary_after_start() {
        let mut mgr = make_auto_run_enabled_manager();
        let settings = make_settings(1500, 300, 900, 4);
        mgr.start_work(settings, WorkStartOrigin::ManualOutsideScheduleWindow)
            .expect("work session should start");

        assert_eq!(
            mgr.apply_schedule_end_boundary_to_current_session(),
            ScheduleEndBoundaryResult::Ignored
        );
        let payload = mgr.complete_work_session_transition();
        assert_eq!(
            payload.transition_reason,
            crate::types::SessionTransitionReason::WorkToBreak
        );
    }

    #[test]
    fn test_schedule_end_boundary_is_ignored_when_idle() {
        let mut mgr = make_auto_run_enabled_manager();

        assert_eq!(
            mgr.apply_schedule_end_boundary_to_current_session(),
            ScheduleEndBoundaryResult::Ignored
        );
    }

    #[test]
    fn test_schedule_end_boundary_is_idempotent_before_session_completion() {
        let mut mgr = make_auto_run_enabled_manager();
        let settings = make_settings(1500, 300, 900, 4);
        mgr.start_work(settings, WorkStartOrigin::ManualInsideScheduleWindow)
            .expect("work session should start");

        assert_eq!(
            mgr.apply_schedule_end_boundary_to_current_session(),
            ScheduleEndBoundaryResult::StopScheduledAtSessionBoundary
        );
        assert_eq!(
            mgr.apply_schedule_end_boundary_to_current_session(),
            ScheduleEndBoundaryResult::Ignored
        );
        let payload = mgr.complete_work_session_transition();
        assert_eq!(
            payload.transition_reason,
            crate::types::SessionTransitionReason::ScheduledStopExecuted
        );
    }

    #[test]
    fn test_schedule_end_boundary_remains_effective_after_auto_run_config_change() {
        let mut mgr = make_auto_run_enabled_manager();
        let settings = make_settings(1500, 300, 900, 4);
        mgr.start_work(settings, WorkStartOrigin::ManualInsideScheduleWindow)
            .expect("work session should start");

        let weekly_auto_run_config = WeeklyAutoRunConfig::try_new(
            DayAutoRunSlot::default(),
            DayAutoRunSlot::default(),
            DayAutoRunSlot::default(),
            DayAutoRunSlot::default(),
            enabled_slot("23:00", "02:00"),
            DayAutoRunSlot::default(),
            DayAutoRunSlot::default(),
        )
        .expect("weekly config should be valid");
        let _ = mgr.sync_automation_policies(
            true,
            weekly_auto_run_config,
            PostBreakTransitionPolicy::RequireManualContinue,
        );

        assert_eq!(
            mgr.apply_schedule_end_boundary_to_current_session(),
            ScheduleEndBoundaryResult::StopScheduledAtSessionBoundary
        );
        let payload = mgr.complete_work_session_transition();
        assert_eq!(
            payload.transition_reason,
            crate::types::SessionTransitionReason::ScheduledStopExecuted
        );
    }

    #[test]
    fn test_scheduled_stop_at_work_boundary_cleans_up_and_skips_dialog() {
        let expected_work_preview_secs = 1800;
        let mut mgr = TimerStateManager::new(Some(TimerState {
            status: TimerStatus::Working,
            session_type: SessionType::Work,
            remaining_seconds: 0,
            current_cycle: 2,
            completed_pomodoros: 3,
            schedule_stop_state: ScheduleStopState::StopAtNextSessionBoundary,
            settings: make_settings(expected_work_preview_secs, 300, 900, 4),
            ..TimerState::default()
        }));

        let payload = mgr.complete_work_session_transition();

        assert_eq!(payload.completed_session_type, SessionType::Work);
        assert_eq!(
            payload.transition_reason,
            crate::types::SessionTransitionReason::ScheduledStopExecuted
        );
        assert_eq!(payload.next_status, TimerStatus::Idle);
        assert_eq!(payload.next_session_type, SessionType::Work);
        assert!(!payload.requires_manual_continue_decision);
        assert!(!payload.post_break_decision_pending);
        let full_state = mgr.get_full_state();
        assert_eq!(full_state.status, TimerStatus::Idle);
        assert_eq!(full_state.session_type, SessionType::Work);
        assert_eq!(full_state.remaining_seconds, expected_work_preview_secs);
        assert_eq!(full_state.completed_pomodoros, 4);
        assert!(!full_state.post_break_decision_pending);
        assert_eq!(
            mgr.get_state().schedule_stop_state,
            ScheduleStopState::NotControlledBySchedule
        );
    }

    #[test]
    fn test_scheduled_stop_at_long_break_boundary_resets_cycle() {
        let mut mgr = TimerStateManager::new(Some(TimerState {
            status: TimerStatus::OnBreak,
            session_type: SessionType::LongBreak,
            current_cycle: 4,
            schedule_stop_state: ScheduleStopState::StopAtNextSessionBoundary,
            ..TimerState::default()
        }));

        let payload = mgr.complete_break_session_transition();

        assert_eq!(payload.completed_session_type, SessionType::LongBreak);
        assert_eq!(
            payload.transition_reason,
            crate::types::SessionTransitionReason::ScheduledStopExecuted
        );
        assert_eq!(payload.next_status, TimerStatus::Idle);
        assert_eq!(payload.next_session_type, SessionType::Work);
        assert!(!payload.requires_manual_continue_decision);
        assert!(!payload.post_break_decision_pending);
        let full_state = mgr.get_full_state();
        assert_eq!(full_state.status, TimerStatus::Idle);
        assert_eq!(full_state.session_type, SessionType::Work);
        assert_eq!(full_state.current_cycle, 0);
        assert_eq!(
            mgr.get_state().schedule_stop_state,
            ScheduleStopState::NotControlledBySchedule
        );
    }

    #[test]
    fn test_initialize_from_persisted_state_returns_payload_when_running() {
        let state = TimerState {
            status: TimerStatus::Working,
            remaining_seconds: 900,
            ..TimerState::default()
        };

        let mut mgr = make_manager();
        let result = mgr.apply_persisted_state(state);

        assert!(result.is_some());
        assert_eq!(result.unwrap().remaining_seconds, 900);
    }

    #[test]
    fn test_initialize_from_persisted_state_returns_none_when_idle() {
        let mut mgr = make_manager();
        let result = mgr.apply_persisted_state(TimerState::default());
        assert!(result.is_none());
    }

    #[test]
    fn test_start_work_is_not_rate_limited_without_recent_transition() {
        let mut mgr = make_manager();
        let settings = make_settings(1500, 300, 900, 4);

        let result = mgr.start_work(settings, WorkStartOrigin::ManualOutsideScheduleWindow);

        assert!(result.is_ok());
    }

    #[test]
    fn test_start_work_is_rate_limited_immediately_after_transition() {
        let mut mgr = make_manager();
        let settings = make_settings(1500, 300, 900, 4);
        let _ = mgr.emergency_stop();

        let err = mgr
            .start_work(settings, WorkStartOrigin::ManualOutsideScheduleWindow)
            .unwrap_err();

        match err {
            PomoliceError::RateLimited { remaining_ms } => {
                assert!(remaining_ms > 0, "remaining_ms should be > 0");
                assert!(
                    remaining_ms <= 300,
                    "remaining_ms should be <= 300, got {remaining_ms}"
                );
            }
            other => panic!("Expected RateLimited, got {other:?}"),
        }
    }

    #[test]
    fn test_start_work_succeeds_after_rate_limit_window_elapses() {
        let mut mgr = make_manager();
        let settings = make_settings(1500, 300, 900, 4);
        let _ = mgr.emergency_stop();
        std::thread::sleep(Duration::from_millis(301));

        let result = mgr.start_work(settings, WorkStartOrigin::ManualOutsideScheduleWindow);

        assert!(result.is_ok());
    }

    #[test]
    fn test_start_work_rate_limited_returns_accurate_remaining_ms() {
        let mut mgr = make_manager();
        let settings = make_settings(1500, 300, 900, 4);
        mgr.start_work(
            settings.clone(),
            WorkStartOrigin::ManualOutsideScheduleWindow,
        )
        .unwrap();

        // Emergency stop returns to Idle and counts as a transition itself,
        // so the rate-limit window is still open for the next manual start.
        let _ = mgr.emergency_stop();

        // Second start_work must be rate-limited.
        // remaining_ms reflects actual elapsed time: close to 300ms when called immediately,
        // so we assert it is in (0, 300] rather than strictly < 300.
        let err = mgr
            .start_work(settings, WorkStartOrigin::ManualOutsideScheduleWindow)
            .unwrap_err();
        match err {
            PomoliceError::RateLimited { remaining_ms } => {
                assert!(remaining_ms > 0, "remaining_ms should be > 0");
                assert!(
                    remaining_ms <= 300,
                    "remaining_ms should be <= 300, got {remaining_ms}"
                );
            }
            other => panic!("Expected RateLimited, got {other:?}"),
        }
    }

    #[test]
    fn test_start_work_with_scheduled_auto_start_skips_rate_limit() {
        let mut mgr = make_manager();
        let settings = make_settings(1500, 300, 900, 4);
        mgr.start_work(
            settings.clone(),
            WorkStartOrigin::ManualOutsideScheduleWindow,
        )
        .unwrap();

        // Return to idle immediately; the stop keeps the rate-limit window open.
        let _ = mgr.emergency_stop();

        let result = mgr.start_work(settings, WorkStartOrigin::ScheduleStartBoundary);
        assert!(result.is_ok());
    }

    fn start_auto_policy_break(mgr: &mut TimerStateManager) {
        mgr.start_work(
            make_auto_start_settings(1500, 300, 900, 4),
            WorkStartOrigin::ManualOutsideScheduleWindow,
        )
        .unwrap();
        mgr.transition_to_break();
        assert_eq!(mgr.status(), TimerStatus::OnBreak);
    }

    #[test]
    fn test_away_during_break_makes_break_end_wait_for_manual_decision() {
        let mut mgr = make_manager();
        start_auto_policy_break(&mut mgr);

        let payload = mgr.set_break_attendance(BreakAttendance::Away).unwrap();
        assert!(payload.away_during_break);
        assert_eq!(payload.status, TimerStatus::OnBreak);

        let completed = mgr.complete_break_session_transition();

        assert_eq!(
            completed.transition_reason,
            crate::types::SessionTransitionReason::BreakWaitingDecision
        );
        assert!(completed.requires_manual_continue_decision);
        let full_state = mgr.get_full_state();
        assert_eq!(full_state.status, TimerStatus::Idle);
        assert!(full_state.post_break_decision_pending);
        assert!(!full_state.away_during_break);
    }

    #[test]
    fn test_returning_during_break_restores_automatic_work_start() {
        let mut mgr = make_manager();
        start_auto_policy_break(&mut mgr);
        mgr.set_break_attendance(BreakAttendance::Away).unwrap();

        let payload = mgr.set_break_attendance(BreakAttendance::AtDesk).unwrap();
        assert!(!payload.away_during_break);

        let completed = mgr.complete_break_session_transition();

        assert_eq!(
            completed.transition_reason,
            crate::types::SessionTransitionReason::BreakAutoStarted
        );
        assert_eq!(mgr.status(), TimerStatus::Working);
    }

    #[test]
    fn test_away_during_break_is_rejected_outside_break() {
        let mut mgr = make_manager();
        assert!(matches!(
            mgr.set_break_attendance(BreakAttendance::Away),
            Err(PomoliceError::InvalidStateTransition { .. })
        ));

        mgr.start_work(
            make_auto_start_settings(1500, 300, 900, 4),
            WorkStartOrigin::ManualOutsideScheduleWindow,
        )
        .unwrap();
        assert!(matches!(
            mgr.set_break_attendance(BreakAttendance::Away),
            Err(PomoliceError::InvalidStateTransition { .. })
        ));
        assert!(!mgr.get_full_state().away_during_break);
    }

    #[test]
    fn test_away_flag_does_not_carry_over_to_the_next_break() {
        let mut mgr = make_manager();
        start_auto_policy_break(&mut mgr);
        mgr.set_break_attendance(BreakAttendance::Away).unwrap();
        mgr.complete_break_session_transition();
        mgr.resolve_post_break_decision(PostBreakDecisionAction::Continue)
            .unwrap();

        mgr.transition_to_break();
        assert!(!mgr.get_full_state().away_during_break);

        let completed = mgr.complete_break_session_transition();
        assert_eq!(
            completed.transition_reason,
            crate::types::SessionTransitionReason::BreakAutoStarted
        );
    }

    #[test]
    fn test_emergency_stop_clears_away_flag() {
        let mut mgr = make_manager();
        start_auto_policy_break(&mut mgr);
        mgr.set_break_attendance(BreakAttendance::Away).unwrap();

        let (_, payload) = mgr.emergency_stop();

        assert!(!payload.away_during_break);
    }

    #[test]
    fn test_scheduled_stop_still_takes_priority_over_away_flag() {
        let mut mgr = make_manager();
        start_auto_policy_break(&mut mgr);
        mgr.set_break_attendance(BreakAttendance::Away).unwrap();
        mgr.state.schedule_stop_state = ScheduleStopState::StopAtNextSessionBoundary;

        let completed = mgr.complete_break_session_transition();

        assert_eq!(
            completed.transition_reason,
            crate::types::SessionTransitionReason::ScheduledStopExecuted
        );
        assert!(!completed.requires_manual_continue_decision);
        assert!(!mgr.get_full_state().away_during_break);
    }
}
