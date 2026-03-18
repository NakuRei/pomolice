// Pomolice - Pomodoro Timer Application
// Backend is the single source of truth for all timer state
// Security: Window control is managed through timer_state module

use chrono::{DateTime, Duration as ChronoDuration, Local};
use std::collections::HashSet;
use std::sync::atomic::Ordering;
use std::time::Duration;
use tauri::{AppHandle, Emitter, Manager};
use tokio::time::MissedTickBehavior;
use tokio_util::sync::CancellationToken;
use tracing::{error, info};

mod commands;
mod error;
mod managers;
mod state;
mod tray;
mod tray_icon_renderer;
mod types;

use crate::managers::weekly_schedule::{
    collect_crossed_end_boundaries, collect_crossed_start_boundaries,
};
use crate::types::{BootPromptRuntimeState, BoundaryOccurrenceId, TimerStartSource, TimerStatus};

const SCHEDULER_INTERVAL_SECONDS: u64 = 1;
const PROCESSED_BOUNDARY_RETENTION_DAYS: i64 = 8;

struct SchedulerRuntimeState {
    processed_boundary_occurrences: HashSet<BoundaryOccurrenceId>,
    last_checked_local: DateTime<Local>,
    start_boundaries_blocked_until_idle: bool,
    last_seen_config_revision: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SessionCompletionEmitEvent {
    SessionCompleted,
    AutoRunStopped,
    TimerTick,
}

const SESSION_COMPLETION_EMIT_ORDER_DEFAULT: [SessionCompletionEmitEvent; 2] = [
    SessionCompletionEmitEvent::SessionCompleted,
    SessionCompletionEmitEvent::TimerTick,
];

const SESSION_COMPLETION_EMIT_ORDER_SCHEDULED_STOP: [SessionCompletionEmitEvent; 3] = [
    SessionCompletionEmitEvent::SessionCompleted,
    SessionCompletionEmitEvent::AutoRunStopped,
    SessionCompletionEmitEvent::TimerTick,
];

fn session_completion_emit_order(
    transition_reason: types::SessionTransitionReason,
) -> &'static [SessionCompletionEmitEvent] {
    match transition_reason {
        types::SessionTransitionReason::ScheduledStopExecuted => {
            &SESSION_COMPLETION_EMIT_ORDER_SCHEDULED_STOP
        }
        _ => &SESSION_COMPLETION_EMIT_ORDER_DEFAULT,
    }
}

/// Handle timer tick event
pub async fn lib_handle_timer_tick(app: AppHandle) {
    let state_handle = app.state::<state::AppState>();

    let (should_complete_session, tick_payload) = {
        let mut timer = state_handle.timer_manager.lock().await;

        // Only tick if running
        if timer.status() == types::TimerStatus::Idle {
            return;
        }

        let (completed, payload) = timer.tick();

        (completed, payload)
    };

    // Emit tick event to frontend
    let _ = app.emit("timer_tick", &tick_payload);
    tray::update_tray_icon(&app, &tick_payload);

    if should_complete_session {
        // Awaiting this transition blocks the current timer-task tick loop until
        // completion handling finishes, preventing overlap with the next tick.
        lib_handle_session_complete(app).await;
    }
}

/// Handle session completion
pub async fn lib_handle_session_complete(app: AppHandle) {
    use types::{SessionTransitionOutcome, SessionTransitionReason, SessionType};

    let state_handle = app.state::<state::AppState>();
    let (
        completed_payload,
        transition_outcome,
        requires_enter_break_mode,
        requires_exit_break_mode,
    ) = {
        let mut timer = state_handle.timer_manager.lock().await;
        let completed_payload = match timer.get_state().session_type {
            SessionType::Work => timer.complete_work_session_transition(),
            SessionType::ShortBreak | SessionType::LongBreak => {
                timer.complete_break_session_transition()
            }
        };

        let transition_outcome = SessionTransitionOutcome {
            transition_reason: completed_payload.transition_reason,
            completed_session_type: completed_payload.completed_session_type,
            next_status: completed_payload.next_status,
            next_session_type: completed_payload.next_session_type,
            requires_manual_continue_decision: completed_payload.requires_manual_continue_decision,
        };

        if matches!(
            transition_outcome.transition_reason,
            SessionTransitionReason::BreakWaitingDecision
                | SessionTransitionReason::ScheduledStopExecuted
        ) {
            // NOTE: this path runs inside the timer task. We only cancel and
            // drop the JoinHandle here. Awaiting it here would self-join.
            let _ = timer.cancel_and_take_task();
        }

        let requires_enter_break_mode =
            transition_outcome.transition_reason == SessionTransitionReason::WorkToBreak;
        let requires_exit_break_mode = matches!(
            transition_outcome.completed_session_type,
            SessionType::ShortBreak | SessionType::LongBreak
        );

        (
            completed_payload,
            transition_outcome,
            requires_enter_break_mode,
            requires_exit_break_mode,
        )
    };

    debug_assert!(
        !(requires_enter_break_mode && requires_exit_break_mode),
        "session transition cannot enter and exit fullscreen break mode at the same time"
    );

    if requires_enter_break_mode {
        enter_break_mode(app.clone()).await;
    } else if requires_exit_break_mode {
        exit_break_mode(app.clone()).await;
    }

    let (
        final_tick_payload,
        final_post_break_transition_policy,
        final_post_break_decision_pending,
        final_scheduled_stop_state,
        final_timer_start_source,
    ) = {
        let timer = state_handle.timer_manager.lock().await;
        let state = timer.get_state();
        (
            state.to_tick_payload(),
            state.settings.post_break_transition_policy,
            state.post_break_decision_pending,
            state.scheduled_stop_state,
            state.timer_start_source,
        )
    };

    info!(
        transition_reason = ?transition_outcome.transition_reason,
        post_break_transition_policy = ?final_post_break_transition_policy,
        post_break_decision_pending = final_post_break_decision_pending,
        scheduled_stop_state = ?final_scheduled_stop_state,
        timer_start_source = ?final_timer_start_source,
        "Session completion pipeline finalized transition state"
    );

    for emit_event in session_completion_emit_order(transition_outcome.transition_reason) {
        match emit_event {
            SessionCompletionEmitEvent::SessionCompleted => {
                let _ = app.emit("session_completed", &completed_payload);
            }
            SessionCompletionEmitEvent::AutoRunStopped => {
                let _ = app.emit("auto_run_stopped", serde_json::json!({}));
            }
            SessionCompletionEmitEvent::TimerTick => {
                // Emit exactly one final timer_tick for every session-boundary transition.
                let _ = app.emit("timer_tick", &final_tick_payload);
                tray::update_tray_icon(&app, &final_tick_payload);
            }
        }
    }

    {
        let timer = state_handle.timer_manager.lock().await;
        state_handle.persistence.save_state(timer.get_state());
    }
}

/// Enter fullscreen break mode
async fn enter_break_mode(app: AppHandle) {
    info!("Entering fullscreen break mode");
    let state_handle = app.state::<state::AppState>();

    // Start input monitoring
    {
        let mut input_monitor = state_handle.input_monitor.lock().await;
        if let Err(e) = input_monitor.start() {
            error!("Failed to start input monitoring: {:?}", e);
        } else {
            info!("Input monitoring started successfully");
        }
    }

    // Enter fullscreen on all monitors
    {
        let mut window_manager = state_handle.window_manager.lock().await;
        if let Err(e) = window_manager
            .enter_fullscreen_all_monitors(app.clone())
            .await
        {
            error!("Failed to enter fullscreen: {:?}", e);
            return;
        }
        info!("Entered fullscreen mode on all monitors");
    }

    // Update state
    {
        let mut timer = state_handle.timer_manager.lock().await;
        timer.set_fullscreen_break(true);
    }

    let _ = app.emit("break_started", serde_json::json!({}));
}

/// Exit fullscreen break mode
async fn exit_break_mode(app: AppHandle) {
    info!("Exiting fullscreen break mode");
    let state_handle = app.state::<state::AppState>();

    // Stop input monitoring
    {
        let mut input_monitor = state_handle.input_monitor.lock().await;
        if let Err(e) = input_monitor.stop() {
            error!("Failed to stop input monitoring: {:?}", e);
        } else {
            info!("Input monitoring stopped successfully");
        }
    }

    // Exit fullscreen on all monitors
    {
        let mut window_manager = state_handle.window_manager.lock().await;
        if let Err(e) = window_manager
            .exit_fullscreen_all_monitors(app.clone())
            .await
        {
            error!("Failed to exit fullscreen: {:?}", e);
            return;
        }
        info!("Exited fullscreen mode on all monitors");
    }

    // Update state and notify frontend that fullscreen break ended.
    {
        let mut timer = state_handle.timer_manager.lock().await;
        timer.set_fullscreen_break(false);
    }

    let _ = app.emit("break_completed", serde_json::json!({}));
}

/// Handle input detected during break
pub async fn lib_handle_input_detected(app: AppHandle) {
    let state_handle = app.state::<state::AppState>();

    let tick_payload = {
        let mut timer = state_handle.timer_manager.lock().await;
        let payload = timer.apply_input_penalty().ok().flatten();
        // Save state
        state_handle.persistence.save_state(timer.get_state());
        payload
    };

    if let Some(ref payload) = tick_payload {
        let _ = app.emit("timer_tick", payload);
        tray::update_tray_icon(&app, payload);
    }
}

/// Handle escape combo detection
pub async fn lib_handle_escape_combo(app: AppHandle) {
    let state_handle = app.state::<state::AppState>();

    // Lock ordering: input_monitor first, then window_manager (matches exit_break_mode)
    {
        let mut input_monitor = state_handle.input_monitor.lock().await;
        let _ = input_monitor.stop();
    }

    {
        let mut window_manager = state_handle.window_manager.lock().await;
        let _ = window_manager
            .exit_fullscreen_all_monitors(app.clone())
            .await;
    }
}

/// Start the weekly scheduler background task.
pub async fn lib_start_scheduler_task(app: AppHandle) {
    let state_handle = app.state::<state::AppState>();
    state_handle.stop_scheduler_task();

    let now_local = state_handle.clock.now_local();
    let startup_timer_status = {
        let timer_manager = state_handle.timer_manager.lock().await;
        timer_manager.status()
    };

    let mut start_boundaries_blocked_until_idle = false;
    let mut initial_previous_local = now_local;

    if matches!(
        startup_timer_status,
        TimerStatus::Working | TimerStatus::OnBreak
    ) {
        start_boundaries_blocked_until_idle = true;
        initial_previous_local = state_handle
            .restored_running_saved_at_local
            .unwrap_or(now_local - ChronoDuration::days(7));
    }

    let cancellation_token = CancellationToken::new();
    let scheduler_cancellation_token = cancellation_token.clone();
    let scheduler_app = app.clone();

    let scheduler_join_handle = tauri::async_runtime::spawn(async move {
        let mut scheduler_interval =
            tokio::time::interval(Duration::from_secs(SCHEDULER_INTERVAL_SECONDS));
        scheduler_interval.set_missed_tick_behavior(MissedTickBehavior::Skip);

        let state_handle = scheduler_app.state::<state::AppState>();
        let mut runtime_state = SchedulerRuntimeState {
            processed_boundary_occurrences: HashSet::new(),
            last_checked_local: initial_previous_local,
            start_boundaries_blocked_until_idle,
            last_seen_config_revision: state_handle.config_revision.load(Ordering::Acquire),
        };

        info!(
            "Weekly scheduler started (previous_local: {}, blocked_until_idle: {})",
            runtime_state.last_checked_local, runtime_state.start_boundaries_blocked_until_idle
        );

        loop {
            tokio::select! {
                _ = scheduler_cancellation_token.cancelled() => {
                    info!("Weekly scheduler received cancellation");
                    break;
                }
                _ = scheduler_interval.tick() => {
                    lib_run_scheduler_tick(scheduler_app.clone(), &mut runtime_state).await;
                }
            }
        }

        info!("Weekly scheduler stopped");
    });

    if let Ok(mut scheduler_task_state) = state_handle.scheduler_task_state.lock() {
        scheduler_task_state.cancellation_token = Some(cancellation_token);
        scheduler_task_state.join_handle = Some(scheduler_join_handle);
    };
}

async fn lib_run_scheduler_tick(app: AppHandle, runtime_state: &mut SchedulerRuntimeState) {
    let state_handle = app.state::<state::AppState>();
    let now_local = state_handle.clock.now_local();
    let previous_local = runtime_state.last_checked_local;

    let timer_snapshot = {
        let timer_manager = state_handle.timer_manager.lock().await;
        let timer_state = timer_manager.get_state();
        (
            timer_manager.status(),
            timer_state.auto_run_feature_enabled,
            timer_state.weekly_auto_run_config.clone(),
            timer_state.settings.clone(),
        )
    };
    let (timer_status, auto_run_feature_enabled, weekly_auto_run_config, timer_settings) =
        timer_snapshot;

    if runtime_state.start_boundaries_blocked_until_idle && timer_status == TimerStatus::Idle {
        runtime_state.start_boundaries_blocked_until_idle = false;
        info!("Weekly scheduler resumed start-boundary evaluation after idle transition");
    }

    let config_revision = state_handle.config_revision.load(Ordering::Acquire);
    let config_changed = config_revision != runtime_state.last_seen_config_revision;
    let mut skip_start_boundaries_this_tick = false;

    let end_boundary_previous_local = if config_changed {
        runtime_state.processed_boundary_occurrences.clear();
        runtime_state.last_checked_local = now_local;
        runtime_state.last_seen_config_revision = config_revision;
        skip_start_boundaries_this_tick = true;
        info!(
            "Weekly scheduler detected config revision change: {}",
            config_revision
        );
        now_local - ChronoDuration::seconds(1)
    } else {
        previous_local
    };

    if auto_run_feature_enabled {
        let crossed_end_boundaries = collect_crossed_end_boundaries(
            end_boundary_previous_local,
            now_local,
            &weekly_auto_run_config,
        );

        for end_boundary in crossed_end_boundaries {
            if runtime_state
                .processed_boundary_occurrences
                .insert(end_boundary.clone())
            {
                info!("Weekly scheduler crossed end boundary: {}", end_boundary.0);
                let did_mark_stop_pending =
                    mark_scheduled_stop_pending_for_current_session(app.clone()).await;
                if did_mark_stop_pending {
                    info!(
                        "Weekly scheduler marked scheduled stop as pending at end boundary: {}",
                        end_boundary.0
                    );
                }
            }
        }
    }

    let auto_start_boundary_enabled = state_handle
        .auto_start_boundary_enabled
        .load(Ordering::Acquire);
    let boot_prompt_check_pending = state_handle
        .boot_prompt_check_pending
        .load(Ordering::Acquire);
    let boot_prompt_runtime_state = match state_handle.boot_prompt_runtime_state.lock() {
        Ok(guard) => *guard,
        Err(_) => BootPromptRuntimeState::NotApplicable,
    };

    let can_run_start_boundary_evaluation = auto_run_feature_enabled
        && auto_start_boundary_enabled
        && !boot_prompt_check_pending
        && boot_prompt_runtime_state != BootPromptRuntimeState::AwaitingDecision
        && !runtime_state.start_boundaries_blocked_until_idle
        && !skip_start_boundaries_this_tick;

    if can_run_start_boundary_evaluation {
        let crossed_start_boundaries =
            collect_crossed_start_boundaries(previous_local, now_local, &weekly_auto_run_config);

        for start_boundary in crossed_start_boundaries {
            if runtime_state
                .processed_boundary_occurrences
                .insert(start_boundary.clone())
            {
                info!(
                    "Weekly scheduler crossed start boundary: {}",
                    start_boundary.0
                );
                let did_start = start_work_from_scheduler_when_idle(
                    app.clone(),
                    timer_status,
                    timer_settings.clone(),
                    &start_boundary,
                )
                .await;

                if did_start {
                    info!(
                        "Weekly scheduler started a work session from start boundary: {}",
                        start_boundary.0
                    );
                }
            }
        }
    }

    trim_old_processed_boundaries(&mut runtime_state.processed_boundary_occurrences, now_local);
    runtime_state.last_checked_local = now_local;
}

fn trim_old_processed_boundaries(
    processed_boundary_occurrences: &mut HashSet<BoundaryOccurrenceId>,
    now_local: DateTime<Local>,
) {
    let oldest_epoch_to_keep =
        (now_local - ChronoDuration::days(PROCESSED_BOUNDARY_RETENTION_DAYS)).timestamp();

    processed_boundary_occurrences.retain(|boundary_id| {
        extract_boundary_epoch_seconds(boundary_id)
            .map(|epoch_seconds| epoch_seconds >= oldest_epoch_to_keep)
            .unwrap_or(false)
    });
}

fn extract_boundary_epoch_seconds(boundary_id: &BoundaryOccurrenceId) -> Option<i64> {
    boundary_id.0.rsplit('|').next()?.parse::<i64>().ok()
}

async fn mark_scheduled_stop_pending_for_current_session(app: AppHandle) -> bool {
    let state_handle = app.state::<state::AppState>();

    let should_persist = {
        let mut timer_manager = state_handle.timer_manager.lock().await;
        timer_manager.mark_scheduled_stop_pending_if_session_is_eligible()
    };

    if should_persist {
        let _ = app.emit("auto_run_stop_pending", serde_json::json!({}));
        {
            let timer_manager = state_handle.timer_manager.lock().await;
            state_handle
                .persistence
                .save_state(timer_manager.get_state());
        }
    }

    should_persist
}

async fn start_work_from_scheduler_when_idle(
    app: AppHandle,
    timer_status_at_tick_start: TimerStatus,
    timer_settings: types::TimerSettings,
    start_boundary: &BoundaryOccurrenceId,
) -> bool {
    if timer_status_at_tick_start != TimerStatus::Idle {
        return false;
    }

    let state_handle = app.state::<state::AppState>();

    let (old_timer_task, tick_payload, work_duration_secs) = {
        let mut timer_manager = state_handle.timer_manager.lock().await;
        if timer_manager.status() != TimerStatus::Idle {
            return false;
        }

        let old_task = timer_manager.cancel_and_take_task();
        let Ok((payload, duration_secs)) = timer_manager
            .start_work_with_source(timer_settings, TimerStartSource::ScheduledAutoStart)
        else {
            info!(
                "Weekly scheduler skipped auto-start due to start_work rejection at boundary {}",
                start_boundary.0
            );
            return false;
        };
        (old_task, payload, duration_secs)
    };

    let _ = app.emit("timer_tick", &tick_payload);
    tray::update_tray_icon(&app, &tick_payload);
    let _ = app.emit(
        "work_started",
        serde_json::json!({ "duration": work_duration_secs }),
    );

    if let Some(join_handle) = old_timer_task {
        let _ = join_handle.await;
    }

    {
        let mut timer_manager = state_handle.timer_manager.lock().await;
        timer_manager.spawn_timer_task(app.clone());
    }

    {
        let timer_manager = state_handle.timer_manager.lock().await;
        state_handle
            .persistence
            .save_state(timer_manager.get_state());
    }

    true
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // Initialize tracing subscriber for structured logging
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .with_target(false)
        .with_thread_ids(false)
        .with_file(true)
        .with_line_number(true)
        .init();

    info!("Starting Pomolice application");

    tauri::Builder::default()
        .plugin(tauri_plugin_notification::init())
        .setup(|app| {
            info!("Initializing application state");
            // Create and register application state
            let app_state = state::AppState::new(app.handle().clone());

            // Initialize state (handles persisted state restoration)
            tauri::async_runtime::block_on(async {
                let tick_payload = app_state
                    .initialize()
                    .await
                    .expect("Failed to initialize application state");

                // Emit restored state to frontend if the timer was running when app closed
                if let Some(payload) = tick_payload {
                    let _ = app.emit("timer_tick", &payload);
                }
            });

            // Register state with Tauri
            app.manage(app_state);

            // Initialize system tray icon
            if let Err(e) = tray::setup_tray(app.handle()) {
                error!("Failed to setup tray icon: {:?}", e);
            }

            // Start weekly scheduler after initialization and state registration.
            let scheduler_app_handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                lib_start_scheduler_task(scheduler_app_handle).await;
            });

            Ok(())
        })
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                if window.label() == "main" {
                    // Hide to tray instead of closing
                    api.prevent_close();
                    let _ = window.hide();

                    // Show a notification the first time the window is minimized to tray
                    if tray::should_show_tray_notification() {
                        info!("Window minimized to tray (first time notification)");
                        use tauri_plugin_notification::NotificationExt;
                        let _ = window
                            .app_handle()
                            .notification()
                            .builder()
                            .title("Pomolice")
                            .body("Pomolice is still running in the system tray.")
                            .show();
                    }
                }
            }
        })
        .invoke_handler(tauri::generate_handler![
            // New commands using dependency injection
            commands::timer::start_work,
            commands::timer::resolve_post_break_decision,
            commands::timer::emergency_stop,
            commands::timer::sync_timer_duration_settings,
            commands::timer::sync_automation_policies,
            commands::timer::get_full_state,
            commands::timer::resolve_boot_auto_start_prompt,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

#[cfg(test)]
mod emit_order_tests {
    use super::{
        session_completion_emit_order, SessionCompletionEmitEvent,
        SESSION_COMPLETION_EMIT_ORDER_DEFAULT, SESSION_COMPLETION_EMIT_ORDER_SCHEDULED_STOP,
    };
    use crate::types::SessionTransitionReason;

    #[test]
    fn test_session_completion_emit_order_for_non_scheduled_stop() {
        let emit_order = session_completion_emit_order(SessionTransitionReason::BreakAutoStarted);
        assert_eq!(emit_order, &SESSION_COMPLETION_EMIT_ORDER_DEFAULT);
        assert_eq!(
            emit_order,
            &[
                SessionCompletionEmitEvent::SessionCompleted,
                SessionCompletionEmitEvent::TimerTick,
            ]
        );
    }

    #[test]
    fn test_session_completion_emit_order_for_scheduled_stop() {
        let emit_order =
            session_completion_emit_order(SessionTransitionReason::ScheduledStopExecuted);
        assert_eq!(emit_order, &SESSION_COMPLETION_EMIT_ORDER_SCHEDULED_STOP);
        assert_eq!(
            emit_order,
            &[
                SessionCompletionEmitEvent::SessionCompleted,
                SessionCompletionEmitEvent::AutoRunStopped,
                SessionCompletionEmitEvent::TimerTick,
            ]
        );
    }
}

#[cfg(test)]
mod tests;
