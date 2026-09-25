// Outcomes are reported as the event the core applies next; failures are
// logged here and never surface as errors.

use tauri::{AppHandle, Runtime};

use crate::error::PomoliceError;
use crate::external_adapters::window_lock::WindowStateManager;
use crate::logging_event::{log_error_event, log_info_event, BackendLogEvent};
use crate::state::AppState;
use crate::timer_core::Event;

pub(crate) async fn enter<R: Runtime>(state: &AppState, app: &AppHandle<R>) -> Event {
    log_info_event(BackendLogEvent::SessionFullscreenBreakEnterStarted, &[]);
    state.input_monitor.lock().await.start();

    match WindowStateManager::capture_main_window_state_for_break(app).await {
        Ok(saved_state) => state
            .window_manager
            .lock()
            .await
            .set_saved_state(saved_state),
        Err(error) => {
            state
                .input_monitor
                .lock()
                .await
                .rollback_break_enter_failure();
            log_failure(
                BackendLogEvent::SessionFullscreenBreakEnterFailed,
                "capture_main_window_state_for_break",
                &error,
            );
            return Event::BreakLockEnterFailed;
        }
    }

    if let Err(error) = WindowStateManager::enter_fullscreen_all_monitors(app.clone()).await {
        rollback_enter_failure(state, app).await;
        log_failure(
            BackendLogEvent::SessionFullscreenBreakEnterFailed,
            "enter_fullscreen_all_monitors",
            &error,
        );
        return Event::BreakLockEnterFailed;
    }

    log_info_event(BackendLogEvent::SessionFullscreenBreakEntered, &[]);
    Event::BreakLockEntered
}

pub(crate) async fn release<R: Runtime>(state: &AppState, app: &AppHandle<R>) -> Event {
    log_info_event(BackendLogEvent::SessionFullscreenBreakExitStarted, &[]);
    state.input_monitor.lock().await.stop();

    if let Err(error) = release_fullscreen(state, app).await {
        log_failure(
            BackendLogEvent::SessionFullscreenBreakExitFailed,
            "exit_fullscreen_all_monitors",
            &error,
        );
        return Event::BreakLockReleaseFailed;
    }

    log_info_event(BackendLogEvent::SessionFullscreenBreakExited, &[]);
    Event::BreakLockReleased
}

async fn rollback_enter_failure<R: Runtime>(state: &AppState, app: &AppHandle<R>) {
    state
        .input_monitor
        .lock()
        .await
        .rollback_break_enter_failure();
    if let Err(error) = release_fullscreen(state, app).await {
        log_failure(
            BackendLogEvent::SessionFullscreenBreakEnterFailed,
            "rollback_exit_fullscreen_all_monitors",
            &error,
        );
    }
}

async fn release_fullscreen<R: Runtime>(
    state: &AppState,
    app: &AppHandle<R>,
) -> crate::error::Result<()> {
    state
        .window_manager
        .lock()
        .await
        .exit_fullscreen_all_monitors(app)
}

fn log_failure(event: BackendLogEvent, step: &'static str, error: &PomoliceError) {
    log_error_event(event, &[("step", step.to_string())], Some(error));
}
