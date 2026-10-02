use crate::ipc::result::CommandResponse;
use tauri::{Manager, Runtime, WebviewWindow};

use crate::error::PomoliceError;
use crate::ipc::errors::BackendErrorResponse;
use crate::ipc::input::ObservedCommandArg;
use crate::logging_event::{log_info_event, log_warn_event, BackendLogEvent};
use crate::state::AppState;
use crate::timer_runtime::observation::TimerSnapshot;
use pomolice_core::settings::input::SettingsChanges;
use pomolice_core::timer_core::Event;
use pomolice_core::types::{BreakAttendance, PostBreakDecisionAction};

/// Start a work session with the saved settings.
#[tauri::command]
#[specta::specta]
pub(crate) async fn start_work<R: Runtime>(
    window: WebviewWindow<R>,
) -> CommandResponse<TimerSnapshot> {
    let state = window.state::<AppState>();
    log_command_invoked("start_work", &window);
    if let Err(error) = authorize_main_window(&window, "start_work") {
        return CommandResponse::Rejected {
            error: BackendErrorResponse::from(&error),
        };
    }
    state
        .timer
        .dispatch(window.app_handle(), Event::StartWorkRequested)
        .await
        .map_err(|error| BackendErrorResponse::from(&error))
        .into()
}

/// Resolve manual post-break decision from continue dialog.
///
/// This command is valid only while `post_break_decision_pending = true`.
#[tauri::command]
#[specta::specta]
pub(crate) async fn resolve_post_break_decision<R: Runtime>(
    window: WebviewWindow<R>,
    action: ObservedCommandArg<PostBreakDecisionAction>,
) -> CommandResponse<TimerSnapshot> {
    let state = window.state::<AppState>();
    if let Err(error) = authorize_main_window(&window, "resolve_post_break_decision") {
        return CommandResponse::Rejected {
            error: BackendErrorResponse::from(&error),
        };
    }
    let action = action.into_inner();
    state
        .timer
        .dispatch(
            window.app_handle(),
            Event::PostBreakDecisionResolved { action },
        )
        .await
        .map_err(|error| BackendErrorResponse::from(&error))
        .into()
}

/// Valid only while the timer is on break.
#[tauri::command]
#[specta::specta]
pub(crate) async fn set_away_during_break<R: Runtime>(
    window: WebviewWindow<R>,
    away: ObservedCommandArg<bool>,
) -> CommandResponse<TimerSnapshot> {
    let state = window.state::<AppState>();
    log_command_invoked("set_away_during_break", &window);
    if let Err(error) = authorize_main_window(&window, "set_away_during_break") {
        return CommandResponse::Rejected {
            error: BackendErrorResponse::from(&error),
        };
    }
    let attendance = BreakAttendance::from_away_flag(away.into_inner());
    state
        .timer
        .dispatch(
            window.app_handle(),
            Event::BreakAttendanceSet { attendance },
        )
        .await
        .map_err(|error| BackendErrorResponse::from(&error))
        .into()
}

/// Emergency stop - reset all state
#[tauri::command]
#[specta::specta]
pub(crate) async fn emergency_stop<R: Runtime>(
    window: WebviewWindow<R>,
) -> CommandResponse<TimerSnapshot> {
    let state = window.state::<AppState>();
    log_command_invoked("emergency_stop", &window);
    if let Err(error) = authorize_main_window(&window, "emergency_stop") {
        return CommandResponse::Rejected {
            error: BackendErrorResponse::from(&error),
        };
    }
    state
        .timer
        .dispatch(window.app_handle(), Event::EmergencyStopRequested)
        .await
        .map_err(|error| BackendErrorResponse::from(&error))
        .into()
}

/// Change the given user settings; the rest keep their values. Accepted in
/// any timer state; a running session keeps its duration and new durations
/// apply from the next one. A rejected save changes nothing and names every
/// input to fix.
#[tauri::command]
#[specta::specta]
pub(crate) async fn update_settings<R: Runtime>(
    window: WebviewWindow<R>,
    changes: ObservedCommandArg<SettingsChanges>,
) -> CommandResponse<TimerSnapshot> {
    let state = window.state::<AppState>();
    log_command_invoked("update_settings", &window);
    if let Err(error) = authorize_main_window(&window, "update_settings") {
        return CommandResponse::Rejected {
            error: BackendErrorResponse::from(&error),
        };
    }
    state
        .timer
        .update_settings(window.app_handle(), changes.into_inner())
        .await
        .map_err(|error| BackendErrorResponse::from(&error))
        .into()
}

/// Read the current state without advancing the timer.
#[tauri::command]
#[specta::specta]
pub(crate) async fn get_full_state<R: Runtime>(
    window: WebviewWindow<R>,
) -> CommandResponse<TimerSnapshot> {
    let state = window.state::<AppState>();
    if let Err(error) = authorize_main_window(&window, "get_full_state") {
        return CommandResponse::Rejected {
            error: BackendErrorResponse::from(&error),
        };
    }
    CommandResponse::Accepted {
        value: state.timer.read_full_state().await,
    }
}

#[tauri::command]
#[specta::specta]
pub(crate) async fn retry_failed_timer_effects<R: Runtime>(
    window: WebviewWindow<R>,
) -> CommandResponse<TimerSnapshot> {
    let state = window.state::<AppState>();
    if let Err(error) = authorize_main_window(&window, "retry_failed_timer_effects") {
        return CommandResponse::Rejected {
            error: BackendErrorResponse::from(&error),
        };
    }
    CommandResponse::Accepted {
        value: state.timer.retry_failed_effects(window.app_handle()).await,
    }
}

/// Resolve startup auto-start confirmation prompt.
#[tauri::command]
#[specta::specta]
pub(crate) async fn resolve_boot_auto_start_prompt<R: Runtime>(
    window: WebviewWindow<R>,
    start_now: ObservedCommandArg<bool>,
) -> CommandResponse<TimerSnapshot> {
    let state = window.state::<AppState>();
    if let Err(error) = authorize_main_window(&window, "resolve_boot_auto_start_prompt") {
        return CommandResponse::Rejected {
            error: BackendErrorResponse::from(&error),
        };
    }
    let start_now = start_now.into_inner();
    state
        .timer
        .dispatch(window.app_handle(), Event::BootPromptResolved { start_now })
        .await
        .map_err(|error| BackendErrorResponse::from(&error))
        .into()
}

fn log_command_invoked<R: Runtime>(command_name: &'static str, window: &WebviewWindow<R>) {
    log_info_event(
        BackendLogEvent::TimerCommandInvoked,
        &[
            ("command_name", command_name.to_string()),
            ("window_label", window.label().to_string()),
        ],
    );
}

fn authorize_main_window_label(window_label: &str) -> crate::error::Result<()> {
    if window_label == "main" {
        return Ok(());
    }

    Err(PomoliceError::Unauthorized(window_label.to_string()))
}

fn authorize_main_window<R: Runtime>(
    window: &WebviewWindow<R>,
    command_name: &'static str,
) -> crate::error::Result<()> {
    let window_label = window.label();
    if let Err(error) = authorize_main_window_label(window_label) {
        log_warn_event(
            BackendLogEvent::TimerCommandUnauthorized,
            &[
                ("command_name", command_name.to_string()),
                ("window_label", window_label.to_string()),
            ],
        );
        return Err(error);
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn main_window_is_authorized() {
        assert!(authorize_main_window_label("main").is_ok());
    }

    #[test]
    fn other_windows_are_rejected_with_their_label() {
        match authorize_main_window_label("break-overlay-1") {
            Err(PomoliceError::Unauthorized(label)) => assert_eq!(label, "break-overlay-1"),
            other => panic!("expected unauthorized error, got {other:?}"),
        }
    }
}
