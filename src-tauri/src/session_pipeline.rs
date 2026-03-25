use tauri::{AppHandle, Emitter, Manager};
use tracing::{error, info};

use crate::state;
use crate::tray;
use crate::types;

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

async fn enter_break_mode(app: AppHandle) {
    info!("Entering fullscreen break mode");
    let state_handle = app.state::<state::AppState>();

    {
        let mut input_monitor = state_handle.input_monitor.lock().await;
        input_monitor.start();
        info!("Input monitoring started successfully");
    }

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

    {
        let mut timer = state_handle.timer_manager.lock().await;
        timer.set_fullscreen_break(true);
    }

    let _ = app.emit("break_started", serde_json::json!({}));
}

async fn exit_break_mode(app: AppHandle) {
    info!("Exiting fullscreen break mode");
    let state_handle = app.state::<state::AppState>();

    {
        let mut input_monitor = state_handle.input_monitor.lock().await;
        input_monitor.stop();
        info!("Input monitoring stopped successfully");
    }

    {
        let mut window_manager = state_handle.window_manager.lock().await;
        if let Err(e) = window_manager.exit_fullscreen_all_monitors(&app) {
            error!("Failed to exit fullscreen: {:?}", e);
            return;
        }
        info!("Exited fullscreen mode on all monitors");
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
