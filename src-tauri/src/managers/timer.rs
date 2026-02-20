// Timer State Manager
// Manages timer state and session lifecycle without global statics

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter};
use tracing::{info, warn};

use crate::error::{PomoliceError, Result};
use crate::types::{
    FullStatePayload, SessionCompletedPayload, SessionType, TimerSettings, TimerState, TimerStatus,
    TimerTickPayload,
};

/// Manager for timer state and lifecycle
pub struct TimerStateManager {
    app_handle: AppHandle,
    state: TimerState,
    timer_running: Arc<AtomicBool>,
    timer_thread: Option<JoinHandle<()>>,
}

impl TimerStateManager {
    /// Create a new timer state manager
    pub fn new(app_handle: AppHandle, persisted_state: Option<TimerState>) -> Self {
        let state = persisted_state.unwrap_or_default();

        Self {
            app_handle,
            state,
            timer_running: Arc::new(AtomicBool::new(false)),
            timer_thread: None,
        }
    }

    /// Initialize from persisted state (called during app setup)
    pub fn initialize_from_persisted_state(&mut self) -> Result<()> {
        // If the timer was running when the app closed, we need to handle elapsed time
        if self.state.status != TimerStatus::Idle {
            // For now, just emit the current state
            // In the future, we can implement elapsed time calculation here
            let payload = self.state.to_tick_payload();
            let _ = self.app_handle.emit("timer_tick", &payload);
        }
        Ok(())
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

    /// Start a work session
    pub fn start_work(&mut self, settings: TimerSettings) -> Result<()> {
        info!("Starting work session with duration: {}s", settings.work_duration_secs);

        if self.state.status != TimerStatus::Idle {
            warn!("Cannot start work from {:?} state", self.state.status);
            return Err(PomoliceError::InvalidStateTransition {
                action: "start work".to_string(),
                current_state: self.state.status,
            });
        }

        if self.is_rate_limited() {
            warn!("Start work action is rate limited");
            return Err(PomoliceError::RateLimited { remaining_ms: 300 });
        }

        // Update settings
        self.state.settings = settings.clone();

        // Initialize work session
        self.state.status = TimerStatus::Working;
        self.state.session_type = SessionType::Work;
        self.state.remaining_seconds = settings.work_duration_secs;
        self.state.show_continue_dialog = false;

        // Initialize cycle on first start
        if self.state.current_cycle == 0 {
            self.state.current_cycle = 1;
        }

        self.state.last_transition = Some(Instant::now());

        // Emit initial state
        let payload = self.state.to_tick_payload();
        let _ = self.app_handle.emit("timer_tick", &payload);

        // Start timer thread
        self.start_timer_thread();

        let _ = self.app_handle.emit(
            "work_started",
            serde_json::json!({ "duration": settings.work_duration_secs }),
        );

        Ok(())
    }

    /// Continue to next work session after break
    pub fn continue_to_next_session(&mut self) -> Result<()> {
        info!("Continuing to next work session");

        if self.state.status != TimerStatus::Idle || !self.state.show_continue_dialog {
            warn!("Cannot continue from {:?} state (show_dialog: {})",
                self.state.status, self.state.show_continue_dialog);
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
        self.state.last_transition = Some(Instant::now());

        // Emit initial state
        let payload = self.state.to_tick_payload();
        let _ = self.app_handle.emit("timer_tick", &payload);

        // Start timer thread
        self.start_timer_thread();

        Ok(())
    }

    /// End the current cycle
    pub fn end_cycle(&mut self) -> Result<()> {
        // Reset to idle
        self.state.status = TimerStatus::Idle;
        self.state.session_type = SessionType::Work;
        self.state.remaining_seconds = self.state.settings.work_duration_secs;
        self.state.show_continue_dialog = false;
        self.state.last_transition = Some(Instant::now());

        // Emit state
        let payload = self.state.to_tick_payload();
        let _ = self.app_handle.emit("timer_tick", &payload);

        Ok(())
    }

    /// Emergency stop - reset all state
    pub fn emergency_stop(&mut self) -> Result<bool> {
        let was_fullscreen_break = self.state.is_fullscreen_break;

        // Reset state
        self.state.status = TimerStatus::Idle;
        self.state.session_type = SessionType::Work;
        self.state.remaining_seconds = self.state.settings.work_duration_secs;
        self.state.show_continue_dialog = false;
        self.state.is_fullscreen_break = false;
        self.state.break_interaction_count = 0;
        self.state.last_penalty_time = None;
        self.state.last_transition = Some(Instant::now());

        // Emit state
        let payload = self.state.to_tick_payload();
        let _ = self.app_handle.emit("timer_tick", &payload);

        // Stop timer thread
        self.stop_timer_thread();

        Ok(was_fullscreen_break)
    }

    /// Sync settings from frontend
    pub fn sync_settings(&mut self, settings: TimerSettings) -> Result<()> {
        if self.state.status != TimerStatus::Idle {
            return Err(PomoliceError::InvalidStateTransition {
                action: "sync settings".to_string(),
                current_state: self.state.status,
            });
        }

        self.state.settings = settings;

        // Update remaining seconds if currently in work mode
        if self.state.session_type == SessionType::Work {
            self.state.remaining_seconds = self.state.settings.work_duration_secs;
        }

        // Emit updated state
        let payload = self.state.to_tick_payload();
        let _ = self.app_handle.emit("timer_tick", &payload);

        Ok(())
    }

    /// Apply input penalty during break
    pub fn apply_input_penalty(&mut self) -> Result<()> {
        // Only apply penalty if on break
        if self.state.status != TimerStatus::OnBreak {
            return Ok(());
        }

        // Debounce: 100ms between penalties
        if let Some(last) = self.state.last_penalty_time {
            if last.elapsed() < Duration::from_millis(100) {
                return Ok(());
            }
        }

        // Increment interaction count
        self.state.break_interaction_count += 1;
        self.state.last_penalty_time = Some(Instant::now());

        // Reset break timer to full duration
        self.state.remaining_seconds = self.state.get_break_duration();

        // Emit updated state
        let payload = self.state.to_tick_payload();
        let _ = self.app_handle.emit("timer_tick", &payload);

        Ok(())
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

        info!("Transitioning to {:?} (completed {} pomodoros)",
            break_type, self.state.completed_pomodoros);

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
        self.state.last_transition = Some(Instant::now());

        // If long break completed, reset cycle
        if is_long_break {
            self.state.current_cycle = 0;
        }

        // Stop timer thread
        self.stop_timer_thread();

        SessionCompletedPayload {
            completed_session_type: completed_type,
            next_session_type: None,
            show_continue_dialog: true,
        }
    }

    /// Check if an action is rate limited
    fn is_rate_limited(&self) -> bool {
        if let Some(t) = self.state.last_transition {
            return t.elapsed() < Duration::from_millis(300);
        }
        false
    }

    /// Start the timer thread
    fn start_timer_thread(&mut self) {
        // Stop any existing timer
        self.stop_timer_thread();

        self.timer_running.store(true, Ordering::SeqCst);

        let app_handle = self.app_handle.clone();
        let timer_running = Arc::clone(&self.timer_running);

        let handle = thread::spawn(move || {
            while timer_running.load(Ordering::SeqCst) {
                thread::sleep(Duration::from_secs(1));

                if !timer_running.load(Ordering::SeqCst) {
                    break;
                }

                // Call the tick handler in an async context
                let app = app_handle.clone();
                tauri::async_runtime::spawn(async move {
                    crate::lib_handle_timer_tick(app).await;
                });
            }
        });

        self.timer_thread = Some(handle);
    }

    /// Stop the timer thread
    fn stop_timer_thread(&mut self) {
        self.timer_running.store(false, Ordering::SeqCst);

        if let Some(handle) = self.timer_thread.take() {
            let _ = handle.join();
        }
    }
}

impl Drop for TimerStateManager {
    fn drop(&mut self) {
        self.stop_timer_thread();
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
}
