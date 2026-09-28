use tauri::{Manager, Runtime, State, WebviewWindow};

use crate::commands::input_rejection::ObservedCommandArg;
use crate::error::{PomoliceError, Result};
use crate::logging_event::{log_info_event, log_warn_event, BackendLogEvent};
use crate::settings::SettingsChanges;
use crate::state::AppState;
use crate::timer_core::Event;
use crate::timer_effects::TimerSnapshot;
use crate::types::{BreakAttendance, PostBreakDecisionAction};

/// Start a work session with the saved settings.
#[tauri::command]
pub(crate) async fn start_work<R: Runtime>(
    window: WebviewWindow<R>,
    state: State<'_, AppState>,
) -> Result<TimerSnapshot> {
    log_command_invoked("start_work", &window);
    authorize_main_window(&window, "start_work")?;
    state
        .timer
        .dispatch(window.app_handle(), Event::StartWorkRequested)
        .await
}

/// Resolve manual post-break decision from continue dialog.
///
/// This command is valid only while `post_break_decision_pending = true`.
#[tauri::command]
pub(crate) async fn resolve_post_break_decision<R: Runtime>(
    window: WebviewWindow<R>,
    state: State<'_, AppState>,
    action: ObservedCommandArg<PostBreakDecisionAction>,
) -> Result<TimerSnapshot> {
    authorize_main_window(&window, "resolve_post_break_decision")?;
    let action = action.into_inner();
    state
        .timer
        .dispatch(
            window.app_handle(),
            Event::PostBreakDecisionResolved { action },
        )
        .await
}

/// Valid only while the timer is on break.
#[tauri::command]
pub(crate) async fn set_away_during_break<R: Runtime>(
    window: WebviewWindow<R>,
    state: State<'_, AppState>,
    away: ObservedCommandArg<bool>,
) -> Result<TimerSnapshot> {
    log_command_invoked("set_away_during_break", &window);
    authorize_main_window(&window, "set_away_during_break")?;
    let attendance = BreakAttendance::from_away_flag(away.into_inner());
    state
        .timer
        .dispatch(
            window.app_handle(),
            Event::BreakAttendanceSet { attendance },
        )
        .await
}

/// Emergency stop - reset all state
#[tauri::command]
pub(crate) async fn emergency_stop<R: Runtime>(
    window: WebviewWindow<R>,
    state: State<'_, AppState>,
) -> Result<TimerSnapshot> {
    log_command_invoked("emergency_stop", &window);
    authorize_main_window(&window, "emergency_stop")?;
    state
        .timer
        .dispatch(window.app_handle(), Event::EmergencyStopRequested)
        .await
}

/// Change the given user settings; the rest keep their values. Accepted in
/// any timer state; a running session keeps its duration and new durations
/// apply from the next one. A rejected save changes nothing and names every
/// input to fix.
#[tauri::command]
pub(crate) async fn update_settings<R: Runtime>(
    window: WebviewWindow<R>,
    state: State<'_, AppState>,
    changes: ObservedCommandArg<SettingsChanges>,
) -> Result<TimerSnapshot> {
    log_command_invoked("update_settings", &window);
    authorize_main_window(&window, "update_settings")?;
    state
        .timer
        .update_settings(window.app_handle(), changes.into_inner())
        .await
}

/// Read the current state without advancing the timer.
#[tauri::command]
pub(crate) async fn get_full_state<R: Runtime>(
    window: WebviewWindow<R>,
    state: State<'_, AppState>,
) -> Result<TimerSnapshot> {
    authorize_main_window(&window, "get_full_state")?;
    Ok(state.timer.read_full_state().await)
}

#[tauri::command]
pub(crate) async fn retry_failed_timer_effects<R: Runtime>(
    window: WebviewWindow<R>,
    state: State<'_, AppState>,
) -> Result<TimerSnapshot> {
    authorize_main_window(&window, "retry_failed_timer_effects")?;
    Ok(state.timer.retry_failed_effects(window.app_handle()).await)
}

/// Resolve startup auto-start confirmation prompt.
#[tauri::command]
pub(crate) async fn resolve_boot_auto_start_prompt<R: Runtime>(
    window: WebviewWindow<R>,
    state: State<'_, AppState>,
    start_now: ObservedCommandArg<bool>,
) -> Result<TimerSnapshot> {
    authorize_main_window(&window, "resolve_boot_auto_start_prompt")?;
    let start_now = start_now.into_inner();
    state
        .timer
        .dispatch(window.app_handle(), Event::BootPromptResolved { start_now })
        .await
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

fn authorize_main_window_label(window_label: &str) -> Result<()> {
    if window_label == "main" {
        return Ok(());
    }

    Err(PomoliceError::Unauthorized(window_label.to_string()))
}

fn authorize_main_window<R: Runtime>(
    window: &WebviewWindow<R>,
    command_name: &'static str,
) -> Result<()> {
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
