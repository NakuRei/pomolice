use tauri::{AppHandle, Emitter, Manager};

use crate::logging_event::{log_error_event, log_info_event, BackendLogEvent};
use crate::state;
use crate::tray;
use crate::types;

struct FinalizedSessionCompletionState {
    tick_payload: types::TimerTickPayload,
    post_break_transition_policy: types::PostBreakTransitionPolicy,
    post_break_decision_pending: bool,
    scheduled_stop_state: types::ScheduledStopState,
    timer_start_source: Option<types::TimerStartSource>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PipelineEmitEvent {
    SessionCompleted,
    AutoRunStopped,
    TimerTick,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum BreakModeChange {
    None,
    Enter,
    Exit,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PersistenceRequirement {
    Skip,
    Persist,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum TimerTaskRestart {
    NoRestart,
    Restart,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PipelineEffectPlan {
    pub emit_order: &'static [PipelineEmitEvent],
    pub break_mode_change: BreakModeChange,
    pub persistence_requirement: PersistenceRequirement,
    pub timer_task_restart: TimerTaskRestart,
}

const TIMER_TICK_ONLY_EMIT_ORDER: [PipelineEmitEvent; 1] = [PipelineEmitEvent::TimerTick];

const SESSION_COMPLETION_EMIT_ORDER_DEFAULT: [PipelineEmitEvent; 2] = [
    PipelineEmitEvent::SessionCompleted,
    PipelineEmitEvent::TimerTick,
];

const SESSION_COMPLETION_EMIT_ORDER_SCHEDULED_STOP: [PipelineEmitEvent; 3] = [
    PipelineEmitEvent::SessionCompleted,
    PipelineEmitEvent::AutoRunStopped,
    PipelineEmitEvent::TimerTick,
];

fn session_completion_emit_order(
    transition_reason: types::SessionTransitionReason,
) -> &'static [PipelineEmitEvent] {
    match transition_reason {
        types::SessionTransitionReason::ScheduledStopExecuted => {
            &SESSION_COMPLETION_EMIT_ORDER_SCHEDULED_STOP
        }
        _ => &SESSION_COMPLETION_EMIT_ORDER_DEFAULT,
    }
}

pub async fn lib_handle_timer_tick(app: AppHandle) {
    let state_handle = app.state::<state::AppState>();

    let (should_complete_session, tick_payload) = {
        let mut timer = state_handle.timer_manager.lock().await;

        if timer.status() == types::TimerStatus::Idle {
            return;
        }

        let (completed, payload) = timer.tick();

        (completed, payload)
    };

    let _ = app.emit("timer_tick", &tick_payload);
    tray::update_tray_icon(&app, &tick_payload);

    if should_complete_session {
        lib_handle_session_complete(app).await;
    }
}

fn transition_requires_task_cancellation(
    transition_reason: types::SessionTransitionReason,
) -> bool {
    matches!(
        transition_reason,
        types::SessionTransitionReason::BreakWaitingDecision
            | types::SessionTransitionReason::ScheduledStopExecuted
    )
}

fn break_mode_requirements(transition_outcome: &types::SessionTransitionOutcome) -> (bool, bool) {
    let requires_enter_break_mode =
        transition_outcome.transition_reason == types::SessionTransitionReason::WorkToBreak;
    let requires_exit_break_mode = matches!(
        transition_outcome.completed_session_type,
        types::SessionType::ShortBreak | types::SessionType::LongBreak
    );

    (requires_enter_break_mode, requires_exit_break_mode)
}

pub(crate) fn build_startup_resume_effect_plan() -> PipelineEffectPlan {
    PipelineEffectPlan {
        emit_order: &TIMER_TICK_ONLY_EMIT_ORDER,
        break_mode_change: BreakModeChange::None,
        persistence_requirement: PersistenceRequirement::Skip,
        timer_task_restart: TimerTaskRestart::Restart,
    }
}

pub(crate) fn build_session_completion_effect_plan(
    transition_outcome: &types::SessionTransitionOutcome,
    restart_timer_task: bool,
) -> PipelineEffectPlan {
    let (requires_enter_break_mode, requires_exit_break_mode) =
        break_mode_requirements(transition_outcome);

    PipelineEffectPlan {
        emit_order: session_completion_emit_order(transition_outcome.transition_reason),
        break_mode_change: if requires_enter_break_mode {
            BreakModeChange::Enter
        } else if requires_exit_break_mode {
            BreakModeChange::Exit
        } else {
            BreakModeChange::None
        },
        persistence_requirement: PersistenceRequirement::Persist,
        timer_task_restart: if restart_timer_task {
            TimerTaskRestart::Restart
        } else {
            TimerTaskRestart::NoRestart
        },
    }
}

async fn complete_session_transition(app: &AppHandle) -> types::SessionTransitionOutcome {
    let state_handle = app.state::<state::AppState>();
    let mut timer = state_handle.timer_manager.lock().await;
    let completed_payload = match timer.get_state().session_type {
        types::SessionType::Work => timer.complete_work_session_transition(),
        types::SessionType::ShortBreak | types::SessionType::LongBreak => {
            timer.complete_break_session_transition()
        }
    };
    let transition_outcome =
        types::SessionTransitionOutcome::from_completed_payload(&completed_payload);

    if transition_requires_task_cancellation(transition_outcome.transition_reason) {
        let _ = timer.cancel_and_take_task();
    }

    transition_outcome
}

async fn apply_break_mode_transition(app: &AppHandle, effect_plan: &PipelineEffectPlan) {
    match effect_plan.break_mode_change {
        BreakModeChange::None => {}
        BreakModeChange::Enter => enter_break_mode(app.clone()).await,
        BreakModeChange::Exit => exit_break_mode(app.clone()).await,
    }
}

async fn finalized_session_completion_state(app: &AppHandle) -> FinalizedSessionCompletionState {
    let state_handle = app.state::<state::AppState>();
    let timer = state_handle.timer_manager.lock().await;
    let state = timer.get_state();

    FinalizedSessionCompletionState {
        tick_payload: state.to_tick_payload(),
        post_break_transition_policy: state.settings.post_break_transition_policy,
        post_break_decision_pending: state.post_break_decision_pending,
        scheduled_stop_state: state.scheduled_stop_state,
        timer_start_source: state.timer_start_source,
    }
}

fn log_session_completion_finalized(
    transition_outcome: &types::SessionTransitionOutcome,
    finalized_state: &FinalizedSessionCompletionState,
) {
    let transition_reason = transition_outcome.transition_reason;
    let post_break_transition_policy = finalized_state.post_break_transition_policy;
    let post_break_decision_pending = finalized_state.post_break_decision_pending;
    let scheduled_stop_state = finalized_state.scheduled_stop_state;
    let timer_start_source = finalized_state.timer_start_source;

    log_info_event(
        BackendLogEvent::TimerTransitionCompleted,
        &[
            (
                "transition_stage",
                "session_completion_pipeline_finalized".to_string(),
            ),
            ("transition_reason", format!("{transition_reason:?}")),
            ("policy", format!("{post_break_transition_policy:?}")),
            (
                "post_break_decision_pending",
                post_break_decision_pending.to_string(),
            ),
            ("scheduled_stop_state", format!("{scheduled_stop_state:?}")),
            ("timer_start_source", format!("{timer_start_source:?}")),
        ],
    );
}

fn emit_pipeline_events(
    app: &AppHandle,
    emit_order: &[PipelineEmitEvent],
    transition_outcome: Option<&types::SessionTransitionOutcome>,
    tick_payload: &types::TimerTickPayload,
) {
    for emit_event in emit_order {
        match emit_event {
            PipelineEmitEvent::SessionCompleted => {
                let Some(transition_outcome) = transition_outcome else {
                    debug_assert!(
                        false,
                        "session_completed emission requires a transition outcome"
                    );
                    continue;
                };
                let completed_payload =
                    types::SessionCompletedPayload::from_transition_outcome(transition_outcome);
                let _ = app.emit("session_completed", &completed_payload);
            }
            PipelineEmitEvent::AutoRunStopped => {
                let _ = app.emit("auto_run_stopped", serde_json::json!({}));
            }
            PipelineEmitEvent::TimerTick => {
                let _ = app.emit("timer_tick", tick_payload);
                tray::update_tray_icon(app, tick_payload);
            }
        }
    }
}

async fn persist_timer_state(app: &AppHandle) {
    let state_handle = app.state::<state::AppState>();
    state_handle.persist_current_state().await;
}

async fn restart_timer_task(app: &AppHandle) {
    let state_handle = app.state::<state::AppState>();
    let mut timer = state_handle.timer_manager.lock().await;
    timer.spawn_timer_task(app.clone());
}

async fn execute_session_completion_effect_plan(
    app: &AppHandle,
    transition_outcome: &types::SessionTransitionOutcome,
    restart_timer_task_after_effects: bool,
) {
    let effect_plan =
        build_session_completion_effect_plan(transition_outcome, restart_timer_task_after_effects);
    apply_break_mode_transition(app, &effect_plan).await;
    let finalized_state = finalized_session_completion_state(app).await;

    log_session_completion_finalized(transition_outcome, &finalized_state);
    emit_pipeline_events(
        app,
        effect_plan.emit_order,
        Some(transition_outcome),
        &finalized_state.tick_payload,
    );

    if effect_plan.persistence_requirement == PersistenceRequirement::Persist {
        persist_timer_state(app).await;
    }

    if effect_plan.timer_task_restart == TimerTaskRestart::Restart {
        restart_timer_task(app).await;
    }
}

pub async fn lib_handle_session_complete(app: AppHandle) {
    let transition_outcome = complete_session_transition(&app).await;
    execute_session_completion_effect_plan(&app, &transition_outcome, false).await;
}

pub(crate) async fn lib_handle_startup_restore(
    app: AppHandle,
    initialization_outcome: state::AppInitializationOutcome,
) {
    let state::AppInitializationOutcome {
        initial_tick_payload,
        restore_status: _,
        startup_action,
    } = initialization_outcome;

    match startup_action {
        state::StartupRestoreAction::None => {}
        state::StartupRestoreAction::RestartRestoredTimerTask => {
            let Some(tick_payload) = initial_tick_payload else {
                debug_assert!(
                    false,
                    "running startup restore must provide an initial tick payload"
                );
                return;
            };

            let effect_plan = build_startup_resume_effect_plan();
            emit_pipeline_events(&app, effect_plan.emit_order, None, &tick_payload);

            if effect_plan.timer_task_restart == TimerTaskRestart::Restart {
                restart_timer_task(&app).await;
            }
        }
        state::StartupRestoreAction::FinalizeExpiredRestoredWork(transition_outcome) => {
            let restart_timer_task = transition_outcome.next_status != types::TimerStatus::Idle;
            execute_session_completion_effect_plan(&app, &transition_outcome, restart_timer_task)
                .await;
        }
    }
}

async fn enter_break_mode(app: AppHandle) {
    log_info_event(BackendLogEvent::SessionFullscreenBreakEnterStarted, &[]);
    let state_handle = app.state::<state::AppState>();

    {
        let mut input_monitor = state_handle.input_monitor.lock().await;
        input_monitor.start();
        log_info_event(BackendLogEvent::InputMonitoringStarted, &[]);
    }

    {
        let mut window_manager = state_handle.window_manager.lock().await;
        if let Err(e) = window_manager
            .enter_fullscreen_all_monitors(app.clone())
            .await
        {
            log_error_event(
                BackendLogEvent::SessionFullscreenBreakEnterFailed,
                &[("error_message", e.to_string())],
                None,
            );
            return;
        }
        log_info_event(BackendLogEvent::SessionFullscreenBreakEntered, &[]);
    }

    {
        let mut timer = state_handle.timer_manager.lock().await;
        timer.set_fullscreen_break(true);
    }

    let _ = app.emit("break_started", serde_json::json!({}));
}

async fn exit_break_mode(app: AppHandle) {
    log_info_event(BackendLogEvent::SessionFullscreenBreakExitStarted, &[]);
    let state_handle = app.state::<state::AppState>();

    {
        let mut input_monitor = state_handle.input_monitor.lock().await;
        input_monitor.stop();
        log_info_event(BackendLogEvent::InputMonitoringStopping, &[]);
    }

    {
        let mut window_manager = state_handle.window_manager.lock().await;
        if let Err(e) = window_manager.exit_fullscreen_all_monitors(&app) {
            log_error_event(
                BackendLogEvent::SessionFullscreenBreakExitFailed,
                &[("error_message", e.to_string())],
                None,
            );
            return;
        }
        log_info_event(BackendLogEvent::SessionFullscreenBreakExited, &[]);
    }

    {
        let mut timer = state_handle.timer_manager.lock().await;
        timer.set_fullscreen_break(false);
    }

    let _ = app.emit("break_completed", serde_json::json!({}));
}

pub async fn lib_handle_input_detected(app: AppHandle) {
    let state_handle = app.state::<state::AppState>();

    let tick_payload = {
        let mut timer = state_handle.timer_manager.lock().await;
        timer.apply_input_penalty()
    };

    if let Some(ref payload) = tick_payload {
        state_handle.persist_current_state().await;
        let _ = app.emit("timer_tick", payload);
        tray::update_tray_icon(&app, payload);
    }
}

pub async fn lib_handle_escape_combo(app: AppHandle) {
    let state_handle = app.state::<state::AppState>();

    {
        let mut input_monitor = state_handle.input_monitor.lock().await;
        input_monitor.stop();
    }

    {
        let mut window_manager = state_handle.window_manager.lock().await;
        let _ = window_manager.exit_fullscreen_all_monitors(&app);
    }
}

#[cfg(test)]
mod emit_order_tests {
    use super::{
        session_completion_emit_order, PipelineEmitEvent, SESSION_COMPLETION_EMIT_ORDER_DEFAULT,
        SESSION_COMPLETION_EMIT_ORDER_SCHEDULED_STOP,
    };
    use crate::types::SessionTransitionReason;

    #[test]
    fn test_session_completion_emit_order_for_non_scheduled_stop() {
        let emit_order = session_completion_emit_order(SessionTransitionReason::BreakAutoStarted);
        assert_eq!(emit_order, &SESSION_COMPLETION_EMIT_ORDER_DEFAULT);
        assert_eq!(
            emit_order,
            &[
                PipelineEmitEvent::SessionCompleted,
                PipelineEmitEvent::TimerTick,
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
                PipelineEmitEvent::SessionCompleted,
                PipelineEmitEvent::AutoRunStopped,
                PipelineEmitEvent::TimerTick,
            ]
        );
    }
}
