use chrono::{DateTime, Duration as ChronoDuration, Local};
use std::collections::HashSet;
use std::sync::atomic::Ordering;
use std::time::Duration;
use tauri::{AppHandle, Emitter, Manager};
use tokio::time::MissedTickBehavior;
use tokio_util::sync::CancellationToken;

use crate::logging_event::{
    log_info_event, log_persistence_write_failure, log_warn_event, BackendLogEvent,
    PERSISTENCE_WRITE_TRIGGER_SCHEDULER_AUTO_START,
    PERSISTENCE_WRITE_TRIGGER_SCHEDULER_STOP_PENDING,
};
use crate::managers::weekly_schedule::{
    collect_crossed_end_boundaries, collect_crossed_start_boundaries,
};
use crate::state;
use crate::tray;
use crate::types::{
    BootPromptRuntimeState, BoundaryOccurrenceId, TimerSettings, TimerStartSource, TimerStatus,
    WeeklyAutoRunConfig,
};

const SCHEDULER_INTERVAL_SECONDS: u64 = 1;
const PROCESSED_BOUNDARY_RETENTION_DAYS: i64 = 8;

struct SchedulerRuntimeState {
    processed_boundary_occurrences: HashSet<BoundaryOccurrenceId>,
    last_checked_local: DateTime<Local>,
    start_boundaries_blocked_until_idle: bool,
    last_seen_config_revision: u64,
}

struct SchedulerTimerSnapshot {
    status: TimerStatus,
    auto_run_feature_enabled: bool,
    weekly_auto_run_config: WeeklyAutoRunConfig,
    timer_settings: TimerSettings,
}

struct SchedulerTickPlan {
    end_boundary_previous_local: DateTime<Local>,
    should_evaluate_start_boundaries: bool,
}

async fn persist_current_state_with_trigger(app: &AppHandle, trigger: &'static str) {
    let state_handle = app.state::<state::AppState>();

    if let Err(error) = state_handle.persist_current_state_async().await {
        log_persistence_write_failure(trigger, &error);
    }
}

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
            .restored_running_saved_at_local()
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

        log_info_event(
            BackendLogEvent::SchedulerStarted,
            &[
                (
                    "previous_local",
                    runtime_state.last_checked_local.to_rfc3339(),
                ),
                (
                    "blocked_until_idle",
                    runtime_state
                        .start_boundaries_blocked_until_idle
                        .to_string(),
                ),
            ],
        );

        loop {
            tokio::select! {
                () = scheduler_cancellation_token.cancelled() => {
                    log_info_event(BackendLogEvent::SchedulerReceivedCancellation, &[]);
                    break;
                }
                _ = scheduler_interval.tick() => {
                    lib_run_scheduler_tick(scheduler_app.clone(), &mut runtime_state).await;
                }
            }
        }

        log_info_event(BackendLogEvent::SchedulerStopped, &[]);
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

    let timer_snapshot = load_scheduler_timer_snapshot(&state_handle).await;
    resume_start_boundary_evaluation_when_idle(runtime_state, timer_snapshot.status);

    let tick_plan = build_scheduler_tick_plan(
        &state_handle,
        runtime_state,
        now_local,
        timer_snapshot.auto_run_feature_enabled,
    );

    process_crossed_end_boundaries(
        &app,
        runtime_state,
        &timer_snapshot.weekly_auto_run_config,
        timer_snapshot.auto_run_feature_enabled,
        tick_plan.end_boundary_previous_local,
        now_local,
    )
    .await;

    if tick_plan.should_evaluate_start_boundaries {
        process_crossed_start_boundaries(
            &app,
            runtime_state,
            &timer_snapshot,
            previous_local,
            now_local,
        )
        .await;
    }

    trim_old_processed_boundaries(&mut runtime_state.processed_boundary_occurrences, now_local);
    runtime_state.last_checked_local = now_local;
}

async fn load_scheduler_timer_snapshot(state_handle: &state::AppState) -> SchedulerTimerSnapshot {
    let timer_manager = state_handle.timer_manager.lock().await;
    let timer_state = timer_manager.get_state();

    SchedulerTimerSnapshot {
        status: timer_manager.status(),
        auto_run_feature_enabled: timer_state.auto_run_feature_enabled,
        weekly_auto_run_config: timer_state.weekly_auto_run_config.clone(),
        timer_settings: timer_state.settings.clone(),
    }
}

fn resume_start_boundary_evaluation_when_idle(
    runtime_state: &mut SchedulerRuntimeState,
    timer_status: TimerStatus,
) {
    if runtime_state.start_boundaries_blocked_until_idle && timer_status == TimerStatus::Idle {
        runtime_state.start_boundaries_blocked_until_idle = false;
        log_info_event(
            BackendLogEvent::SchedulerResumedStartBoundaryEvaluation,
            &[],
        );
    }
}

fn build_scheduler_tick_plan(
    state_handle: &state::AppState,
    runtime_state: &mut SchedulerRuntimeState,
    now_local: DateTime<Local>,
    auto_run_feature_enabled: bool,
) -> SchedulerTickPlan {
    let config_revision = state_handle.config_revision.load(Ordering::Acquire);
    let config_changed = config_revision != runtime_state.last_seen_config_revision;
    let mut skip_start_boundaries_this_tick = false;

    let end_boundary_previous_local = if config_changed {
        runtime_state.processed_boundary_occurrences.clear();
        runtime_state.last_checked_local = now_local;
        runtime_state.last_seen_config_revision = config_revision;
        skip_start_boundaries_this_tick = true;
        log_info_event(
            BackendLogEvent::SchedulerConfigRevisionChanged,
            &[("config_revision", config_revision.to_string())],
        );
        now_local - ChronoDuration::seconds(1)
    } else {
        runtime_state.last_checked_local
    };

    let should_evaluate_start_boundaries = auto_run_feature_enabled
        && state_handle
            .auto_start_boundary_enabled
            .load(Ordering::Acquire)
        && !state_handle
            .boot_prompt_check_pending
            .load(Ordering::Acquire)
        && current_boot_prompt_runtime_state(state_handle)
            != BootPromptRuntimeState::AwaitingDecision
        && !runtime_state.start_boundaries_blocked_until_idle
        && !skip_start_boundaries_this_tick;

    SchedulerTickPlan {
        end_boundary_previous_local,
        should_evaluate_start_boundaries,
    }
}

fn current_boot_prompt_runtime_state(state_handle: &state::AppState) -> BootPromptRuntimeState {
    match state_handle.boot_prompt_runtime_state.lock() {
        Ok(guard) => *guard,
        Err(_) => BootPromptRuntimeState::NotApplicable,
    }
}

async fn process_crossed_end_boundaries(
    app: &AppHandle,
    runtime_state: &mut SchedulerRuntimeState,
    weekly_auto_run_config: &WeeklyAutoRunConfig,
    auto_run_feature_enabled: bool,
    previous_local: DateTime<Local>,
    now_local: DateTime<Local>,
) {
    if !auto_run_feature_enabled {
        return;
    }

    let crossed_end_boundaries =
        collect_crossed_end_boundaries(previous_local, now_local, weekly_auto_run_config);

    for end_boundary in crossed_end_boundaries {
        if !record_processed_boundary_occurrence(runtime_state, &end_boundary) {
            continue;
        }

        log_info_event(
            BackendLogEvent::SchedulerCrossedEndBoundary,
            &[("boundary_id", end_boundary.0.clone())],
        );
        let did_mark_stop_pending =
            mark_scheduled_stop_pending_for_current_session(app.clone()).await;
        if did_mark_stop_pending {
            log_info_event(
                BackendLogEvent::SchedulerMarkedStopPending,
                &[("boundary_id", end_boundary.0.clone())],
            );
        }
    }
}

async fn process_crossed_start_boundaries(
    app: &AppHandle,
    runtime_state: &mut SchedulerRuntimeState,
    timer_snapshot: &SchedulerTimerSnapshot,
    previous_local: DateTime<Local>,
    now_local: DateTime<Local>,
) {
    let crossed_start_boundaries = collect_crossed_start_boundaries(
        previous_local,
        now_local,
        &timer_snapshot.weekly_auto_run_config,
    );

    for start_boundary in crossed_start_boundaries {
        if !record_processed_boundary_occurrence(runtime_state, &start_boundary) {
            continue;
        }

        log_info_event(
            BackendLogEvent::SchedulerCrossedStartBoundary,
            &[("boundary_id", start_boundary.0.clone())],
        );
        let did_start = start_work_from_scheduler_when_idle(
            app.clone(),
            timer_snapshot.status,
            timer_snapshot.timer_settings.clone(),
            &start_boundary,
        )
        .await;

        if did_start {
            log_info_event(
                BackendLogEvent::SchedulerStartedWorkFromBoundary,
                &[("boundary_id", start_boundary.0.clone())],
            );
        }
    }
}

fn record_processed_boundary_occurrence(
    runtime_state: &mut SchedulerRuntimeState,
    boundary: &BoundaryOccurrenceId,
) -> bool {
    runtime_state
        .processed_boundary_occurrences
        .insert(boundary.clone())
}

pub(crate) fn trim_old_processed_boundaries(
    processed_boundary_occurrences: &mut HashSet<BoundaryOccurrenceId>,
    now_local: DateTime<Local>,
) {
    let oldest_epoch_to_keep =
        (now_local - ChronoDuration::days(PROCESSED_BOUNDARY_RETENTION_DAYS)).timestamp();

    processed_boundary_occurrences.retain(|boundary_id| {
        extract_boundary_epoch_seconds(boundary_id)
            .is_some_and(|epoch_seconds| epoch_seconds >= oldest_epoch_to_keep)
    });
}

pub(crate) fn extract_boundary_epoch_seconds(boundary_id: &BoundaryOccurrenceId) -> Option<i64> {
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
        persist_current_state_with_trigger(&app, PERSISTENCE_WRITE_TRIGGER_SCHEDULER_STOP_PENDING)
            .await;
    }

    should_persist
}

async fn start_work_from_scheduler_when_idle(
    app: AppHandle,
    timer_status_at_tick_start: TimerStatus,
    timer_settings: TimerSettings,
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
            log_warn_event(
                BackendLogEvent::SchedulerSkippedAutoStartDueToRejection,
                &[("boundary_id", start_boundary.0.clone())],
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

    persist_current_state_with_trigger(&app, PERSISTENCE_WRITE_TRIGGER_SCHEDULER_AUTO_START).await;

    true
}
