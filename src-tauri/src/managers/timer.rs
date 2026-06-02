// Timer State Manager
// Manages timer state and session lifecycle without global statics

use std::time::{Duration, Instant};
use tokio_util::sync::CancellationToken;

use crate::error::{PomoliceError, Result};
use crate::logging_event::{log_info_event, log_warn_event, BackendLogEvent};
use crate::types::{
    FullStatePayload, LongBreakDurationSecs, LongBreakIntervalCount, PostBreakDecisionAction,
    PostBreakTransitionPolicy, ScheduledStopState, SessionCompletedPayload,
    SessionTransitionOutcome, SessionTransitionReason, SessionType, ShortBreakDurationSecs,
    TimerSettings, TimerStartSource, TimerState, TimerStatus, TimerTickPayload,
    WeeklyAutoRunConfig, WorkDurationSecs,
};

/// Manager for timer state and lifecycle
///
/// This struct holds only pure timer state and task lifecycle handles.
/// All side effects (event emission to frontend) are the caller's responsibility.
pub(crate) struct TimerStateManager {
    state: TimerState,
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

        Self {
            state,
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

    /// Start a work session with an explicit session start source.
    pub(crate) fn start_work_with_source(
        &mut self,
        settings: TimerSettings,
        timer_start_source: TimerStartSource,
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

        if timer_start_source != TimerStartSource::ScheduledAutoStart {
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
        self.state.timer_start_source = Some(timer_start_source);
        self.state.scheduled_stop_state = ScheduledStopState::None;

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
                ("timer_start_source", format!("{timer_start_source:?}")),
            ],
        );

        // Note: caller must call cancel_and_take_task() before the lock,
        // await the old handle outside the lock, then call spawn_timer_task()
        // inside a new lock. See commands/timer.rs for the call sequence.

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
        self.state.scheduled_stop_state = ScheduledStopState::None;
        self.state.last_transition = Some(Instant::now());

        self.log_manual_decision_snapshot(action);

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
        self.state.break_interaction_count = 0;
        self.state.last_penalty_time = None;
        self.state.timer_start_source = None;
        self.state.scheduled_stop_state = ScheduledStopState::None;
        self.state.last_transition = Some(Instant::now());

        // Note: caller must call cancel_and_take_task() inside the lock,
        // then await the handle outside the lock. See commands/timer.rs.

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
    /// This method is allowed in every timer state. It intentionally does not
    /// recalculate `timer_start_source` and only clears pending scheduled stop
    /// when the feature is disabled.
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
        // Only apply penalty if on break
        if self.state.status != TimerStatus::OnBreak {
            return None;
        }

        // Debounce: 100ms between penalties
        if let Some(last) = self.state.last_penalty_time {
            if last.elapsed() < Duration::from_millis(100) {
                return None;
            }
        }

        // Increment interaction count
        self.state.break_interaction_count += 1;
        self.state.last_penalty_time = Some(Instant::now());

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
        if self.state.scheduled_stop_state == ScheduledStopState::Pending {
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

        if self.state.scheduled_stop_state == ScheduledStopState::Pending {
            return self.execute_scheduled_stop_transition(completed_session_type);
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
        self.state.timer_start_source = None;
        self.state.scheduled_stop_state = ScheduledStopState::None;
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

        // Auto-start path must commit these fields together.
        // Keep timer_start_source to preserve scheduled-stop eligibility provenance.
        // scheduled_stop_state is re-armed on later end-boundary crossings.
        self.state.status = TimerStatus::Working;
        self.state.session_type = SessionType::Work;
        self.state.remaining_seconds = self.state.settings.work_duration_secs.get();
        self.state.post_break_decision_pending = false;
        self.state.is_fullscreen_break = false;
        self.state.scheduled_stop_state = ScheduledStopState::None;
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

    /// Transition to idle when a scheduled stop is pending at session boundary.
    ///
    /// This path intentionally skips the continue dialog and clears scheduled-stop
    /// metadata so later sessions start cleanly.
    #[cfg(test)]
    pub(crate) fn transition_to_idle_due_to_scheduled_stop(
        &mut self,
        completed_session_type: SessionType,
    ) -> SessionCompletedPayload {
        self.execute_scheduled_stop_transition(completed_session_type)
    }

    pub(crate) fn execute_scheduled_stop_transition(
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
        self.state.timer_start_source = None;
        self.state.scheduled_stop_state = ScheduledStopState::None;
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
                    "scheduled_stop_state",
                    format!("{:?}", self.state.scheduled_stop_state),
                ),
                (
                    "timer_start_source",
                    format!("{:?}", self.state.timer_start_source),
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
                    "scheduled_stop_state",
                    format!("{:?}", self.state.scheduled_stop_state),
                ),
                (
                    "timer_start_source",
                    format!("{:?}", self.state.timer_start_source),
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
        self.state.auto_run_feature_enabled = auto_run_feature_enabled;
        self.state.weekly_auto_run_config = weekly_auto_run_config;
        self.state.settings.post_break_transition_policy = post_break_transition_policy;

        log_info_event(
            BackendLogEvent::SettingsSyncApplied,
            &[("timer_status", format!("{:?}", self.state.status))],
        );

        if !self.state.auto_run_feature_enabled {
            self.state.scheduled_stop_state = ScheduledStopState::None;
        }
    }

    /// Marks scheduled stop as pending when the current session is eligible.
    ///
    /// Eligibility rules are fixed by start source:
    /// - `manual_outside_window` does not schedule a stop
    /// - `manual_inside_window` schedules a stop
    /// - `scheduled_auto_start` schedules a stop
    ///
    /// Returns `true` when the state changed from `none` to `pending`.
    pub(crate) fn mark_scheduled_stop_pending_if_session_is_eligible(&mut self) -> bool {
        if !matches!(
            self.state.status,
            TimerStatus::Working | TimerStatus::OnBreak
        ) {
            return false;
        }

        if !self.state.auto_run_feature_enabled {
            return false;
        }

        if self.state.scheduled_stop_state == ScheduledStopState::Pending {
            return false;
        }

        match self.state.timer_start_source {
            Some(TimerStartSource::ManualOutsideWindow) | None => false,
            Some(TimerStartSource::ManualInsideWindow | TimerStartSource::ScheduledAutoStart) => {
                self.state.scheduled_stop_state = ScheduledStopState::Pending;
                true
            }
        }
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

    #[cfg(test)]
    pub(crate) fn set_task_handles_for_test(
        &mut self,
        cancel_token: CancellationToken,
        timer_task: tokio::task::JoinHandle<()>,
    ) {
        self.cancel_token = Some(cancel_token);
        self.timer_task = Some(timer_task);
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

    fn enabled_slot(start_time: &str, end_time: &str) -> crate::types::DayAutoRunSlot {
        crate::types::DayAutoRunSlot::try_new(
            true,
            crate::types::HhMm::try_from(start_time.to_string())
                .expect("start time should be valid"),
            crate::types::HhMm::try_from(end_time.to_string()).expect("end time should be valid"),
        )
        .expect("slot should be valid")
    }

    #[tokio::test]
    async fn test_spawn_timer_task_invokes_and_awaits_callback_before_cancelled_task_finishes() {
        let mut mgr = make_manager();
        let callback_started = std::sync::Arc::new(tokio::sync::Notify::new());
        let callback_release = std::sync::Arc::new(tokio::sync::Notify::new());
        let callback_finished = std::sync::Arc::new(tokio::sync::Notify::new());

        {
            let callback_started = std::sync::Arc::clone(&callback_started);
            let callback_release = std::sync::Arc::clone(&callback_release);
            let callback_finished = std::sync::Arc::clone(&callback_finished);
            mgr.spawn_timer_task(move || {
                let callback_started = std::sync::Arc::clone(&callback_started);
                let callback_release = std::sync::Arc::clone(&callback_release);
                let callback_finished = std::sync::Arc::clone(&callback_finished);
                async move {
                    callback_started.notify_one();
                    callback_release.notified().await;
                    callback_finished.notify_one();
                }
            });
        }

        tokio::time::timeout(Duration::from_millis(1500), callback_started.notified())
            .await
            .expect("callback should start after the first tick");

        let mut handle = mgr
            .cancel_and_take_task()
            .expect("running timer task should return a handle");

        tokio::time::timeout(Duration::from_millis(100), &mut handle)
            .await
            .expect_err("cancelled task should still await the pending callback");

        callback_release.notify_one();
        tokio::time::timeout(Duration::from_millis(100), callback_finished.notified())
            .await
            .expect("callback should finish after release");
        tokio::time::timeout(Duration::from_millis(100), handle)
            .await
            .expect("task should finish after callback completion")
            .expect("timer task should not panic");
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
            .start_work_with_source(settings, TimerStartSource::ManualOutsideWindow)
            .unwrap();

        assert_eq!(mgr.status(), TimerStatus::Working);
        assert_eq!(duration, 1500);
        assert_eq!(payload.remaining_seconds, 1500);
        assert_eq!(payload.status, TimerStatus::Working);
    }

    #[test]
    fn test_start_work_with_source_sets_timer_start_source() {
        let mut mgr = make_manager();
        let settings = make_settings(1500, 300, 900, 4);

        let _ = mgr
            .start_work_with_source(settings, TimerStartSource::ManualInsideWindow)
            .unwrap();

        assert_eq!(
            mgr.state.timer_start_source,
            Some(TimerStartSource::ManualInsideWindow)
        );
    }

    #[test]
    fn test_start_work_with_source_sets_manual_outside_window_source() {
        let mut mgr = make_manager();
        let settings = make_settings(1500, 300, 900, 4);

        let _ = mgr
            .start_work_with_source(settings, TimerStartSource::ManualOutsideWindow)
            .unwrap();

        assert_eq!(
            mgr.state.timer_start_source,
            Some(TimerStartSource::ManualOutsideWindow)
        );
    }

    #[test]
    fn test_start_work_fails_when_not_idle() {
        let mut mgr = make_manager();
        let settings = make_settings(1500, 300, 900, 4);

        mgr.start_work_with_source(settings.clone(), TimerStartSource::ManualOutsideWindow)
            .unwrap();

        // Second call must fail
        let err = mgr
            .start_work_with_source(settings, TimerStartSource::ManualOutsideWindow)
            .unwrap_err();
        assert!(matches!(err, PomoliceError::InvalidStateTransition { .. }));
    }

    #[test]
    fn test_tick_decrements_remaining_seconds() {
        let mut mgr = make_manager();
        let settings = make_settings(5, 300, 900, 4);
        mgr.start_work_with_source(settings, TimerStartSource::ManualOutsideWindow)
            .unwrap();

        let (should_complete, payload) = mgr.tick();
        assert!(!should_complete);
        assert_eq!(payload.remaining_seconds, 4);
    }

    #[test]
    fn test_tick_signals_completion_at_zero() {
        let mut mgr = make_manager();
        let settings = make_settings(1, 300, 900, 4);
        mgr.start_work_with_source(settings, TimerStartSource::ManualOutsideWindow)
            .unwrap();

        let (should_complete, _) = mgr.tick();
        assert!(should_complete);
    }

    #[test]
    fn test_transition_to_break_emits_work_to_break_contract_fields() {
        let mut mgr = make_manager();
        let settings = make_settings(1500, 300, 900, 4);
        mgr.start_work_with_source(settings, TimerStartSource::ManualOutsideWindow)
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
        mgr.start_work_with_source(settings, TimerStartSource::ManualOutsideWindow)
            .unwrap();
        mgr.transition_to_break();
        mgr.state.remaining_seconds = 0;
        mgr.state.is_fullscreen_break = true;

        let payload = mgr.transition_to_idle_after_break();

        assert_eq!(
            payload.transition_reason,
            crate::types::SessionTransitionReason::BreakWaitingDecision
        );
        assert_eq!(payload.next_status, TimerStatus::Idle);
        assert_eq!(payload.next_session_type, SessionType::Work);
        assert!(payload.requires_manual_continue_decision);
        assert!(payload.post_break_decision_pending);
        assert_eq!(mgr.state.status, TimerStatus::Idle);
        assert_eq!(mgr.state.session_type, SessionType::Work);
        assert_eq!(mgr.state.remaining_seconds, expected_work_preview_secs);
        assert!(mgr.state.post_break_decision_pending);
        assert!(!mgr.state.is_fullscreen_break);
    }

    #[test]
    fn test_complete_work_session_transition_prioritizes_scheduled_stop() {
        let mut mgr = make_manager();
        mgr.state.status = TimerStatus::Working;
        mgr.state.session_type = SessionType::Work;
        mgr.state.completed_pomodoros = 3;
        mgr.state.current_cycle = 2;
        mgr.state.scheduled_stop_state = ScheduledStopState::Pending;
        mgr.state.timer_start_source = Some(TimerStartSource::ManualInsideWindow);

        let payload = mgr.complete_work_session_transition();

        assert_eq!(
            payload.transition_reason,
            crate::types::SessionTransitionReason::ScheduledStopExecuted
        );
        assert_eq!(payload.completed_session_type, SessionType::Work);
        assert_eq!(payload.next_status, TimerStatus::Idle);
        assert_eq!(payload.next_session_type, SessionType::Work);
        assert!(!payload.requires_manual_continue_decision);
        assert_eq!(mgr.state.status, TimerStatus::Idle);
        assert_eq!(mgr.state.completed_pomodoros, 4);
        assert_eq!(mgr.state.scheduled_stop_state, ScheduledStopState::None);
    }

    #[test]
    fn test_complete_break_session_transition_waits_for_manual_decision_when_policy_requires() {
        let mut mgr = make_manager();
        let expected_work_preview_secs = 1800;
        mgr.state.status = TimerStatus::OnBreak;
        mgr.state.session_type = SessionType::ShortBreak;
        mgr.state.current_cycle = 2;
        mgr.state.settings = make_settings(expected_work_preview_secs, 300, 900, 4);
        mgr.state.settings.post_break_transition_policy =
            PostBreakTransitionPolicy::RequireManualContinue;

        let payload = mgr.complete_break_session_transition();

        assert_eq!(
            payload.transition_reason,
            crate::types::SessionTransitionReason::BreakWaitingDecision
        );
        assert_eq!(payload.next_status, TimerStatus::Idle);
        assert_eq!(payload.next_session_type, SessionType::Work);
        assert!(payload.requires_manual_continue_decision);
        assert_eq!(mgr.state.status, TimerStatus::Idle);
        assert_eq!(mgr.state.session_type, SessionType::Work);
        assert_eq!(mgr.state.remaining_seconds, expected_work_preview_secs);
        assert!(mgr.state.post_break_decision_pending);
    }

    #[test]
    fn test_complete_break_session_transition_auto_starts_work_and_resets_long_break_cycle() {
        let mut mgr = make_manager();
        let expected_work_preview_secs = 1800;
        mgr.state.status = TimerStatus::OnBreak;
        mgr.state.session_type = SessionType::LongBreak;
        mgr.state.current_cycle = 4;
        mgr.state.timer_start_source = Some(TimerStartSource::ManualInsideWindow);
        mgr.state.settings = make_settings(expected_work_preview_secs, 300, 900, 4);
        mgr.state.settings.post_break_transition_policy =
            PostBreakTransitionPolicy::StartNextWorkAutomatically;
        mgr.state.post_break_decision_pending = true;
        mgr.state.is_fullscreen_break = true;

        let payload = mgr.complete_break_session_transition();

        assert_eq!(
            payload.transition_reason,
            crate::types::SessionTransitionReason::BreakAutoStarted
        );
        assert_eq!(payload.completed_session_type, SessionType::LongBreak);
        assert_eq!(payload.next_status, TimerStatus::Working);
        assert_eq!(payload.next_session_type, SessionType::Work);
        assert!(!payload.requires_manual_continue_decision);
        assert_eq!(mgr.state.status, TimerStatus::Working);
        assert_eq!(mgr.state.session_type, SessionType::Work);
        assert_eq!(mgr.state.remaining_seconds, expected_work_preview_secs);
        assert_eq!(mgr.state.current_cycle, 1);
        assert!(!mgr.state.post_break_decision_pending);
        assert!(!mgr.state.is_fullscreen_break);
        assert_eq!(
            mgr.state.timer_start_source,
            Some(TimerStartSource::ManualInsideWindow)
        );
    }

    #[test]
    fn test_break_auto_start_keeps_source_eligible_for_scheduled_stop_pending() {
        let mut mgr = make_manager();
        mgr.state.status = TimerStatus::OnBreak;
        mgr.state.session_type = SessionType::ShortBreak;
        mgr.state.auto_run_feature_enabled = true;
        mgr.state.timer_start_source = Some(TimerStartSource::ManualInsideWindow);
        mgr.state.settings.post_break_transition_policy =
            PostBreakTransitionPolicy::StartNextWorkAutomatically;

        let payload = mgr.complete_break_session_transition();

        assert_eq!(
            payload.transition_reason,
            crate::types::SessionTransitionReason::BreakAutoStarted
        );
        assert_eq!(
            mgr.state.timer_start_source,
            Some(TimerStartSource::ManualInsideWindow)
        );
        assert_eq!(mgr.state.scheduled_stop_state, ScheduledStopState::None);

        let changed = mgr.mark_scheduled_stop_pending_if_session_is_eligible();
        assert!(changed);
        assert_eq!(mgr.state.scheduled_stop_state, ScheduledStopState::Pending);
    }

    #[tokio::test]
    async fn test_complete_break_session_transition_auto_start_keeps_task_handles_untouched() {
        let mut mgr = make_manager();
        mgr.state.status = TimerStatus::OnBreak;
        mgr.state.session_type = SessionType::ShortBreak;
        mgr.state.settings.post_break_transition_policy =
            PostBreakTransitionPolicy::StartNextWorkAutomatically;
        mgr.cancel_token = Some(CancellationToken::new());
        mgr.timer_task = Some(tokio::spawn(async {
            tokio::time::sleep(Duration::from_secs(60)).await;
        }));

        let payload = mgr.complete_break_session_transition();

        assert_eq!(
            payload.transition_reason,
            crate::types::SessionTransitionReason::BreakAutoStarted
        );
        assert!(mgr.cancel_token.is_some());
        assert!(!mgr
            .cancel_token
            .as_ref()
            .expect("cancel token should remain")
            .is_cancelled());
        assert!(mgr.timer_task.is_some());

        if let Some(handle) = mgr.timer_task.take() {
            handle.abort();
            let _ = handle.await;
        }
    }

    #[test]
    fn test_complete_break_session_transition_prioritizes_scheduled_stop_over_auto_policy() {
        let mut mgr = make_manager();
        mgr.state.status = TimerStatus::OnBreak;
        mgr.state.session_type = SessionType::ShortBreak;
        mgr.state.settings.post_break_transition_policy =
            PostBreakTransitionPolicy::StartNextWorkAutomatically;
        mgr.state.scheduled_stop_state = ScheduledStopState::Pending;

        let payload = mgr.complete_break_session_transition();

        assert_eq!(
            payload.transition_reason,
            crate::types::SessionTransitionReason::ScheduledStopExecuted
        );
        assert_eq!(payload.next_status, TimerStatus::Idle);
        assert_eq!(payload.next_session_type, SessionType::Work);
        assert_eq!(mgr.state.status, TimerStatus::Idle);
        assert!(!mgr.state.post_break_decision_pending);
        assert_eq!(mgr.state.scheduled_stop_state, ScheduledStopState::None);
    }

    #[test]
    fn test_resolve_post_break_decision_continue_starts_work() {
        let mut mgr = make_manager();
        mgr.state.status = TimerStatus::Idle;
        mgr.state.post_break_decision_pending = true;
        mgr.state.current_cycle = 0;

        let payload = mgr
            .resolve_post_break_decision(PostBreakDecisionAction::Continue)
            .unwrap();

        assert_eq!(payload.status, TimerStatus::Working);
        assert_eq!(mgr.state.status, TimerStatus::Working);
        assert_eq!(mgr.state.session_type, SessionType::Work);
        assert_eq!(mgr.state.current_cycle, 1);
        assert!(!mgr.state.post_break_decision_pending);
    }

    #[test]
    fn test_resolve_post_break_decision_stop_cycle_returns_idle_preview() {
        let mut mgr = make_manager();
        let expected_work_preview_secs = 1800;
        mgr.state.status = TimerStatus::Idle;
        mgr.state.post_break_decision_pending = true;
        mgr.state.current_cycle = 2;
        mgr.state.settings = make_settings(expected_work_preview_secs, 300, 900, 4);

        let payload = mgr
            .resolve_post_break_decision(PostBreakDecisionAction::StopCycle)
            .unwrap();

        assert_eq!(payload.status, TimerStatus::Idle);
        assert_eq!(mgr.state.status, TimerStatus::Idle);
        assert_eq!(mgr.state.session_type, SessionType::Work);
        assert_eq!(mgr.state.current_cycle, 2);
        assert_eq!(mgr.state.remaining_seconds, expected_work_preview_secs);
        assert!(!mgr.state.post_break_decision_pending);
    }

    #[test]
    fn test_resolve_post_break_decision_rejects_when_not_pending() {
        let mut mgr = make_manager();
        mgr.state.status = TimerStatus::Idle;
        mgr.state.post_break_decision_pending = false;

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
        let mut mgr = make_manager();
        let settings = make_settings(1500, 300, 900, 4);
        mgr.start_work_with_source(settings, TimerStartSource::ManualOutsideWindow)
            .unwrap();
        mgr.transition_to_break();

        // Drain some time from the break timer
        mgr.state.remaining_seconds = 100;

        let result = mgr.apply_input_penalty();
        assert!(result.is_some());
        // Timer reset to full short break duration
        assert_eq!(mgr.state.remaining_seconds, 300);
        assert_eq!(mgr.state.break_interaction_count, 1);
    }

    #[test]
    fn test_emergency_stop_resets_to_idle() {
        let mut mgr = make_manager();
        let settings = make_settings(1500, 300, 900, 4);
        mgr.start_work_with_source(settings, TimerStartSource::ManualOutsideWindow)
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
        assert_eq!(mgr.state.settings.work_duration_secs.get(), 1800);
        assert_eq!(mgr.state.settings.short_break_duration_secs.get(), 420);
        assert_eq!(mgr.state.settings.long_break_duration_secs.get(), 1200);
        assert_eq!(mgr.state.settings.long_break_interval.get(), 5);
        assert_eq!(mgr.state.remaining_seconds, 1800);
    }

    #[test]
    fn test_sync_timer_duration_settings_rejects_non_idle_state() {
        let mut mgr = make_manager();
        mgr.state.status = TimerStatus::Working;

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
        let mut mgr = make_manager();
        mgr.state.status = TimerStatus::Working;
        mgr.state.timer_start_source = Some(crate::types::TimerStartSource::ManualInsideWindow);
        mgr.state.scheduled_stop_state = ScheduledStopState::Pending;

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
        assert!(mgr.state.auto_run_feature_enabled);
        assert_eq!(mgr.state.weekly_auto_run_config, weekly_auto_run_config);
        assert_eq!(mgr.state.scheduled_stop_state, ScheduledStopState::Pending);
        assert_eq!(
            mgr.state.timer_start_source,
            Some(crate::types::TimerStartSource::ManualInsideWindow)
        );
    }

    #[test]
    fn test_sync_automation_policies_clears_pending_stop_state_when_feature_is_disabled() {
        let mut mgr = make_manager();
        mgr.state.status = TimerStatus::OnBreak;
        mgr.state.scheduled_stop_state = ScheduledStopState::Pending;

        let payload = mgr.sync_automation_policies(
            false,
            WeeklyAutoRunConfig::default(),
            PostBreakTransitionPolicy::RequireManualContinue,
        );

        assert_eq!(payload.status, TimerStatus::OnBreak);
        assert_eq!(mgr.state.scheduled_stop_state, ScheduledStopState::None);
    }

    #[test]
    fn test_manual_inside_window_start_sets_scheduled_stop_pending() {
        let mut mgr = make_manager();
        mgr.state.status = TimerStatus::Working;
        mgr.state.auto_run_feature_enabled = true;
        mgr.state.timer_start_source = Some(TimerStartSource::ManualInsideWindow);
        mgr.state.scheduled_stop_state = ScheduledStopState::None;

        let changed = mgr.mark_scheduled_stop_pending_if_session_is_eligible();

        assert!(changed);
        assert_eq!(mgr.state.scheduled_stop_state, ScheduledStopState::Pending);
    }

    #[test]
    fn test_scheduled_auto_start_sets_scheduled_stop_pending() {
        let mut mgr = make_manager();
        mgr.state.status = TimerStatus::Working;
        mgr.state.auto_run_feature_enabled = true;
        mgr.state.timer_start_source = Some(TimerStartSource::ScheduledAutoStart);
        mgr.state.scheduled_stop_state = ScheduledStopState::None;

        let changed = mgr.mark_scheduled_stop_pending_if_session_is_eligible();

        assert!(changed);
        assert_eq!(mgr.state.scheduled_stop_state, ScheduledStopState::Pending);
    }

    #[test]
    fn test_manual_outside_window_start_does_not_set_scheduled_stop_pending() {
        let mut mgr = make_manager();
        mgr.state.status = TimerStatus::Working;
        mgr.state.auto_run_feature_enabled = true;
        mgr.state.timer_start_source = Some(TimerStartSource::ManualOutsideWindow);
        mgr.state.scheduled_stop_state = ScheduledStopState::None;

        let changed = mgr.mark_scheduled_stop_pending_if_session_is_eligible();

        assert!(!changed);
        assert_eq!(mgr.state.scheduled_stop_state, ScheduledStopState::None);
    }

    #[test]
    fn test_mark_scheduled_stop_pending_does_not_change_when_idle() {
        let mut mgr = make_manager();
        mgr.state.status = TimerStatus::Idle;
        mgr.state.auto_run_feature_enabled = true;
        mgr.state.timer_start_source = Some(TimerStartSource::ManualInsideWindow);
        mgr.state.scheduled_stop_state = ScheduledStopState::None;

        let changed = mgr.mark_scheduled_stop_pending_if_session_is_eligible();

        assert!(!changed);
        assert_eq!(mgr.state.scheduled_stop_state, ScheduledStopState::None);
    }

    #[test]
    fn test_mark_scheduled_stop_pending_keeps_pending_when_called_repeatedly() {
        let mut mgr = make_manager();
        mgr.state.status = TimerStatus::OnBreak;
        mgr.state.auto_run_feature_enabled = true;
        mgr.state.timer_start_source = Some(TimerStartSource::ManualInsideWindow);
        mgr.state.scheduled_stop_state = ScheduledStopState::Pending;

        let changed = mgr.mark_scheduled_stop_pending_if_session_is_eligible();

        assert!(!changed);
        assert_eq!(mgr.state.scheduled_stop_state, ScheduledStopState::Pending);
    }

    #[test]
    fn test_mark_scheduled_stop_pending_uses_timer_start_source_after_auto_run_config_change() {
        let mut mgr = make_manager();
        mgr.state.status = TimerStatus::Working;
        mgr.state.auto_run_feature_enabled = true;
        mgr.state.timer_start_source = Some(TimerStartSource::ManualInsideWindow);
        mgr.state.scheduled_stop_state = ScheduledStopState::None;

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

        let changed = mgr.mark_scheduled_stop_pending_if_session_is_eligible();

        assert!(changed);
        assert_eq!(mgr.state.scheduled_stop_state, ScheduledStopState::Pending);
    }

    #[test]
    fn test_transition_to_idle_due_to_scheduled_stop_from_work_cleans_up_and_skips_dialog() {
        let mut mgr = make_manager();
        let expected_work_preview_secs = 1800;
        mgr.state.status = TimerStatus::Working;
        mgr.state.session_type = SessionType::Work;
        mgr.state.remaining_seconds = 0;
        mgr.state.current_cycle = 2;
        mgr.state.completed_pomodoros = 3;
        mgr.state.post_break_decision_pending = false;
        mgr.state.timer_start_source = Some(TimerStartSource::ManualInsideWindow);
        mgr.state.scheduled_stop_state = ScheduledStopState::Pending;
        mgr.state.settings = make_settings(expected_work_preview_secs, 300, 900, 4);

        let payload = mgr.transition_to_idle_due_to_scheduled_stop(SessionType::Work);

        assert_eq!(payload.completed_session_type, SessionType::Work);
        assert_eq!(
            payload.transition_reason,
            crate::types::SessionTransitionReason::ScheduledStopExecuted
        );
        assert_eq!(payload.next_status, TimerStatus::Idle);
        assert_eq!(payload.next_session_type, SessionType::Work);
        assert!(!payload.requires_manual_continue_decision);
        assert!(!payload.post_break_decision_pending);
        assert_eq!(mgr.state.status, TimerStatus::Idle);
        assert_eq!(mgr.state.session_type, SessionType::Work);
        assert_eq!(mgr.state.remaining_seconds, expected_work_preview_secs);
        assert_eq!(mgr.state.completed_pomodoros, 4);
        assert!(!mgr.state.post_break_decision_pending);
        assert_eq!(mgr.state.timer_start_source, None);
        assert_eq!(mgr.state.scheduled_stop_state, ScheduledStopState::None);
    }

    #[test]
    fn test_transition_to_idle_due_to_scheduled_stop_from_long_break_resets_cycle() {
        let mut mgr = make_manager();
        mgr.state.status = TimerStatus::OnBreak;
        mgr.state.session_type = SessionType::LongBreak;
        mgr.state.current_cycle = 4;
        mgr.state.post_break_decision_pending = false;
        mgr.state.timer_start_source = Some(TimerStartSource::ScheduledAutoStart);
        mgr.state.scheduled_stop_state = ScheduledStopState::Pending;

        let payload = mgr.transition_to_idle_due_to_scheduled_stop(SessionType::LongBreak);

        assert_eq!(payload.completed_session_type, SessionType::LongBreak);
        assert_eq!(
            payload.transition_reason,
            crate::types::SessionTransitionReason::ScheduledStopExecuted
        );
        assert_eq!(payload.next_status, TimerStatus::Idle);
        assert_eq!(payload.next_session_type, SessionType::Work);
        assert!(!payload.requires_manual_continue_decision);
        assert!(!payload.post_break_decision_pending);
        assert_eq!(mgr.state.status, TimerStatus::Idle);
        assert_eq!(mgr.state.session_type, SessionType::Work);
        assert_eq!(mgr.state.current_cycle, 0);
        assert_eq!(mgr.state.timer_start_source, None);
        assert_eq!(mgr.state.scheduled_stop_state, ScheduledStopState::None);
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
    fn test_rate_limit_remaining_ms_returns_none_when_no_transition() {
        let mgr = make_manager();
        // last_transition is None on a fresh manager
        assert!(mgr.rate_limit_remaining_ms().is_none());
    }

    #[test]
    fn test_rate_limit_remaining_ms_returns_some_immediately_after_transition() {
        let mut mgr = make_manager();
        let settings = make_settings(1500, 300, 900, 4);
        mgr.start_work_with_source(settings, TimerStartSource::ManualOutsideWindow)
            .unwrap();

        // Called immediately after start_work sets last_transition
        let remaining = mgr.rate_limit_remaining_ms();
        assert!(remaining.is_some());
        let ms = remaining.unwrap();
        assert!(ms > 0, "remaining_ms should be > 0");
        assert!(ms <= 300, "remaining_ms should be <= 300, got {ms}");
    }

    #[test]
    fn test_rate_limit_remaining_ms_returns_none_after_limit_elapsed() {
        let mut mgr = make_manager();
        let Some(expired_transition) = Instant::now().checked_sub(Duration::from_millis(301))
        else {
            panic!("expected checked_sub to succeed for short test duration");
        };
        mgr.state.last_transition = Some(expired_transition);

        assert!(mgr.rate_limit_remaining_ms().is_none());
    }

    #[test]
    fn test_start_work_rate_limited_returns_accurate_remaining_ms() {
        let mut mgr = make_manager();
        let settings = make_settings(1500, 300, 900, 4);
        mgr.start_work_with_source(settings.clone(), TimerStartSource::ManualOutsideWindow)
            .unwrap();

        // Immediately call emergency_stop to reset to Idle, preserving last_transition timing
        mgr.state.status = TimerStatus::Idle;

        // Second start_work must be rate-limited.
        // remaining_ms reflects actual elapsed time: close to 300ms when called immediately,
        // so we assert it is in (0, 300] rather than strictly < 300.
        let err = mgr
            .start_work_with_source(settings, TimerStartSource::ManualOutsideWindow)
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
        mgr.start_work_with_source(settings.clone(), TimerStartSource::ManualOutsideWindow)
            .unwrap();

        // Simulate returning to idle immediately while keeping last_transition fresh.
        mgr.state.status = TimerStatus::Idle;

        let result = mgr.start_work_with_source(settings, TimerStartSource::ScheduledAutoStart);
        assert!(result.is_ok());
    }
}
