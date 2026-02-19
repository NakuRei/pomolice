// Timer State Management Module
// Single source of truth for all timer state
// Security: Prevents unauthorized window control by managing state in Rust

use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter, Manager, Window};

use crate::persistence;

/// Timer status representing the current phase
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TimerStatus {
    Idle,
    Working,
    OnBreak,
}

/// Session type representing work or break sessions
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SessionType {
    Work,
    ShortBreak,
    LongBreak,
}

/// Timer settings synchronized from frontend
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimerSettings {
    pub work_duration_secs: u64,
    pub short_break_duration_secs: u64,
    pub long_break_duration_secs: u64,
    pub long_break_interval: u32,
}

impl Default for TimerSettings {
    fn default() -> Self {
        Self {
            work_duration_secs: 25 * 60,       // 25 minutes
            short_break_duration_secs: 5 * 60, // 5 minutes
            long_break_duration_secs: 15 * 60, // 15 minutes
            long_break_interval: 4,
        }
    }
}

/// Payload sent to frontend on each timer tick
#[derive(Debug, Clone, Serialize)]
pub struct TimerTickPayload {
    pub status: TimerStatus,
    pub remaining_seconds: u64,
    pub session_type: SessionType,
    pub current_cycle: u32,
    pub completed_pomodoros: u32,
    pub break_interaction_count: u32,
    pub is_fullscreen_break: bool,
}

/// Payload sent to frontend when a session completes
#[derive(Debug, Clone, Serialize)]
pub struct SessionCompletedPayload {
    pub completed_session_type: SessionType,
    pub next_session_type: Option<SessionType>,
    pub show_continue_dialog: bool,
}

/// Full timer state sent on initial sync
#[derive(Debug, Clone, Serialize)]
pub struct FullStatePayload {
    pub status: TimerStatus,
    pub remaining_seconds: u64,
    pub session_type: SessionType,
    pub current_cycle: u32,
    pub completed_pomodoros: u32,
    pub break_interaction_count: u32,
    pub is_fullscreen_break: bool,
    pub show_continue_dialog: bool,
    pub settings: TimerSettings,
}

/// Application state for timer management - the single source of truth
#[derive(Debug)]
pub struct TimerState {
    // Current phase
    pub status: TimerStatus,

    // Timer countdown (in seconds)
    pub remaining_seconds: u64,

    // Session management
    pub session_type: SessionType,
    pub current_cycle: u32,
    pub completed_pomodoros: u32,

    // Break enforcement
    pub break_interaction_count: u32,
    pub last_penalty_time: Option<Instant>,
    pub skip_attempts: u32,

    // Settings (synced from frontend)
    pub settings: TimerSettings,

    // UI state
    pub show_continue_dialog: bool,
    pub is_fullscreen_break: bool,

    // Internal
    pub last_transition: Option<Instant>,
}

impl Default for TimerState {
    fn default() -> Self {
        let settings = TimerSettings::default();
        Self {
            status: TimerStatus::Idle,
            remaining_seconds: settings.work_duration_secs,
            session_type: SessionType::Work,
            current_cycle: 0,
            completed_pomodoros: 0,
            break_interaction_count: 0,
            last_penalty_time: None,
            skip_attempts: 0,
            settings,
            show_continue_dialog: false,
            is_fullscreen_break: false,
            last_transition: None,
        }
    }
}

impl TimerState {
    pub fn to_tick_payload(&self) -> TimerTickPayload {
        TimerTickPayload {
            status: self.status,
            remaining_seconds: self.remaining_seconds,
            session_type: self.session_type,
            current_cycle: self.current_cycle,
            completed_pomodoros: self.completed_pomodoros,
            break_interaction_count: self.break_interaction_count,
            is_fullscreen_break: self.is_fullscreen_break,
        }
    }

    pub fn to_full_payload(&self) -> FullStatePayload {
        FullStatePayload {
            status: self.status,
            remaining_seconds: self.remaining_seconds,
            session_type: self.session_type,
            current_cycle: self.current_cycle,
            completed_pomodoros: self.completed_pomodoros,
            break_interaction_count: self.break_interaction_count,
            is_fullscreen_break: self.is_fullscreen_break,
            show_continue_dialog: self.show_continue_dialog,
            settings: self.settings.clone(),
        }
    }

    /// Get break duration based on session type
    fn get_break_duration(&self) -> u64 {
        match self.session_type {
            SessionType::ShortBreak => self.settings.short_break_duration_secs,
            SessionType::LongBreak => self.settings.long_break_duration_secs,
            SessionType::Work => 0,
        }
    }
}

// Global application state
static TIMER_STATE: Mutex<TimerState> = Mutex::new(TimerState {
    status: TimerStatus::Idle,
    remaining_seconds: 25 * 60,
    session_type: SessionType::Work,
    current_cycle: 0,
    completed_pomodoros: 0,
    break_interaction_count: 0,
    last_penalty_time: None,
    skip_attempts: 0,
    settings: TimerSettings {
        work_duration_secs: 25 * 60,
        short_break_duration_secs: 5 * 60,
        long_break_duration_secs: 15 * 60,
        long_break_interval: 4,
    },
    show_continue_dialog: false,
    is_fullscreen_break: false,
    last_transition: None,
});

// Flag to signal timer thread to stop
static TIMER_RUNNING: AtomicBool = AtomicBool::new(false);

// Store the timer thread handle for cleanup
static TIMER_THREAD: Mutex<Option<JoinHandle<()>>> = Mutex::new(None);

fn is_rate_limited(last: Option<Instant>) -> bool {
    if let Some(t) = last {
        return t.elapsed() < Duration::from_millis(300);
    }
    false
}

/// Determine break type based on current cycle
fn get_break_type(cycle: u32, interval: u32) -> SessionType {
    if cycle > 0 && cycle % interval == 0 {
        SessionType::LongBreak
    } else {
        SessionType::ShortBreak
    }
}

/// Start the timer thread that ticks every second
fn start_timer_thread(app: AppHandle) {
    // Stop any existing timer
    stop_timer_thread();

    TIMER_RUNNING.store(true, Ordering::SeqCst);

    let handle = thread::spawn(move || {
        while TIMER_RUNNING.load(Ordering::SeqCst) {
            thread::sleep(Duration::from_secs(1));

            if !TIMER_RUNNING.load(Ordering::SeqCst) {
                break;
            }

            let should_complete_session = {
                let mut state = TIMER_STATE.lock().expect("Failed to lock timer state");

                // Only tick if running
                if state.status == TimerStatus::Idle {
                    continue;
                }

                // Decrement timer
                if state.remaining_seconds > 0 {
                    state.remaining_seconds -= 1;
                }

                // Emit tick event
                let payload = state.to_tick_payload();
                let _ = app.emit("timer_tick", &payload);

                // Save state for persistence
                persistence::save_state(&state);

                // Check if session is complete
                state.remaining_seconds == 0
            };

            if should_complete_session {
                handle_session_complete(&app);
            }
        }
    });

    if let Ok(mut thread_handle) = TIMER_THREAD.lock() {
        *thread_handle = Some(handle);
    }
}

/// Stop the timer thread
fn stop_timer_thread() {
    TIMER_RUNNING.store(false, Ordering::SeqCst);

    if let Ok(mut handle) = TIMER_THREAD.lock() {
        if let Some(h) = handle.take() {
            // Give the thread time to finish
            let _ = h.join();
        }
    }
}

/// Handle session completion and transition to next state
fn handle_session_complete(app: &AppHandle) {
    let mut state = TIMER_STATE.lock().expect("Failed to lock timer state");

    match state.session_type {
        SessionType::Work => {
            // Work session completed
            state.completed_pomodoros += 1;

            // Determine break type
            let break_type = get_break_type(state.current_cycle, state.settings.long_break_interval);
            state.session_type = break_type;

            // Set break duration
            state.remaining_seconds = match break_type {
                SessionType::ShortBreak => state.settings.short_break_duration_secs,
                SessionType::LongBreak => state.settings.long_break_duration_secs,
                SessionType::Work => unreachable!(),
            };

            // Transition to break
            state.status = TimerStatus::OnBreak;
            state.break_interaction_count = 0;
            state.last_transition = Some(Instant::now());

            // Emit session completed event
            let payload = SessionCompletedPayload {
                completed_session_type: SessionType::Work,
                next_session_type: Some(break_type),
                show_continue_dialog: false,
            };
            let _ = app.emit("session_completed", &payload);

            // Enter fullscreen break mode
            let app_clone = app.clone();
            drop(state); // Release lock before async operation

            tauri::async_runtime::spawn(async move {
                enter_break_mode_internal(app_clone).await;
            });
        }
        SessionType::ShortBreak | SessionType::LongBreak => {
            // Break session completed
            let is_long_break = state.session_type == SessionType::LongBreak;

            // Stop timer
            state.status = TimerStatus::Idle;
            state.show_continue_dialog = true;
            state.last_transition = Some(Instant::now());

            // If long break completed, reset cycle
            if is_long_break {
                state.current_cycle = 0;
            }

            // Emit session completed event
            let payload = SessionCompletedPayload {
                completed_session_type: state.session_type,
                next_session_type: None,
                show_continue_dialog: true,
            };
            let _ = app.emit("session_completed", &payload);

            // Exit fullscreen break mode
            let app_clone = app.clone();
            drop(state); // Release lock before async operation

            tauri::async_runtime::spawn(async move {
                exit_break_mode_internal(app_clone).await;
            });

            // Stop timer thread since we're idle
            TIMER_RUNNING.store(false, Ordering::SeqCst);
        }
    }
}

/// Enter fullscreen break mode (internal)
async fn enter_break_mode_internal(app: AppHandle) {
    // Start input monitoring
    if let Err(e) = crate::input_monitor::start_monitoring(app.clone()) {
        eprintln!("Failed to start input monitoring: {}", e);
    }

    // Enter fullscreen on all monitors
    if let Err(e) = crate::window_control::enter_fullscreen_all_monitors(app.clone()).await {
        eprintln!("Failed to enter fullscreen: {}", e);
        return;
    }

    // Update state
    if let Ok(mut state) = TIMER_STATE.lock() {
        state.is_fullscreen_break = true;
    }

    let _ = app.emit("break_started", serde_json::json!({}));
}

/// Exit fullscreen break mode (internal)
async fn exit_break_mode_internal(app: AppHandle) {
    // Stop input monitoring
    if let Err(e) = crate::input_monitor::stop_monitoring() {
        eprintln!("Failed to stop input monitoring: {}", e);
    }

    // Exit fullscreen on all monitors
    if let Err(e) = crate::window_control::exit_fullscreen_all_monitors(app.clone()).await {
        eprintln!("Failed to exit fullscreen: {}", e);
        return;
    }

    // Update state
    if let Ok(mut state) = TIMER_STATE.lock() {
        state.is_fullscreen_break = false;
    }

    let _ = app.emit("break_completed", serde_json::json!({}));
}

// ============================================================================
// Tauri Commands
// ============================================================================

/// Start a work session with settings
#[tauri::command]
pub async fn start_work(
    window: Window,
    work_duration_secs: u64,
    short_break_duration_secs: u64,
    long_break_duration_secs: u64,
    long_break_interval: u32,
) -> Result<(), String> {
    if window.label() != "main" {
        return Err("Unauthorized window".to_string());
    }

    let app: AppHandle = window.app_handle().clone();

    {
        let mut state = TIMER_STATE.lock().expect("Failed to lock timer state");

        if state.status != TimerStatus::Idle {
            return Err(format!("Cannot start work from {:?} state", state.status));
        }

        if is_rate_limited(state.last_transition) {
            return Err("Too many requests".to_string());
        }

        // Update settings
        state.settings = TimerSettings {
            work_duration_secs,
            short_break_duration_secs,
            long_break_duration_secs,
            long_break_interval,
        };

        // Initialize work session
        state.status = TimerStatus::Working;
        state.session_type = SessionType::Work;
        state.remaining_seconds = work_duration_secs;
        state.show_continue_dialog = false;

        // Initialize cycle on first start
        if state.current_cycle == 0 {
            state.current_cycle = 1;
        }

        state.last_transition = Some(Instant::now());

        // Emit initial state
        let payload = state.to_tick_payload();
        let _ = app.emit("timer_tick", &payload);

        // Save state
        persistence::save_state(&state);
    }

    // Start timer thread
    start_timer_thread(app.clone());

    let _ = app.emit(
        "work_started",
        serde_json::json!({ "duration": work_duration_secs }),
    );

    Ok(())
}

/// Continue to next work session after break
#[tauri::command]
pub async fn continue_to_next_session(window: Window) -> Result<(), String> {
    if window.label() != "main" {
        return Err("Unauthorized window".to_string());
    }

    let app: AppHandle = window.app_handle().clone();

    {
        let mut state = TIMER_STATE.lock().expect("Failed to lock timer state");

        if state.status != TimerStatus::Idle || !state.show_continue_dialog {
            return Err("Cannot continue: not in continue dialog state".to_string());
        }

        // Increment cycle for next work session
        state.current_cycle += 1;
        state.session_type = SessionType::Work;
        state.remaining_seconds = state.settings.work_duration_secs;
        state.show_continue_dialog = false;
        state.status = TimerStatus::Working;
        state.last_transition = Some(Instant::now());

        // Emit initial state
        let payload = state.to_tick_payload();
        let _ = app.emit("timer_tick", &payload);

        // Save state
        persistence::save_state(&state);
    }

    // Start timer thread
    start_timer_thread(app.clone());

    Ok(())
}

/// End the current cycle (from continue dialog)
#[tauri::command]
pub async fn end_cycle(window: Window) -> Result<(), String> {
    if window.label() != "main" {
        return Err("Unauthorized window".to_string());
    }

    let app: AppHandle = window.app_handle().clone();

    {
        let mut state = TIMER_STATE.lock().expect("Failed to lock timer state");

        // Reset to idle
        state.status = TimerStatus::Idle;
        state.session_type = SessionType::Work;
        state.remaining_seconds = state.settings.work_duration_secs;
        state.show_continue_dialog = false;
        // Keep completedPomodoros and currentCycle for statistics
        state.last_transition = Some(Instant::now());

        // Emit state
        let payload = state.to_tick_payload();
        let _ = app.emit("timer_tick", &payload);

        // Save state
        persistence::save_state(&state);
    }

    Ok(())
}

/// Emergency stop - reset all state
#[tauri::command]
pub async fn emergency_stop(window: Window) -> Result<(), String> {
    if window.label() != "main" {
        return Err("Unauthorized window".to_string());
    }

    let app: AppHandle = window.app_handle().clone();

    let was_fullscreen_break = {
        let mut state = TIMER_STATE.lock().expect("Failed to lock timer state");

        let was_fullscreen = state.is_fullscreen_break;

        // Reset state
        state.status = TimerStatus::Idle;
        state.session_type = SessionType::Work;
        state.remaining_seconds = state.settings.work_duration_secs;
        state.show_continue_dialog = false;
        state.is_fullscreen_break = false;
        state.break_interaction_count = 0;
        state.last_penalty_time = None;
        // Keep completedPomodoros and currentCycle
        state.last_transition = Some(Instant::now());

        // Emit state
        let payload = state.to_tick_payload();
        let _ = app.emit("timer_tick", &payload);

        // Save state
        persistence::save_state(&state);

        was_fullscreen
    };

    // Stop timer thread
    stop_timer_thread();

    // Exit fullscreen if needed
    if was_fullscreen_break {
        exit_break_mode_internal(app).await;
    }

    Ok(())
}

/// Sync settings from frontend
#[tauri::command]
pub async fn sync_settings(
    window: Window,
    work_duration_secs: u64,
    short_break_duration_secs: u64,
    long_break_duration_secs: u64,
    long_break_interval: u32,
) -> Result<(), String> {
    if window.label() != "main" {
        return Err("Unauthorized window".to_string());
    }

    let app: AppHandle = window.app_handle().clone();

    {
        let mut state = TIMER_STATE.lock().expect("Failed to lock timer state");

        // Only allow settings sync when idle
        if state.status != TimerStatus::Idle {
            return Err("Cannot sync settings while timer is running".to_string());
        }

        state.settings = TimerSettings {
            work_duration_secs,
            short_break_duration_secs,
            long_break_duration_secs,
            long_break_interval,
        };

        // Update remaining seconds if currently in work mode
        if state.session_type == SessionType::Work {
            state.remaining_seconds = work_duration_secs;
        }

        // Emit updated state
        let payload = state.to_tick_payload();
        let _ = app.emit("timer_tick", &payload);

        // Save state
        persistence::save_state(&state);
    }

    Ok(())
}

/// Get full timer state for initial sync
#[tauri::command]
pub async fn get_full_state() -> Result<FullStatePayload, String> {
    let state = TIMER_STATE.lock().expect("Failed to lock timer state");
    Ok(state.to_full_payload())
}

/// Apply input penalty during break (called from input_monitor)
pub fn apply_input_penalty(app: &AppHandle) {
    let mut state = TIMER_STATE.lock().expect("Failed to lock timer state");

    // Only apply penalty if on break
    if state.status != TimerStatus::OnBreak {
        return;
    }

    // Debounce: 100ms between penalties
    if let Some(last) = state.last_penalty_time {
        if last.elapsed() < Duration::from_millis(100) {
            return;
        }
    }

    // Increment interaction count
    state.break_interaction_count += 1;
    state.last_penalty_time = Some(Instant::now());

    // Reset break timer to full duration
    state.remaining_seconds = state.get_break_duration();

    // Emit updated state
    let payload = state.to_tick_payload();
    let _ = app.emit("timer_tick", &payload);

    // Save state
    persistence::save_state(&state);
}

/// Record a skip attempt
#[tauri::command]
pub async fn record_skip_attempt() -> Result<u32, String> {
    let mut state = TIMER_STATE.lock().expect("Failed to lock timer state");
    if state.status != TimerStatus::OnBreak {
        return Err("Not on break".to_string());
    }
    state.skip_attempts += 1;
    Ok(state.skip_attempts)
}

/// Get current timer status (legacy compatibility)
#[tauri::command]
pub async fn get_timer_status() -> Result<String, String> {
    let state = TIMER_STATE.lock().expect("Failed to lock timer state");
    let status_str = match state.status {
        TimerStatus::Idle => "idle",
        TimerStatus::Working => "working",
        TimerStatus::OnBreak => "on_break",
    };
    Ok(status_str.to_string())
}

/// Initialize state from persistence on app startup
pub fn initialize_state(app: &AppHandle) {
    if let Some(persisted) = persistence::load_state() {
        // Calculate elapsed time first (before moving any fields)
        let elapsed_secs = persisted.elapsed_seconds_since_save();
        let persisted_remaining = persisted.remaining_seconds;
        let persisted_status = persisted.status;

        let mut state = TIMER_STATE.lock().expect("Failed to lock timer state");

        // Restore persisted state
        state.status = persisted.status;
        state.session_type = persisted.session_type;
        state.current_cycle = persisted.current_cycle;
        state.completed_pomodoros = persisted.completed_pomodoros;
        state.settings = persisted.settings;
        state.show_continue_dialog = persisted.show_continue_dialog;

        if persisted_status != TimerStatus::Idle {
            if elapsed_secs >= persisted_remaining {
                // Session would have completed - trigger completion
                state.remaining_seconds = 0;
                drop(state);

                // Handle session completion
                handle_session_complete(app);
            } else {
                // Adjust remaining time and continue
                state.remaining_seconds = persisted_remaining.saturating_sub(elapsed_secs);

                // Emit initial state
                let payload = state.to_tick_payload();
                let _ = app.emit("timer_tick", &payload);

                drop(state);

                // Restart timer thread
                start_timer_thread(app.clone());

                // If was on break, re-enter fullscreen
                let state = TIMER_STATE.lock().expect("Failed to lock timer state");
                if state.status == TimerStatus::OnBreak {
                    let app_clone = app.clone();
                    drop(state);
                    tauri::async_runtime::spawn(async move {
                        enter_break_mode_internal(app_clone).await;
                    });
                }
            }
        } else {
            // Emit initial state for idle
            let payload = state.to_tick_payload();
            let _ = app.emit("timer_tick", &payload);
        }
    }
}

// ============================================================================
// Legacy commands for backward compatibility (to be removed)
// ============================================================================

/// Start a break session (legacy - now handled automatically)
#[tauri::command]
pub async fn start_break(window: Window, _duration: u64) -> Result<(), String> {
    if window.label() != "main" {
        return Err("Unauthorized window".to_string());
    }

    // Break is now started automatically after work session completes
    // This command is kept for backward compatibility but does nothing
    Ok(())
}

/// Complete a break session (legacy - now handled automatically)
#[tauri::command]
pub async fn complete_break(window: Window) -> Result<(), String> {
    if window.label() != "main" {
        return Err("Unauthorized window".to_string());
    }

    // Break completion is now handled automatically
    // This command is kept for backward compatibility but does nothing
    Ok(())
}

/// Complete a work session (legacy)
#[tauri::command]
pub async fn complete_work(window: Window) -> Result<(), String> {
    if window.label() != "main" {
        return Err("Unauthorized window".to_string());
    }

    // Work completion is now handled automatically
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_timer_state_default() {
        let state = TimerState::default();
        assert_eq!(state.status, TimerStatus::Idle);
        assert_eq!(state.session_type, SessionType::Work);
        assert_eq!(state.remaining_seconds, 25 * 60);
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
    fn test_timer_settings_default() {
        let settings = TimerSettings::default();
        assert_eq!(settings.work_duration_secs, 25 * 60);
        assert_eq!(settings.short_break_duration_secs, 5 * 60);
        assert_eq!(settings.long_break_duration_secs, 15 * 60);
        assert_eq!(settings.long_break_interval, 4);
    }
}
