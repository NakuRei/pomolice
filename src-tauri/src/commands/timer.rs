use tauri::{Manager, State, WebviewWindow};

use crate::commands::input_rejection::ObservedCommandArg;
use crate::error::{PomoliceError, Result};
use crate::logging_event::{log_info_event, log_warn_event, BackendLogEvent};
use crate::state::AppState;
use crate::timer_core::{Event, TimerCore};
use crate::types::{
    BreakAttendance, FullStatePayload, LongBreakDurationSecs, LongBreakIntervalCount,
    PostBreakDecisionAction, PostBreakTransitionPolicy, ShortBreakDurationSecs, TimerSettings,
    WeeklyAutoRunConfig, WorkDurationSecs,
};

/// Start a work session with settings
#[tauri::command]
pub(crate) async fn start_work(
    window: WebviewWindow,
    state: State<'_, AppState>,
    work_duration_secs: ObservedCommandArg<WorkDurationSecs>,
    short_break_duration_secs: ObservedCommandArg<ShortBreakDurationSecs>,
    long_break_duration_secs: ObservedCommandArg<LongBreakDurationSecs>,
    long_break_interval: ObservedCommandArg<LongBreakIntervalCount>,
    post_break_transition_policy: ObservedCommandArg<PostBreakTransitionPolicy>,
) -> Result<()> {
    log_command_invoked("start_work", &window);
    authorize_main_window(&window, "start_work")?;

    let settings = TimerSettings::new(
        work_duration_secs.into_inner(),
        short_break_duration_secs.into_inner(),
        long_break_duration_secs.into_inner(),
        long_break_interval.into_inner(),
        post_break_transition_policy.into_inner(),
    );
    state
        .timer
        .dispatch(window.app_handle(), Event::StartWorkRequested { settings })
        .await
}

/// Resolve manual post-break decision from continue dialog.
///
/// This command is valid only while `post_break_decision_pending = true`.
#[tauri::command]
pub(crate) async fn resolve_post_break_decision(
    window: WebviewWindow,
    state: State<'_, AppState>,
    action: ObservedCommandArg<PostBreakDecisionAction>,
) -> Result<()> {
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
pub(crate) async fn set_away_during_break(
    window: WebviewWindow,
    state: State<'_, AppState>,
    away: ObservedCommandArg<bool>,
) -> Result<()> {
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
pub(crate) async fn emergency_stop(
    window: WebviewWindow,
    state: State<'_, AppState>,
) -> Result<()> {
    log_command_invoked("emergency_stop", &window);
    authorize_main_window(&window, "emergency_stop")?;
    state
        .timer
        .dispatch(window.app_handle(), Event::EmergencyStopRequested)
        .await
}

/// Sync timer duration settings from frontend (idle only)
#[tauri::command]
pub(crate) async fn sync_timer_duration_settings(
    window: WebviewWindow,
    state: State<'_, AppState>,
    work_duration_secs: ObservedCommandArg<WorkDurationSecs>,
    short_break_duration_secs: ObservedCommandArg<ShortBreakDurationSecs>,
    long_break_duration_secs: ObservedCommandArg<LongBreakDurationSecs>,
    long_break_interval: ObservedCommandArg<LongBreakIntervalCount>,
) -> Result<()> {
    authorize_main_window(&window, "sync_timer_duration_settings")?;
    state
        .timer
        .dispatch(
            window.app_handle(),
            Event::TimerDurationSettingsChanged {
                work_duration_secs: work_duration_secs.into_inner(),
                short_break_duration_secs: short_break_duration_secs.into_inner(),
                long_break_duration_secs: long_break_duration_secs.into_inner(),
                long_break_interval: long_break_interval.into_inner(),
            },
        )
        .await
}

/// Sync automation policies from frontend
#[tauri::command]
pub(crate) async fn sync_automation_policies(
    window: WebviewWindow,
    state: State<'_, AppState>,
    auto_run_feature_enabled: ObservedCommandArg<bool>,
    weekly_auto_run_config: ObservedCommandArg<WeeklyAutoRunConfig>,
    post_break_transition_policy: ObservedCommandArg<PostBreakTransitionPolicy>,
) -> Result<()> {
    authorize_main_window(&window, "sync_automation_policies")?;
    state
        .timer
        .dispatch(
            window.app_handle(),
            Event::AutomationPoliciesChanged {
                auto_run_feature_enabled: auto_run_feature_enabled.into_inner(),
                weekly_auto_run_config: weekly_auto_run_config.into_inner(),
                post_break_transition_policy: post_break_transition_policy.into_inner(),
            },
        )
        .await
}

/// Get full timer state for initial sync
#[tauri::command]
pub(crate) async fn get_full_state(
    window: WebviewWindow,
    state: State<'_, AppState>,
) -> Result<FullStatePayload> {
    authorize_main_window(&window, "get_full_state")?;
    state
        .timer
        .dispatch_then_read(
            window.app_handle(),
            Event::FirstFullStateObserved,
            TimerCore::full_state_payload,
        )
        .await
}

/// Resolve startup auto-start confirmation prompt.
#[tauri::command]
pub(crate) async fn resolve_boot_auto_start_prompt(
    window: WebviewWindow,
    state: State<'_, AppState>,
    start_now: ObservedCommandArg<bool>,
) -> Result<()> {
    authorize_main_window(&window, "resolve_boot_auto_start_prompt")?;
    let start_now = start_now.into_inner();
    state
        .timer
        .dispatch(window.app_handle(), Event::BootPromptResolved { start_now })
        .await
}

fn log_command_invoked(command_name: &'static str, window: &WebviewWindow) {
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

fn authorize_main_window(window: &WebviewWindow, command_name: &'static str) -> Result<()> {
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
