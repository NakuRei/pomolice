// Timer commands using dependency injection

use std::sync::atomic::Ordering;

use chrono::{DateTime, Local};
use tauri::{Emitter, Manager, State, WebviewWindow};
use tracing::{info, warn};

use crate::error::{PomoliceError, Result};
use crate::managers::weekly_schedule::{
    find_first_overlapping_weekday_pair, is_now_inside_schedule_window, ActiveWindow,
};
use crate::state::AppState;
use crate::types::{
    ActiveWindowSummaryPayload, BootPromptRuntimeState, FullStatePayload, TimerSettings,
    TimerStartSource, TimerStatus, WeeklyAutoRunConfig,
};

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
    info!(
        "Command: start_work called from window '{}'",
        window.label()
    );

    // Authorization check
    if window.label() != "main" {
        warn!(
            "Unauthorized start_work attempt from window '{}'",
            window.label()
        );
        return Err(PomoliceError::Unauthorized(window.label().to_string()));
    }

    let settings = TimerSettings {
        work_duration_secs,
        short_break_duration_secs,
        long_break_duration_secs,
        long_break_interval,
    };
    let now_local = state.clock.now_local();

    let (auto_run_feature_enabled, weekly_auto_run_config) = {
        let timer = state.timer_manager.lock().await;
        let timer_state = timer.get_state();
        (
            timer_state.auto_run_feature_enabled,
            timer_state.weekly_auto_run_config.clone(),
        )
    };
    let manual_start_source = determine_manual_start_source(
        now_local,
        auto_run_feature_enabled,
        &weekly_auto_run_config,
    );

    // Step 1: Update state and cancel the old task inside the lock.
    // cancel_and_take_task() only signals cancellation (non-blocking),
    // so holding the lock here is safe.
    let (old_task, tick_payload, work_duration) = {
        let mut timer = state.timer_manager.lock().await;
        let old = timer.cancel_and_take_task();
        let (payload, duration) = timer.start_work_with_source(settings, manual_start_source)?;
        (old, payload, duration)
    };

    // Emit initial state and work_started event
    let app = window.app_handle();
    let _ = app.emit("timer_tick", &tick_payload);
    crate::tray::update_tray_icon(app, &tick_payload);
    let _ = app.emit(
        "work_started",
        serde_json::json!({ "duration": work_duration }),
    );

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
    warn!(
        "Command: emergency_stop called from window '{}'",
        window.label()
    );

    if window.label() != "main" {
        warn!(
            "Unauthorized emergency_stop attempt from window '{}'",
            window.label()
        );
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
pub async fn sync_all_settings(
    window: WebviewWindow,
    state: State<'_, AppState>,
    work_duration_secs: u64,
    short_break_duration_secs: u64,
    long_break_duration_secs: u64,
    long_break_interval: u32,
    auto_run_feature_enabled: bool,
    weekly_auto_run_config: WeeklyAutoRunConfig,
) -> Result<()> {
    if window.label() != "main" {
        return Err(PomoliceError::Unauthorized(window.label().to_string()));
    }

    validate_weekly_auto_run_config_for_sync(&weekly_auto_run_config)?;

    let settings = TimerSettings {
        work_duration_secs,
        short_break_duration_secs,
        long_break_duration_secs,
        long_break_interval,
    };

    let tick_payload = {
        let mut timer = state.timer_manager.lock().await;
        timer.sync_all_settings(settings, auto_run_feature_enabled, weekly_auto_run_config)?
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

/// Sync only auto-run settings from frontend
#[tauri::command]
pub async fn sync_auto_run_settings(
    window: WebviewWindow,
    state: State<'_, AppState>,
    auto_run_feature_enabled: bool,
    weekly_auto_run_config: WeeklyAutoRunConfig,
) -> Result<()> {
    if window.label() != "main" {
        return Err(PomoliceError::Unauthorized(window.label().to_string()));
    }

    validate_weekly_auto_run_config_for_sync(&weekly_auto_run_config)?;

    let tick_payload = {
        let mut timer = state.timer_manager.lock().await;
        timer.sync_auto_run_settings(auto_run_feature_enabled, weekly_auto_run_config)
    };

    state.config_revision.fetch_add(1, Ordering::AcqRel);

    let _ = window.app_handle().emit("timer_tick", &tick_payload);
    crate::tray::update_tray_icon(window.app_handle(), &tick_payload);

    {
        let timer = state.timer_manager.lock().await;
        state.persistence.save_state(timer.get_state());
    }

    Ok(())
}

fn to_active_window_summary_payload(active_window: &ActiveWindow) -> ActiveWindowSummaryPayload {
    ActiveWindowSummaryPayload {
        weekday_key: active_window.slot_key.clone(),
        start_time: active_window.start_time.clone(),
        end_time: active_window.end_time.clone(),
        crosses_midnight: active_window.crosses_midnight,
    }
}

fn determine_manual_start_source(
    now_local: DateTime<Local>,
    auto_run_feature_enabled: bool,
    weekly_auto_run_config: &WeeklyAutoRunConfig,
) -> TimerStartSource {
    if auto_run_feature_enabled
        && is_now_inside_schedule_window(now_local, weekly_auto_run_config).is_some()
    {
        TimerStartSource::ManualInsideWindow
    } else {
        TimerStartSource::ManualOutsideWindow
    }
}

fn validate_weekly_auto_run_config_for_sync(
    weekly_auto_run_config: &WeeklyAutoRunConfig,
) -> Result<()> {
    if let Some((primary_weekday, secondary_weekday)) =
        find_first_overlapping_weekday_pair(weekly_auto_run_config)
    {
        return Err(PomoliceError::InvalidAutoRunSchedule {
            reason: format!(
                "overlapping schedule windows between {primary_weekday} and {secondary_weekday}"
            ),
        });
    }

    Ok(())
}

fn evaluate_initial_boot_prompt_decision(
    is_first_get_full_state: bool,
    timer_status: TimerStatus,
    is_inside_schedule_window: bool,
) -> (bool, Option<BootPromptRuntimeState>, bool) {
    if !is_first_get_full_state {
        return (false, None, false);
    }

    if timer_status != TimerStatus::Idle {
        return (false, Some(BootPromptRuntimeState::NotApplicable), true);
    }

    if is_inside_schedule_window {
        return (true, Some(BootPromptRuntimeState::AwaitingDecision), false);
    }

    (false, Some(BootPromptRuntimeState::NotApplicable), true)
}

/// Get full timer state for initial sync
#[tauri::command]
pub async fn get_full_state(state: State<'_, AppState>) -> Result<FullStatePayload> {
    let now_local = state.clock.now_local();
    let is_first_get_full_state = state.boot_prompt_check_pending.swap(false, Ordering::AcqRel);

    let (mut full_state_payload, timer_status, auto_run_feature_enabled, weekly_auto_run_config) = {
        let timer_manager = state.timer_manager.lock().await;
        let timer_state = timer_manager.get_state();
        (
            timer_manager.get_full_state(),
            timer_manager.status(),
            timer_state.auto_run_feature_enabled,
            timer_state.weekly_auto_run_config.clone(),
        )
    };

    let active_window = if auto_run_feature_enabled {
        is_now_inside_schedule_window(now_local, &weekly_auto_run_config)
    } else {
        None
    };

    full_state_payload.active_window_summary =
        active_window.as_ref().map(to_active_window_summary_payload);

    let (should_prompt_start_on_boot, runtime_state_transition, should_enable_auto_start) =
        evaluate_initial_boot_prompt_decision(
            is_first_get_full_state,
            timer_status,
            active_window.is_some(),
        );

    if let Some(next_runtime_state) = runtime_state_transition {
        if let Ok(mut boot_prompt_runtime_state) = state.boot_prompt_runtime_state.lock() {
            *boot_prompt_runtime_state = next_runtime_state;
        }
    }

    if should_enable_auto_start {
        state.auto_start_boundary_enabled.store(true, Ordering::Release);
    }

    full_state_payload.should_prompt_start_on_boot = should_prompt_start_on_boot;
    Ok(full_state_payload)
}

/// Resolve startup auto-start confirmation prompt.
#[tauri::command]
pub async fn resolve_boot_auto_start_prompt(
    window: WebviewWindow,
    state: State<'_, AppState>,
    start_now: bool,
) -> Result<BootPromptRuntimeState> {
    if window.label() != "main" {
        return Err(PomoliceError::Unauthorized(window.label().to_string()));
    }

    let current_runtime_state = match state.boot_prompt_runtime_state.lock() {
        Ok(runtime_state) => *runtime_state,
        Err(_) => BootPromptRuntimeState::NotApplicable,
    };

    let can_resolve_prompt = {
        let timer_manager = state.timer_manager.lock().await;
        timer_manager.status() == TimerStatus::Idle
            && current_runtime_state == BootPromptRuntimeState::AwaitingDecision
    };

    if !can_resolve_prompt {
        if current_runtime_state == BootPromptRuntimeState::AwaitingDecision {
            if let Ok(mut runtime_state) = state.boot_prompt_runtime_state.lock() {
                *runtime_state = BootPromptRuntimeState::NotApplicable;
            }
            state.auto_start_boundary_enabled.store(true, Ordering::Release);
        }
        return Ok(BootPromptRuntimeState::NotApplicable);
    }

    let timer_settings = {
        let timer_manager = state.timer_manager.lock().await;
        timer_manager.get_state().settings.clone()
    };

    if start_now {
        let app = window.app_handle();
        let start_attempt = {
            let mut timer_manager = state.timer_manager.lock().await;
            match timer_manager
                .start_work_with_source(timer_settings, TimerStartSource::ManualInsideWindow)
            {
                Ok((payload, duration)) => {
                    let old = timer_manager.cancel_and_take_task();
                    Ok(Some((old, payload, duration)))
                }
                Err(PomoliceError::InvalidStateTransition { .. }) => Ok(None),
                Err(err) => Err(err),
            }
        }?;

        let Some((old_task, tick_payload, work_duration)) = start_attempt else {
            if let Ok(mut runtime_state) = state.boot_prompt_runtime_state.lock() {
                if *runtime_state == BootPromptRuntimeState::AwaitingDecision {
                    *runtime_state = BootPromptRuntimeState::NotApplicable;
                }
            }
            state.auto_start_boundary_enabled.store(true, Ordering::Release);
            return Ok(BootPromptRuntimeState::NotApplicable);
        };

        let _ = app.emit("timer_tick", &tick_payload);
        crate::tray::update_tray_icon(app, &tick_payload);
        let _ = app.emit(
            "work_started",
            serde_json::json!({ "duration": work_duration }),
        );

        if let Some(handle) = old_task {
            let _ = handle.await;
        }

        {
            let mut timer_manager = state.timer_manager.lock().await;
            timer_manager.spawn_timer_task(app.clone());
        }

        {
            let timer_manager = state.timer_manager.lock().await;
            state.persistence.save_state(timer_manager.get_state());
        }

        if let Ok(mut runtime_state) = state.boot_prompt_runtime_state.lock() {
            *runtime_state = BootPromptRuntimeState::ResolvedStart;
        }
        state.auto_start_boundary_enabled.store(true, Ordering::Release);

        Ok(BootPromptRuntimeState::ResolvedStart)
    } else {
        if let Ok(mut runtime_state) = state.boot_prompt_runtime_state.lock() {
            *runtime_state = BootPromptRuntimeState::ResolvedLater;
        }
        state.auto_start_boundary_enabled.store(true, Ordering::Release);
        Ok(BootPromptRuntimeState::ResolvedLater)
    }
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

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    fn make_enabled_daytime_monday_config() -> WeeklyAutoRunConfig {
        let mut weekly_auto_run_config = WeeklyAutoRunConfig::default();
        weekly_auto_run_config.mon.enabled = true;
        weekly_auto_run_config.mon.start_time = "09:00".to_string();
        weekly_auto_run_config.mon.end_time = "18:00".to_string();
        weekly_auto_run_config
    }

    #[test]
    fn test_determine_manual_start_source_returns_inside_when_enabled_and_in_window() {
        let now_local = Local
            .with_ymd_and_hms(2025, 1, 6, 10, 0, 0)
            .single()
            .expect("local datetime should be valid");
        let weekly_auto_run_config = make_enabled_daytime_monday_config();

        let source = determine_manual_start_source(now_local, true, &weekly_auto_run_config);

        assert_eq!(source, TimerStartSource::ManualInsideWindow);
    }

    #[test]
    fn test_determine_manual_start_source_returns_outside_when_enabled_and_out_of_window() {
        let now_local = Local
            .with_ymd_and_hms(2025, 1, 6, 8, 0, 0)
            .single()
            .expect("local datetime should be valid");
        let weekly_auto_run_config = make_enabled_daytime_monday_config();

        let source = determine_manual_start_source(now_local, true, &weekly_auto_run_config);

        assert_eq!(source, TimerStartSource::ManualOutsideWindow);
    }

    #[test]
    fn test_determine_manual_start_source_returns_outside_when_feature_disabled() {
        let now_local = Local
            .with_ymd_and_hms(2025, 1, 6, 10, 0, 0)
            .single()
            .expect("local datetime should be valid");
        let weekly_auto_run_config = make_enabled_daytime_monday_config();

        let source = determine_manual_start_source(now_local, false, &weekly_auto_run_config);

        assert_eq!(source, TimerStartSource::ManualOutsideWindow);
    }

    #[test]
    fn test_evaluate_initial_boot_prompt_decision_returns_awaiting_on_first_idle_inside_window() {
        let result = evaluate_initial_boot_prompt_decision(true, TimerStatus::Idle, true);
        assert_eq!(
            result,
            (true, Some(BootPromptRuntimeState::AwaitingDecision), false)
        );
    }

    #[test]
    fn test_evaluate_initial_boot_prompt_decision_returns_not_applicable_on_first_non_idle() {
        let result = evaluate_initial_boot_prompt_decision(true, TimerStatus::Working, true);
        assert_eq!(
            result,
            (false, Some(BootPromptRuntimeState::NotApplicable), true)
        );
    }

    #[test]
    fn test_evaluate_initial_boot_prompt_decision_returns_not_applicable_on_first_idle_outside_window(
    ) {
        let result = evaluate_initial_boot_prompt_decision(true, TimerStatus::Idle, false);
        assert_eq!(
            result,
            (false, Some(BootPromptRuntimeState::NotApplicable), true)
        );
    }

    #[test]
    fn test_evaluate_initial_boot_prompt_decision_returns_no_transition_after_first_call() {
        let result = evaluate_initial_boot_prompt_decision(false, TimerStatus::Idle, true);
        assert_eq!(result, (false, None, false));
    }

    #[test]
    fn test_evaluate_initial_boot_prompt_decision_prompts_only_on_first_call() {
        let first_result = evaluate_initial_boot_prompt_decision(true, TimerStatus::Idle, true);
        let second_result = evaluate_initial_boot_prompt_decision(false, TimerStatus::Idle, true);

        assert_eq!(
            first_result,
            (true, Some(BootPromptRuntimeState::AwaitingDecision), false)
        );
        assert_eq!(second_result, (false, None, false));
    }

    #[test]
    fn test_validate_weekly_auto_run_config_for_sync_rejects_overlapping_windows() {
        let mut weekly_auto_run_config = WeeklyAutoRunConfig::default();
        weekly_auto_run_config.mon.enabled = true;
        weekly_auto_run_config.mon.start_time = "22:00".to_string();
        weekly_auto_run_config.mon.end_time = "02:00".to_string();
        weekly_auto_run_config.tue.enabled = true;
        weekly_auto_run_config.tue.start_time = "01:00".to_string();
        weekly_auto_run_config.tue.end_time = "05:00".to_string();

        let validation_result = validate_weekly_auto_run_config_for_sync(&weekly_auto_run_config);
        assert!(matches!(
            validation_result,
            Err(PomoliceError::InvalidAutoRunSchedule { .. })
        ));
    }

    #[test]
    fn test_validate_weekly_auto_run_config_for_sync_accepts_non_overlapping_windows() {
        let mut weekly_auto_run_config = WeeklyAutoRunConfig::default();
        weekly_auto_run_config.mon.enabled = true;
        weekly_auto_run_config.mon.start_time = "09:00".to_string();
        weekly_auto_run_config.mon.end_time = "12:00".to_string();
        weekly_auto_run_config.tue.enabled = true;
        weekly_auto_run_config.tue.start_time = "13:00".to_string();
        weekly_auto_run_config.tue.end_time = "18:00".to_string();

        let validation_result = validate_weekly_auto_run_config_for_sync(&weekly_auto_run_config);
        assert!(validation_result.is_ok());
    }
}
