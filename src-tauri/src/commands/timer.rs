// Timer commands using dependency injection

use tauri::{Emitter, Manager, State, WebviewWindow};
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

    // Step 1: Update state and cancel the old task inside the lock.
    // cancel_and_take_task() only signals cancellation (non-blocking),
    // so holding the lock here is safe.
    let (old_task, tick_payload, work_duration) = {
        let mut timer = state.timer_manager.lock().await;
        let old = timer.cancel_and_take_task();
        let (payload, duration) = timer.start_work(settings)?;
        (old, payload, duration)
    };

    // Emit initial state and work_started event
    let app = window.app_handle();
    let _ = app.emit("timer_tick", &tick_payload);
    crate::tray::update_tray_icon(app, &tick_payload);
    let _ = app.emit("work_started", serde_json::json!({ "duration": work_duration }));

    // Step 2: Await the old task outside the lock.
    // This ensures the old tick loop has fully stopped before the new one starts,
    // preventing a residual tick from running concurrently with the new session.
    if let Some(handle) = old_task {
        let _ = handle.await;
    }

    // Step 3: Spawn the new timer task inside a fresh lock.
    {
        let mut timer = state.timer_manager.lock().await;
        timer.spawn_timer_task(app.clone());
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

    let (old_task, tick_payload) = {
        let mut timer = state.timer_manager.lock().await;
        let old = timer.cancel_and_take_task();
        let payload = timer.continue_to_next_session()?;
        (old, payload)
    };

    let app = window.app_handle();
    let _ = app.emit("timer_tick", &tick_payload);
    crate::tray::update_tray_icon(app, &tick_payload);

    if let Some(handle) = old_task {
        let _ = handle.await;
    }

    {
        let mut timer = state.timer_manager.lock().await;
        timer.spawn_timer_task(app.clone());
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

    let tick_payload = {
        let mut timer = state.timer_manager.lock().await;
        timer.end_cycle()?
    };

    let _ = window.app_handle().emit("timer_tick", &tick_payload);
    crate::tray::update_tray_icon(window.app_handle(), &tick_payload);

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

    let (was_fullscreen_break, tick_payload, old_task) = {
        let mut timer = state.timer_manager.lock().await;
        let old = timer.cancel_and_take_task();
        let (was_fullscreen, payload) = timer.emergency_stop()?;
        (was_fullscreen, payload, old)
    };

    let _ = window.app_handle().emit("timer_tick", &tick_payload);
    crate::tray::update_tray_icon(window.app_handle(), &tick_payload);

    if let Some(handle) = old_task {
        let _ = handle.await;
    }

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

    let tick_payload = {
        let mut timer = state.timer_manager.lock().await;
        timer.sync_settings(settings)?
    };

    let _ = window.app_handle().emit("timer_tick", &tick_payload);
    crate::tray::update_tray_icon(window.app_handle(), &tick_payload);

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
