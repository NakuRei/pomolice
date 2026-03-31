use tauri::{AppHandle, Emitter, Manager};

use crate::logging_event::{log_error_event, log_info_event, BackendLogEvent};
use crate::state;
use crate::tray;
use crate::types;

struct SessionCompletionTransition {
    completed_payload: types::SessionCompletedPayload,
    transition_outcome: types::SessionTransitionOutcome,
    requires_enter_break_mode: bool,
    requires_exit_break_mode: bool,
}

struct FinalizedSessionCompletionState {
    tick_payload: types::TimerTickPayload,
    post_break_transition_policy: types::PostBreakTransitionPolicy,
    post_break_decision_pending: bool,
    scheduled_stop_state: types::ScheduledStopState,
    timer_start_source: Option<types::TimerStartSource>,
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

fn build_transition_outcome(
    completed_payload: &types::SessionCompletedPayload,
) -> types::SessionTransitionOutcome {
    types::SessionTransitionOutcome {
        transition_reason: completed_payload.transition_reason,
        completed_session_type: completed_payload.completed_session_type,
        next_status: completed_payload.next_status,
        next_session_type: completed_payload.next_session_type,
        requires_manual_continue_decision: completed_payload.requires_manual_continue_decision,
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

async fn complete_session_transition(app: &AppHandle) -> SessionCompletionTransition {
    let state_handle = app.state::<state::AppState>();
    let mut timer = state_handle.timer_manager.lock().await;
    let completed_payload = match timer.get_state().session_type {
        types::SessionType::Work => timer.complete_work_session_transition(),
        types::SessionType::ShortBreak | types::SessionType::LongBreak => {
            timer.complete_break_session_transition()
        }
    };
    let transition_outcome = build_transition_outcome(&completed_payload);

    if transition_requires_task_cancellation(transition_outcome.transition_reason) {
        let _ = timer.cancel_and_take_task();
    }

    let (requires_enter_break_mode, requires_exit_break_mode) =
        break_mode_requirements(&transition_outcome);

    SessionCompletionTransition {
        completed_payload,
        transition_outcome,
        requires_enter_break_mode,
        requires_exit_break_mode,
    }
}

async fn apply_break_mode_transition(app: &AppHandle, transition: &SessionCompletionTransition) {
    debug_assert!(
        !(transition.requires_enter_break_mode && transition.requires_exit_break_mode),
        "session transition cannot enter and exit fullscreen break mode at the same time"
    );

    if transition.requires_enter_break_mode {
        enter_break_mode(app.clone()).await;
    } else if transition.requires_exit_break_mode {
        exit_break_mode(app.clone()).await;
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

fn emit_session_completion_events(
    app: &AppHandle,
    transition: &SessionCompletionTransition,
    finalized_state: &FinalizedSessionCompletionState,
) {
    for emit_event in session_completion_emit_order(transition.transition_outcome.transition_reason)
    {
        match emit_event {
            SessionCompletionEmitEvent::SessionCompleted => {
                let _ = app.emit("session_completed", &transition.completed_payload);
            }
            SessionCompletionEmitEvent::AutoRunStopped => {
                let _ = app.emit("auto_run_stopped", serde_json::json!({}));
            }
            SessionCompletionEmitEvent::TimerTick => {
                let _ = app.emit("timer_tick", &finalized_state.tick_payload);
                tray::update_tray_icon(app, &finalized_state.tick_payload);
            }
        }
    }
}

async fn persist_timer_state(app: &AppHandle) {
    let state_handle = app.state::<state::AppState>();
    let timer = state_handle.timer_manager.lock().await;
    state_handle.persistence.save_state(timer.get_state());
}

pub async fn lib_handle_session_complete(app: AppHandle) {
    let transition = complete_session_transition(&app).await;
    apply_break_mode_transition(&app, &transition).await;
    let finalized_state = finalized_session_completion_state(&app).await;

    log_session_completion_finalized(&transition.transition_outcome, &finalized_state);
    emit_session_completion_events(&app, &transition, &finalized_state);
    persist_timer_state(&app).await;
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
        let payload = timer.apply_input_penalty();
        state_handle.persistence.save_state(timer.get_state());
        payload
    };

    if let Some(ref payload) = tick_payload {
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
