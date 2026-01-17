// Timer State Management Module
// Centralized state management for the Pomodoro timer
// Security: Prevents unauthorized window control by managing state in Rust

use tauri::{AppHandle, Emitter, Manager, Window};
use std::sync::Mutex;
use std::time::{Duration, Instant};

/// Timer status representing the current phase
#[derive(Debug, Clone, PartialEq)]
pub enum TimerStatus {
    /// No timer running, idle state
    Idle,
    /// Work session in progress
    Working,
    /// Break session in progress (fullscreen enforced)
    OnBreak,
}

/// Application state for timer management
#[derive(Debug)]
pub struct AppState {
    /// Current timer status
    status: TimerStatus,
    /// When the break started (None if not on break)
    break_start_time: Option<Instant>,
    /// Number of skip attempts during current session
    skip_attempts: u32,
    /// Last time a break/work transition command was accepted (rate limiting)
    last_transition: Option<Instant>,
}

// Global application state
static APP_STATE: Mutex<AppState> = Mutex::new(AppState {
    status: TimerStatus::Idle,
    break_start_time: None,
    skip_attempts: 0,
    last_transition: None,
});

fn is_rate_limited(last: Option<Instant>) -> bool {
    if let Some(t) = last {
        return t.elapsed() < Duration::from_millis(300);
    }
    false
}

/// Start a break session
///
/// This is the ONLY authorized way to enter fullscreen mode.
/// Security: Validates state transition before allowing fullscreen.
///
/// # Arguments
/// * `duration` - Break duration in seconds
///
/// # Errors
/// Returns error if:
/// - Not in Working state
/// - Window control fails
#[tauri::command]
pub async fn start_break(
    window: Window,
    duration: u64,
) -> Result<(), String> {
    // Only allow the main window to initiate break mode
    if window.label() != "main" {
        return Err("Unauthorized window".to_string());
    }

    let app: AppHandle = window.app_handle().clone();

    // Check current state
    {
        let mut state = APP_STATE.lock().unwrap();

        // Only allow break if currently working
        if state.status != TimerStatus::Working {
            return Err("Cannot start break unless working".to_string());
        }

        // Simple rate limit to avoid rapid toggling
        if is_rate_limited(state.last_transition) {
            return Err("Too many break requests".to_string());
        }

        // Transition to break state
        state.status = TimerStatus::OnBreak;
        state.break_start_time = Some(Instant::now());
        state.skip_attempts = 0;
        state.last_transition = Some(Instant::now());
    }

    // Start global input monitoring (backend-managed)
    if let Err(e) = crate::input_monitor::start_monitoring(app.clone()) {
        // Roll back state on failure
        let mut state = APP_STATE.lock().unwrap();
        state.status = TimerStatus::Idle;
        state.break_start_time = None;
        state.last_transition = None;
        return Err(format!("Failed to start input monitoring: {}", e));
    }

    // Enter fullscreen on all monitors
    // Call the internal window control function
    match crate::window_control::enter_fullscreen_all_monitors(app.clone()).await {
        Ok(_) => {
            // Emit event to frontend
            let _ = app.emit("break_started", serde_json::json!({
                "duration": duration
            }));
            Ok(())
        },
        Err(e) => {
            // Rollback state on failure
            let mut state = APP_STATE.lock().unwrap();
            state.status = TimerStatus::Idle;
            state.break_start_time = None;
            Err(format!("Failed to enter fullscreen: {}", e))
        }
    }
}

/// Complete a break session
///
/// This is the ONLY authorized way to exit fullscreen mode.
/// Security: Validates state before allowing exit.
///
/// # Errors
/// Returns error if:
/// - Not in OnBreak state
/// - Window control fails
#[tauri::command]
pub async fn complete_break(
    window: Window,
) -> Result<(), String> {
    if window.label() != "main" {
        return Err("Unauthorized window".to_string());
    }

    let app: AppHandle = window.app_handle().clone();

    // Check current state
    {
        let state = APP_STATE.lock().unwrap();

        if state.status != TimerStatus::OnBreak {
            return Err("Not on break".to_string());
        }

        if is_rate_limited(state.last_transition) {
            return Err("Too many break completion requests".to_string());
        }
    }

    // Stop global input monitoring (best-effort)
    if let Err(e) = crate::input_monitor::stop_monitoring() {
        eprintln!("Failed to stop input monitoring: {}", e);
    }

    // Exit fullscreen on all monitors
    match crate::window_control::exit_fullscreen_all_monitors(app.clone()).await {
        Ok(_) => {
            // Update state
            {
                let mut state = APP_STATE.lock().unwrap();
                state.status = TimerStatus::Idle;
                state.break_start_time = None;
                state.skip_attempts = 0;
                state.last_transition = Some(Instant::now());
            }

            // Emit event to frontend
            let _ = app.emit("break_completed", serde_json::json!({}));
            Ok(())
        },
        Err(e) => {
            Err(format!("Failed to exit fullscreen: {}", e))
        }
    }
}

/// Start a work session
///
/// Transitions from Idle to Working state.
#[tauri::command]
pub async fn start_work(
    window: Window,
    duration: u64,
) -> Result<(), String> {
    if window.label() != "main" {
        return Err("Unauthorized window".to_string());
    }

    let app: AppHandle = window.app_handle().clone();

    let mut state = APP_STATE.lock().unwrap();

    if state.status != TimerStatus::Idle {
        return Err(format!("Cannot start work from {:?} state", state.status));
    }

    state.status = TimerStatus::Working;
    state.break_start_time = None;
    state.skip_attempts = 0;
    state.last_transition = Some(Instant::now());

    // Emit event to frontend
    let _ = app.emit("work_started", serde_json::json!({
        "duration": duration
    }));

    Ok(())
}

/// Complete a work session
///
/// Transitions from Working to Idle state.
#[tauri::command]
pub async fn complete_work(
    window: Window,
) -> Result<(), String> {
    if window.label() != "main" {
        return Err("Unauthorized window".to_string());
    }

    let app: AppHandle = window.app_handle().clone();

    let mut state = APP_STATE.lock().unwrap();

    if state.status != TimerStatus::Working {
        return Err("Not working".to_string());
    }

    state.status = TimerStatus::Idle;
    state.break_start_time = None;
    state.skip_attempts = 0;
    state.last_transition = Some(Instant::now());

    // Emit event to frontend
    let _ = app.emit("work_completed", serde_json::json!({}));

    Ok(())
}

/// Get current timer status
///
/// Allows frontend to query current state for UI synchronization.
#[tauri::command]
pub async fn get_timer_status() -> Result<String, String> {
    let state = APP_STATE.lock().unwrap();
    let status_str = match state.status {
        TimerStatus::Idle => "idle",
        TimerStatus::Working => "working",
        TimerStatus::OnBreak => "on_break",
    };
    Ok(status_str.to_string())
}

/// Record a skip attempt
///
/// Increments skip counter for analytics/penalty calculation.
#[tauri::command]
pub async fn record_skip_attempt() -> Result<u32, String> {
    let mut state = APP_STATE.lock().unwrap();
    if state.status != TimerStatus::OnBreak {
        return Err("Not on break".to_string());
    }
    state.skip_attempts += 1;
    Ok(state.skip_attempts)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_app_state_initial() {
        let state = AppState {
            status: TimerStatus::Idle,
            break_start_time: None,
            skip_attempts: 0,
            last_transition: None,
        };
        assert_eq!(state.status, TimerStatus::Idle);
        assert_eq!(state.break_start_time, None);
        assert_eq!(state.skip_attempts, 0);
    }

    #[test]
    fn test_timer_status_equality() {
        assert_eq!(TimerStatus::Idle, TimerStatus::Idle);
        assert_ne!(TimerStatus::Idle, TimerStatus::Working);
        assert_ne!(TimerStatus::Working, TimerStatus::OnBreak);
    }
}
