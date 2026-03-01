// Timer State Manager
// Manages timer state and session lifecycle without global statics

use std::time::{Duration, Instant};
use tauri::AppHandle;
use tokio_util::sync::CancellationToken;
use tracing::{info, warn};

use crate::error::{PomoliceError, Result};
use crate::types::{
    FullStatePayload, ScheduledStopState, SessionCompletedPayload, SessionType, TimerSettings,
    TimerStartSource, TimerState, TimerStatus, TimerTickPayload, WeeklyAutoRunConfig,
};

/// Manager for timer state and lifecycle
///
/// This struct holds only pure timer state and task lifecycle handles.
/// All side effects (event emission to frontend) are the caller's responsibility.
pub struct TimerStateManager {
    state: TimerState,
    /// Token used to signal the timer task to stop.
    /// Cancelling this token causes the task's `select!` to resolve immediately,
    /// preventing any in-flight tick from being spawned after the task exits.
    cancel_token: Option<CancellationToken>,
    timer_task: Option<tokio::task::JoinHandle<()>>,
}

impl TimerStateManager {
    /// Create a new timer state manager
    pub fn new(persisted_state: Option<TimerState>) -> Self {
        let state = persisted_state.unwrap_or_default();

        Self {
            state,
            cancel_token: None,
            timer_task: None,
        }
    }

    /// Initialize from persisted state (called during app setup)
    ///
    /// Returns the tick payload to emit if the timer was running when the app closed,
    /// or `None` if the timer was idle.
    pub fn initialize_from_persisted_state(&mut self) -> Result<Option<TimerTickPayload>> {
        if self.state.status != TimerStatus::Idle {
            // In the future, we can implement elapsed time calculation here
            let payload = self.state.to_tick_payload();
            return Ok(Some(payload));
        }
        Ok(None)
    }

    /// Get current timer status
    pub fn status(&self) -> TimerStatus {
        self.state.status
    }

    /// Get full state for frontend sync
    pub fn get_full_state(&self) -> FullStatePayload {
        self.state.to_full_payload()
    }

    /// Get mutable reference to internal state (for persistence)
    pub fn get_state(&self) -> &TimerState {
        &self.state
    }

    /// Start a work session with an explicit session start source.
    pub fn start_work_with_source(
        &mut self,
        settings: TimerSettings,
        timer_start_source: TimerStartSource,
    ) -> Result<(TimerTickPayload, u64)> {
        info!(
            "Starting work session with duration: {}s",
            settings.work_duration_secs
        );

        if self.state.status != TimerStatus::Idle {
            warn!("Cannot start work from {:?} state", self.state.status);
            return Err(PomoliceError::InvalidStateTransition {
                action: "start work".to_string(),
                current_state: self.state.status,
            });
        }

        if timer_start_source != TimerStartSource::ScheduledAutoStart {
            if let Some(remaining_ms) = self.rate_limit_remaining_ms() {
                warn!(
                    "Start work action is rate limited ({}ms remaining)",
                    remaining_ms
                );
                return Err(PomoliceError::RateLimited { remaining_ms });
            }
        }

        let work_duration_secs = settings.work_duration_secs;

        // Update settings
        self.state.settings = settings;

        // Initialize work session
        self.state.status = TimerStatus::Working;
        self.state.session_type = SessionType::Work;
        self.state.remaining_seconds = work_duration_secs;
        self.state.show_continue_dialog = false;
        self.state.timer_start_source = Some(timer_start_source);
        self.state.scheduled_stop_state = ScheduledStopState::None;

        // Initialize cycle on first start
        if self.state.current_cycle == 0 {
            self.state.current_cycle = 1;
        }

        self.state.last_transition = Some(Instant::now());

        // Note: caller must call cancel_and_take_task() before the lock,
        // await the old handle outside the lock, then call spawn_timer_task()
        // inside a new lock. See commands/timer.rs for the call sequence.

        let payload = self.state.to_tick_payload();
        Ok((payload, work_duration_secs))
    }

    /// Continue to next work session after break
    ///
    /// Returns the tick payload for the caller to emit as `"timer_tick"`.
    pub fn continue_to_next_session(&mut self) -> Result<TimerTickPayload> {
        info!("Continuing to next work session");

        if self.state.status != TimerStatus::Idle || !self.state.show_continue_dialog {
            warn!(
                "Cannot continue from {:?} state (show_dialog: {})",
                self.state.status, self.state.show_continue_dialog
            );
            return Err(PomoliceError::InvalidStateTransition {
                action: "continue to next session".to_string(),
                current_state: self.state.status,
            });
        }

        // Increment cycle for next work session
        self.state.current_cycle += 1;
        self.state.session_type = SessionType::Work;
        self.state.remaining_seconds = self.state.settings.work_duration_secs;
        self.state.show_continue_dialog = false;
        self.state.status = TimerStatus::Working;
        self.state.scheduled_stop_state = ScheduledStopState::None;
        self.state.last_transition = Some(Instant::now());

        // Note: see start_work for task management call sequence.

        Ok(self.state.to_tick_payload())
    }

    /// End the current cycle
    ///
    /// Returns the tick payload for the caller to emit as `"timer_tick"`.
    pub fn end_cycle(&mut self) -> Result<TimerTickPayload> {
        // Reset to idle
        self.state.status = TimerStatus::Idle;
        self.state.session_type = SessionType::Work;
        self.state.remaining_seconds = self.state.settings.work_duration_secs;
        self.state.show_continue_dialog = false;
        self.state.timer_start_source = None;
        self.state.scheduled_stop_state = ScheduledStopState::None;
        self.state.last_transition = Some(Instant::now());

        Ok(self.state.to_tick_payload())
    }

    /// Emergency stop - reset all state
    ///
    /// Returns `(was_fullscreen_break, tick_payload)`.
    /// The caller is responsible for emitting `"timer_tick"` and exiting fullscreen if needed.
    pub fn emergency_stop(&mut self) -> Result<(bool, TimerTickPayload)> {
        let was_fullscreen_break = self.state.is_fullscreen_break;

        // Reset state
        self.state.status = TimerStatus::Idle;
        self.state.session_type = SessionType::Work;
        self.state.remaining_seconds = self.state.settings.work_duration_secs;
        self.state.show_continue_dialog = false;
        self.state.is_fullscreen_break = false;
        self.state.break_interaction_count = 0;
        self.state.last_penalty_time = None;
        self.state.timer_start_source = None;
        self.state.scheduled_stop_state = ScheduledStopState::None;
        self.state.last_transition = Some(Instant::now());

        // Note: caller must call cancel_and_take_task() inside the lock,
        // then await the handle outside the lock. See commands/timer.rs.

        Ok((was_fullscreen_break, self.state.to_tick_payload()))
    }

    /// Sync timer settings and auto-run settings from frontend.
    ///
    /// Returns the tick payload for the caller to emit as `"timer_tick"`.
    pub fn sync_all_settings(
        &mut self,
        settings: TimerSettings,
        auto_run_feature_enabled: bool,
        weekly_auto_run_config: WeeklyAutoRunConfig,
    ) -> Result<TimerTickPayload> {
        self.ensure_idle_for_duration_setting_sync("sync all settings")?;

        self.state.settings = settings;
        self.apply_auto_run_settings_update(auto_run_feature_enabled, weekly_auto_run_config);

        // Update remaining seconds if currently in work mode
        if self.state.session_type == SessionType::Work {
            self.state.remaining_seconds = self.state.settings.work_duration_secs;
        }

        Ok(self.state.to_tick_payload())
    }

    /// Sync only auto-run settings from frontend.
    ///
    /// This method is allowed in every timer state. It intentionally does not
    /// recalculate `timer_start_source` and only clears pending scheduled stop
    /// when the feature is disabled.
    pub fn sync_auto_run_settings(
        &mut self,
        auto_run_feature_enabled: bool,
        weekly_auto_run_config: WeeklyAutoRunConfig,
    ) -> TimerTickPayload {
        self.apply_auto_run_settings_update(auto_run_feature_enabled, weekly_auto_run_config);
        self.state.to_tick_payload()
    }

    /// Apply input penalty during break
    ///
    /// Returns the tick payload for the caller to emit as `"timer_tick"`,
    /// or `None` if the penalty was skipped (not on break, or debounced).
    pub fn apply_input_penalty(&mut self) -> Result<Option<TimerTickPayload>> {
        // Only apply penalty if on break
        if self.state.status != TimerStatus::OnBreak {
            return Ok(None);
        }

        // Debounce: 100ms between penalties
        if let Some(last) = self.state.last_penalty_time {
            if last.elapsed() < Duration::from_millis(100) {
                return Ok(None);
            }
        }

        // Increment interaction count
        self.state.break_interaction_count += 1;
        self.state.last_penalty_time = Some(Instant::now());

        // Reset break timer to full duration
        self.state.remaining_seconds = self.state.get_break_duration();

        Ok(Some(self.state.to_tick_payload()))
    }

    /// Record a skip attempt
    pub fn record_skip_attempt(&mut self) -> Result<u32> {
        if self.state.status != TimerStatus::OnBreak {
            return Err(PomoliceError::InvalidStateTransition {
                action: "record skip attempt".to_string(),
                current_state: self.state.status,
            });
        }
        self.state.skip_attempts += 1;
        Ok(self.state.skip_attempts)
    }

    /// Set fullscreen break state
    pub fn set_fullscreen_break(&mut self, is_fullscreen: bool) {
        self.state.is_fullscreen_break = is_fullscreen;
    }

    /// Handle a timer tick (called every second)
    /// Returns (should_complete_session, tick_payload)
    pub fn tick(&mut self) -> (bool, TimerTickPayload) {
        // Decrement timer
        if self.state.remaining_seconds > 0 {
            self.state.remaining_seconds -= 1;
        }

        let payload = self.state.to_tick_payload();
        let should_complete = self.state.remaining_seconds == 0;

        (should_complete, payload)
    }

    /// Transition from work session to break
    /// Returns (break_type, session_completed_payload)
    pub fn transition_to_break(&mut self) -> (SessionType, SessionCompletedPayload) {
        use crate::types::SessionType;

        // Work session completed
        self.state.completed_pomodoros += 1;

        // Determine break type
        let break_type = get_break_type(
            self.state.current_cycle,
            self.state.settings.long_break_interval,
        );

        info!(
            "Transitioning to {:?} (completed {} pomodoros)",
            break_type, self.state.completed_pomodoros
        );

        self.state.session_type = break_type;

        // Set break duration
        self.state.remaining_seconds = match break_type {
            SessionType::ShortBreak => self.state.settings.short_break_duration_secs,
            SessionType::LongBreak => self.state.settings.long_break_duration_secs,
            SessionType::Work => unreachable!(),
        };

        // Transition to break
        self.state.status = TimerStatus::OnBreak;
        self.state.break_interaction_count = 0;
        self.state.last_transition = Some(Instant::now());

        let payload = SessionCompletedPayload {
            completed_session_type: SessionType::Work,
            next_session_type: Some(break_type),
            show_continue_dialog: false,
        };

        (break_type, payload)
    }

    /// Transition from break session to idle
    /// Returns session_completed_payload
    pub fn transition_to_idle_after_break(&mut self) -> SessionCompletedPayload {
        let is_long_break = self.state.session_type == SessionType::LongBreak;
        let completed_type = self.state.session_type;

        // Stop timer
        self.state.status = TimerStatus::Idle;
        self.state.show_continue_dialog = true;
        self.state.timer_start_source = None;
        self.state.scheduled_stop_state = ScheduledStopState::None;
        self.state.last_transition = Some(Instant::now());

        // If long break completed, reset cycle
        if is_long_break {
            self.state.current_cycle = 0;
        }

        // Note: caller must cancel and await the old task outside the lock.

        SessionCompletedPayload {
            completed_session_type: completed_type,
            next_session_type: None,
            show_continue_dialog: true,
        }
    }

    /// Transition to idle when a scheduled stop is pending at session boundary.
    ///
    /// This path intentionally skips the continue dialog and clears scheduled-stop
    /// metadata so later sessions start cleanly.
    pub fn transition_to_idle_due_to_scheduled_stop(
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
        self.state.remaining_seconds = self.state.settings.work_duration_secs;
        self.state.show_continue_dialog = false;
        self.state.timer_start_source = None;
        self.state.scheduled_stop_state = ScheduledStopState::None;
        self.state.last_transition = Some(Instant::now());

        SessionCompletedPayload {
            completed_session_type,
            next_session_type: None,
            show_continue_dialog: false,
        }
    }

    /// Returns the remaining rate-limit duration in milliseconds, or None if not rate-limited.
    fn rate_limit_remaining_ms(&self) -> Option<u64> {
        if let Some(t) = self.state.last_transition {
            let elapsed = t.elapsed();
            let limit = Duration::from_millis(300);
            if elapsed < limit {
                return Some((limit - elapsed).as_millis() as u64);
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
    ) {
        self.state.auto_run_feature_enabled = auto_run_feature_enabled;
        self.state.weekly_auto_run_config = weekly_auto_run_config;

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
    pub fn mark_scheduled_stop_pending_if_session_is_eligible(&mut self) -> bool {
        if !matches!(self.state.status, TimerStatus::Working | TimerStatus::OnBreak) {
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

    /// Cancel the running timer task and extract its JoinHandle.
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
    pub fn cancel_and_take_task(&mut self) -> Option<tokio::task::JoinHandle<()>> {
        if let Some(token) = self.cancel_token.take() {
            token.cancel();
        }
        self.timer_task.take()
    }

    /// Spawn a new timer task.
    ///
    /// `app_handle` is passed by value here (not stored as a field) so that
    /// `TimerStateManager` remains free of `AppHandle` ownership.
    ///
    /// Must be called **after** the old task has been awaited (outside the lock).
    /// See `cancel_and_take_task` for the required call sequence.
    pub fn spawn_timer_task(&mut self, app_handle: AppHandle) {
        let token = CancellationToken::new();
        self.cancel_token = Some(token.clone());

        let handle = tokio::task::spawn(async move {
            loop {
                tokio::select! {
                    // Cancel wins immediately with no side-effect
                    _ = token.cancelled() => break,
                    // Sleep for one second, then dispatch a tick
                    _ = tokio::time::sleep(Duration::from_secs(1)) => {
                        // Re-check cancellation after waking so we never dispatch
                        // a tick that races with a newly started session
                        if token.is_cancelled() {
                            break;
                        }
                        crate::lib_handle_timer_tick(app_handle.clone()).await;
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

    fn make_manager() -> TimerStateManager {
        TimerStateManager::new(None)
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
        assert_eq!(state.skip_attempts, 0);
        assert!(!state.show_continue_dialog);
        assert!(!state.is_fullscreen_break);
    }

    #[test]
    fn test_timer_state_get_break_duration() {
        let mut state = TimerState::default();
        state.settings.short_break_duration_secs = 300;
        state.settings.long_break_duration_secs = 900;

        state.session_type = SessionType::ShortBreak;
        assert_eq!(state.get_break_duration(), 300);

        state.session_type = SessionType::LongBreak;
        assert_eq!(state.get_break_duration(), 900);

        state.session_type = SessionType::Work;
        assert_eq!(state.get_break_duration(), 0); // Work session returns 0
    }

    #[test]
    fn test_timer_state_to_tick_payload() {
        let mut state = TimerState::default();
        state.status = TimerStatus::Working;
        state.remaining_seconds = 1500;
        state.session_type = SessionType::Work;
        state.current_cycle = 2;

        let payload = state.to_tick_payload();
        assert_eq!(payload.status, TimerStatus::Working);
        assert_eq!(payload.remaining_seconds, 1500);
        assert_eq!(payload.session_type, SessionType::Work);
        assert_eq!(payload.current_cycle, 2);
    }

    #[test]
    fn test_timer_state_to_full_payload() {
        let mut state = TimerState::default();
        state.status = TimerStatus::OnBreak;
        state.remaining_seconds = 300;
        state.session_type = SessionType::ShortBreak;
        state.current_cycle = 1;
        state.completed_pomodoros = 1;
        state.show_continue_dialog = false;

        let payload = state.to_full_payload();
        assert_eq!(payload.status, TimerStatus::OnBreak);
        assert_eq!(payload.remaining_seconds, 300);
        assert_eq!(payload.session_type, SessionType::ShortBreak);
        assert_eq!(payload.current_cycle, 1);
        assert_eq!(payload.completed_pomodoros, 1);
        assert!(!payload.show_continue_dialog);
    }

    #[test]
    fn test_start_work_transitions_to_working() {
        let mut mgr = make_manager();
        let settings = TimerSettings {
            work_duration_secs: 1500,
            short_break_duration_secs: 300,
            long_break_duration_secs: 900,
            long_break_interval: 4,
        };

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
        let settings = TimerSettings {
            work_duration_secs: 1500,
            short_break_duration_secs: 300,
            long_break_duration_secs: 900,
            long_break_interval: 4,
        };

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
        let settings = TimerSettings {
            work_duration_secs: 1500,
            short_break_duration_secs: 300,
            long_break_duration_secs: 900,
            long_break_interval: 4,
        };

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
        let settings = TimerSettings {
            work_duration_secs: 1500,
            short_break_duration_secs: 300,
            long_break_duration_secs: 900,
            long_break_interval: 4,
        };

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
        let settings = TimerSettings {
            work_duration_secs: 5,
            short_break_duration_secs: 300,
            long_break_duration_secs: 900,
            long_break_interval: 4,
        };
        mgr.start_work_with_source(settings, TimerStartSource::ManualOutsideWindow)
            .unwrap();

        let (should_complete, payload) = mgr.tick();
        assert!(!should_complete);
        assert_eq!(payload.remaining_seconds, 4);
    }

    #[test]
    fn test_tick_signals_completion_at_zero() {
        let mut mgr = make_manager();
        let settings = TimerSettings {
            work_duration_secs: 1,
            short_break_duration_secs: 300,
            long_break_duration_secs: 900,
            long_break_interval: 4,
        };
        mgr.start_work_with_source(settings, TimerStartSource::ManualOutsideWindow)
            .unwrap();

        let (should_complete, _) = mgr.tick();
        assert!(should_complete);
    }

    #[test]
    fn test_apply_input_penalty_skipped_when_not_on_break() {
        let mut mgr = make_manager();
        let result = mgr.apply_input_penalty().unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn test_apply_input_penalty_resets_break_timer() {
        let mut mgr = make_manager();
        let settings = TimerSettings {
            work_duration_secs: 1500,
            short_break_duration_secs: 300,
            long_break_duration_secs: 900,
            long_break_interval: 4,
        };
        mgr.start_work_with_source(settings, TimerStartSource::ManualOutsideWindow)
            .unwrap();
        mgr.transition_to_break();

        // Drain some time from the break timer
        mgr.state.remaining_seconds = 100;

        let result = mgr.apply_input_penalty().unwrap();
        assert!(result.is_some());
        // Timer reset to full short break duration
        assert_eq!(mgr.state.remaining_seconds, 300);
        assert_eq!(mgr.state.break_interaction_count, 1);
    }

    #[test]
    fn test_emergency_stop_resets_to_idle() {
        let mut mgr = make_manager();
        let settings = TimerSettings {
            work_duration_secs: 1500,
            short_break_duration_secs: 300,
            long_break_duration_secs: 900,
            long_break_interval: 4,
        };
        mgr.start_work_with_source(settings, TimerStartSource::ManualOutsideWindow)
            .unwrap();

        let (was_fullscreen, payload) = mgr.emergency_stop().unwrap();
        assert!(!was_fullscreen);
        assert_eq!(mgr.status(), TimerStatus::Idle);
        assert_eq!(payload.status, TimerStatus::Idle);
    }

    #[test]
    fn test_end_cycle_resets_to_idle() {
        let mut mgr = make_manager();
        let settings = TimerSettings {
            work_duration_secs: 1500,
            short_break_duration_secs: 300,
            long_break_duration_secs: 900,
            long_break_interval: 4,
        };
        mgr.start_work_with_source(settings, TimerStartSource::ManualOutsideWindow)
            .unwrap();
        mgr.state.status = TimerStatus::Idle;
        mgr.state.show_continue_dialog = true;

        let payload = mgr.end_cycle().unwrap();
        assert_eq!(mgr.status(), TimerStatus::Idle);
        assert!(!mgr.state.show_continue_dialog);
        assert_eq!(payload.status, TimerStatus::Idle);
    }

    #[test]
    fn test_sync_all_settings_updates_timer_duration_and_auto_run_fields_when_idle() {
        let mut mgr = make_manager();
        let mut weekly_auto_run_config = WeeklyAutoRunConfig::default();
        weekly_auto_run_config.mon.enabled = true;
        weekly_auto_run_config.mon.start_time = "08:30".to_string();
        weekly_auto_run_config.mon.end_time = "17:30".to_string();

        let payload = mgr
            .sync_all_settings(
                TimerSettings {
                    work_duration_secs: 1800,
                    short_break_duration_secs: 420,
                    long_break_duration_secs: 1200,
                    long_break_interval: 5,
                },
                true,
                weekly_auto_run_config.clone(),
            )
            .unwrap();

        assert_eq!(payload.status, TimerStatus::Idle);
        assert_eq!(mgr.state.settings.work_duration_secs, 1800);
        assert_eq!(mgr.state.remaining_seconds, 1800);
        assert!(mgr.state.auto_run_feature_enabled);
        assert_eq!(mgr.state.weekly_auto_run_config, weekly_auto_run_config);
    }

    #[test]
    fn test_sync_all_settings_rejects_non_idle_state() {
        let mut mgr = make_manager();
        mgr.state.status = TimerStatus::Working;

        let err = mgr
            .sync_all_settings(
                TimerSettings {
                    work_duration_secs: 1800,
                    short_break_duration_secs: 420,
                    long_break_duration_secs: 1200,
                    long_break_interval: 5,
                },
                true,
                WeeklyAutoRunConfig::default(),
            )
            .unwrap_err();

        assert!(matches!(err, PomoliceError::InvalidStateTransition { .. }));
    }

    #[test]
    fn test_sync_auto_run_settings_keeps_pending_stop_state_when_feature_remains_enabled() {
        let mut mgr = make_manager();
        mgr.state.status = TimerStatus::Working;
        mgr.state.timer_start_source = Some(crate::types::TimerStartSource::ManualInsideWindow);
        mgr.state.scheduled_stop_state = ScheduledStopState::Pending;

        let mut weekly_auto_run_config = WeeklyAutoRunConfig::default();
        weekly_auto_run_config.tue.enabled = true;
        weekly_auto_run_config.tue.start_time = "10:00".to_string();
        weekly_auto_run_config.tue.end_time = "19:00".to_string();

        let payload = mgr.sync_auto_run_settings(true, weekly_auto_run_config.clone());

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
    fn test_sync_auto_run_settings_clears_pending_stop_state_when_feature_is_disabled() {
        let mut mgr = make_manager();
        mgr.state.status = TimerStatus::OnBreak;
        mgr.state.scheduled_stop_state = ScheduledStopState::Pending;

        let payload = mgr.sync_auto_run_settings(false, WeeklyAutoRunConfig::default());

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

        let mut weekly_auto_run_config = WeeklyAutoRunConfig::default();
        weekly_auto_run_config.fri.enabled = true;
        weekly_auto_run_config.fri.start_time = "23:00".to_string();
        weekly_auto_run_config.fri.end_time = "02:00".to_string();
        let _ = mgr.sync_auto_run_settings(true, weekly_auto_run_config);

        let changed = mgr.mark_scheduled_stop_pending_if_session_is_eligible();

        assert!(changed);
        assert_eq!(mgr.state.scheduled_stop_state, ScheduledStopState::Pending);
    }

    #[test]
    fn test_transition_to_idle_due_to_scheduled_stop_from_work_cleans_up_and_skips_dialog() {
        let mut mgr = make_manager();
        mgr.state.status = TimerStatus::Working;
        mgr.state.session_type = SessionType::Work;
        mgr.state.remaining_seconds = 0;
        mgr.state.current_cycle = 2;
        mgr.state.completed_pomodoros = 3;
        mgr.state.show_continue_dialog = false;
        mgr.state.timer_start_source = Some(TimerStartSource::ManualInsideWindow);
        mgr.state.scheduled_stop_state = ScheduledStopState::Pending;

        let payload = mgr.transition_to_idle_due_to_scheduled_stop(SessionType::Work);

        assert_eq!(payload.completed_session_type, SessionType::Work);
        assert!(!payload.show_continue_dialog);
        assert_eq!(mgr.state.status, TimerStatus::Idle);
        assert_eq!(mgr.state.session_type, SessionType::Work);
        assert_eq!(mgr.state.remaining_seconds, mgr.state.settings.work_duration_secs);
        assert_eq!(mgr.state.completed_pomodoros, 4);
        assert!(!mgr.state.show_continue_dialog);
        assert_eq!(mgr.state.timer_start_source, None);
        assert_eq!(mgr.state.scheduled_stop_state, ScheduledStopState::None);
    }

    #[test]
    fn test_transition_to_idle_due_to_scheduled_stop_from_long_break_resets_cycle() {
        let mut mgr = make_manager();
        mgr.state.status = TimerStatus::OnBreak;
        mgr.state.session_type = SessionType::LongBreak;
        mgr.state.current_cycle = 4;
        mgr.state.show_continue_dialog = false;
        mgr.state.timer_start_source = Some(TimerStartSource::ScheduledAutoStart);
        mgr.state.scheduled_stop_state = ScheduledStopState::Pending;

        let payload = mgr.transition_to_idle_due_to_scheduled_stop(SessionType::LongBreak);

        assert_eq!(payload.completed_session_type, SessionType::LongBreak);
        assert!(!payload.show_continue_dialog);
        assert_eq!(mgr.state.status, TimerStatus::Idle);
        assert_eq!(mgr.state.session_type, SessionType::Work);
        assert_eq!(mgr.state.current_cycle, 0);
        assert_eq!(mgr.state.timer_start_source, None);
        assert_eq!(mgr.state.scheduled_stop_state, ScheduledStopState::None);
    }

    #[test]
    fn test_initialize_from_persisted_state_returns_payload_when_running() {
        let mut state = TimerState::default();
        state.status = TimerStatus::Working;
        state.remaining_seconds = 900;

        let mut mgr = TimerStateManager::new(Some(state));
        let result = mgr.initialize_from_persisted_state().unwrap();

        assert!(result.is_some());
        assert_eq!(result.unwrap().remaining_seconds, 900);
    }

    #[test]
    fn test_initialize_from_persisted_state_returns_none_when_idle() {
        let mut mgr = make_manager();
        let result = mgr.initialize_from_persisted_state().unwrap();
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
        let settings = TimerSettings {
            work_duration_secs: 1500,
            short_break_duration_secs: 300,
            long_break_duration_secs: 900,
            long_break_interval: 4,
        };
        mgr.start_work_with_source(settings, TimerStartSource::ManualOutsideWindow)
            .unwrap();

        // Called immediately after start_work sets last_transition
        let remaining = mgr.rate_limit_remaining_ms();
        assert!(remaining.is_some());
        let ms = remaining.unwrap();
        assert!(ms > 0, "remaining_ms should be > 0");
        assert!(ms <= 300, "remaining_ms should be <= 300, got {}", ms);
    }

    #[test]
    fn test_start_work_rate_limited_returns_accurate_remaining_ms() {
        let mut mgr = make_manager();
        let settings = TimerSettings {
            work_duration_secs: 1500,
            short_break_duration_secs: 300,
            long_break_duration_secs: 900,
            long_break_interval: 4,
        };
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
                    "remaining_ms should be <= 300, got {}",
                    remaining_ms
                );
            }
            other => panic!("Expected RateLimited, got {:?}", other),
        }
    }

    #[test]
    fn test_start_work_with_scheduled_auto_start_skips_rate_limit() {
        let mut mgr = make_manager();
        let settings = TimerSettings {
            work_duration_secs: 1500,
            short_break_duration_secs: 300,
            long_break_duration_secs: 900,
            long_break_interval: 4,
        };
        mgr.start_work_with_source(settings.clone(), TimerStartSource::ManualOutsideWindow)
            .unwrap();

        // Simulate returning to idle immediately while keeping last_transition fresh.
        mgr.state.status = TimerStatus::Idle;

        let result = mgr.start_work_with_source(settings, TimerStartSource::ScheduledAutoStart);
        assert!(result.is_ok());
    }
}
