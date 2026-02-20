// Timer commands using dependency injection

use tauri::{Manager, State, WebviewWindow};
use tracing::{info, warn};

use crate::error::{PomoliceError, Result};
use crate::state::AppState;
use crate::types::{FullStatePayload, TimerSettings};

/// Start a work session with settings
#[tauri::command]
pub async fn start_work(
    window: WebviewWindow,
    state: State<'_, AppState>,
    work_duration_secs: u64,
    short_break_duration_secs: u64,
    long_break_duration_secs: u64,
    long_break_interval: u32,
) -> Result<()> {
    info!("Command: start_work called from window '{}'", window.label());

    // Authorization check
    if window.label() != "main" {
        warn!("Unauthorized start_work attempt from window '{}'", window.label());
        return Err(PomoliceError::Unauthorized(window.label().to_string()));
    }

    let settings = TimerSettings {
        work_duration_secs,
        short_break_duration_secs,
        long_break_duration_secs,
        long_break_interval,
    };

    // Start work session
    {
        let mut timer = state.timer_manager.lock().await;
        timer.start_work(settings)?;
    }

    // Save state to persistence
    {
        let timer = state.timer_manager.lock().await;
        state.persistence.save_state(timer.get_state());
    }

    Ok(())
}

/// Continue to next work session after break
#[tauri::command]
pub async fn continue_to_next_session(
    window: WebviewWindow,
    state: State<'_, AppState>,
) -> Result<()> {
    if window.label() != "main" {
        return Err(PomoliceError::Unauthorized(window.label().to_string()));
    }

    {
        let mut timer = state.timer_manager.lock().await;
        timer.continue_to_next_session()?;
    }

    // Save state
    {
        let timer = state.timer_manager.lock().await;
        state.persistence.save_state(timer.get_state());
    }

    Ok(())
}

/// End the current cycle (from continue dialog)
#[tauri::command]
pub async fn end_cycle(window: WebviewWindow, state: State<'_, AppState>) -> Result<()> {
    if window.label() != "main" {
        return Err(PomoliceError::Unauthorized(window.label().to_string()));
    }

    {
        let mut timer = state.timer_manager.lock().await;
        timer.end_cycle()?;
    }

    // Save state
    {
        let timer = state.timer_manager.lock().await;
        state.persistence.save_state(timer.get_state());
    }

    Ok(())
}

/// Emergency stop - reset all state
#[tauri::command]
pub async fn emergency_stop(window: WebviewWindow, state: State<'_, AppState>) -> Result<()> {
    warn!("Command: emergency_stop called from window '{}'", window.label());

    if window.label() != "main" {
        warn!("Unauthorized emergency_stop attempt from window '{}'", window.label());
        return Err(PomoliceError::Unauthorized(window.label().to_string()));
    }

    let was_fullscreen_break = {
        let mut timer = state.timer_manager.lock().await;
        timer.emergency_stop()?
    };

    // Exit fullscreen if needed
    if was_fullscreen_break {
        let app_handle = window.app_handle().clone();
        let window_manager = state.window_manager.clone();

        tauri::async_runtime::spawn(async move {
            let mut mgr = window_manager.lock().await;
            let _ = mgr.exit_fullscreen_all_monitors(app_handle).await;
        });
    }

    // Save state
    {
        let timer = state.timer_manager.lock().await;
        state.persistence.save_state(timer.get_state());
    }

    Ok(())
}

/// Sync settings from frontend
#[tauri::command]
pub async fn sync_settings(
    window: WebviewWindow,
    state: State<'_, AppState>,
    work_duration_secs: u64,
    short_break_duration_secs: u64,
    long_break_duration_secs: u64,
    long_break_interval: u32,
) -> Result<()> {
    if window.label() != "main" {
        return Err(PomoliceError::Unauthorized(window.label().to_string()));
    }

    let settings = TimerSettings {
        work_duration_secs,
        short_break_duration_secs,
        long_break_duration_secs,
        long_break_interval,
    };

    {
        let mut timer = state.timer_manager.lock().await;
        timer.sync_settings(settings)?;
    }

    // Save state
    {
        let timer = state.timer_manager.lock().await;
        state.persistence.save_state(timer.get_state());
    }

    Ok(())
}

/// Get full timer state for initial sync
#[tauri::command]
pub async fn get_full_state(state: State<'_, AppState>) -> Result<FullStatePayload> {
    let timer = state.timer_manager.lock().await;
    Ok(timer.get_full_state())
}

/// Record a skip attempt
#[tauri::command]
pub async fn record_skip_attempt(state: State<'_, AppState>) -> Result<u32> {
    let mut timer = state.timer_manager.lock().await;
    timer.record_skip_attempt()
}

/// Get current timer status (legacy compatibility)
#[tauri::command]
pub async fn get_timer_status(state: State<'_, AppState>) -> Result<String> {
    let timer = state.timer_manager.lock().await;
    let status_str = match timer.status() {
        crate::types::TimerStatus::Idle => "idle",
        crate::types::TimerStatus::Working => "working",
        crate::types::TimerStatus::OnBreak => "on_break",
    };
    Ok(status_str.to_string())
}
