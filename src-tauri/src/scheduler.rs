use chrono::{DateTime, Duration as ChronoDuration, Local};
use std::collections::HashSet;
use std::sync::atomic::Ordering;
use std::time::Duration;
use tauri::{AppHandle, Emitter, Manager};
use tokio::time::MissedTickBehavior;
use tokio_util::sync::CancellationToken;
use tracing::info;

use crate::managers::weekly_schedule::{
    collect_crossed_end_boundaries, collect_crossed_start_boundaries,
};
use crate::state;
use crate::tray;
use crate::types::{
    BootPromptRuntimeState, BoundaryOccurrenceId, TimerSettings, TimerStartSource, TimerStatus,
};

const SCHEDULER_INTERVAL_SECONDS: u64 = 1;
const PROCESSED_BOUNDARY_RETENTION_DAYS: i64 = 8;

struct SchedulerRuntimeState {
    processed_boundary_occurrences: HashSet<BoundaryOccurrenceId>,
    last_checked_local: DateTime<Local>,
    start_boundaries_blocked_until_idle: bool,
    last_seen_config_revision: u64,
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

pub(crate) fn trim_old_processed_boundaries(
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
