use tauri::{Runtime, WebviewWindow};

use crate::error::PomoliceError;
use crate::logging_event::{log_warn_event, BackendLogEvent};

pub(super) mod application;
pub(super) mod timer;

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
            BackendLogEvent::IpcCommandUnauthorized,
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
    use crate::ipc::input::{
        observe_command_arg_result, CommandInputRejectionContext, ObservedCommandArg,
    };
    use pomolice_core::settings::input::SettingsChanges;
    use pomolice_core::timer_core::PostBreakDecisionAction;
    use serde_json::json;
    use tauri::test::mock_builder;
    use tauri::{WebviewUrl, WebviewWindowBuilder};

    fn command_argument<T>(value: T) -> ObservedCommandArg<T> {
        observe_command_arg_result(
            Ok(value),
            CommandInputRejectionContext {
                command_name: "test",
                argument_name: "value",
                window_label: "break-overlay-1".to_string(),
            },
        )
        .expect("valid command argument")
    }

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

    #[test]
    fn commands_reject_non_main_windows_before_accessing_application_state() {
        let app = mock_builder()
            .build(crate::app_context())
            .expect("app without managed state should build");
        for label in ["break-overlay-1", "settings"] {
            let window = WebviewWindowBuilder::new(&app, label, WebviewUrl::default())
                .build()
                .expect("window should build");

            tauri::async_runtime::block_on(async {
                macro_rules! assert_unauthorized {
                    ($response:expr) => {
                        assert_eq!(
                            serde_json::to_value($response.await)
                                .expect("command response should serialize"),
                            json!({
                                "status": "rejected",
                                "error": { "code": "unauthorized_window" },
                            }),
                            "{} from {label}",
                            stringify!($response),
                        );
                    };
                }

                assert_unauthorized!(application::get_emergency_shortcut_status(window.clone()));
                assert_unauthorized!(application::has_input_monitor_failed(window.clone()));
                assert_unauthorized!(application::is_tray_available(window.clone()));
                assert_unauthorized!(application::exit_without_tray(window.clone()));
                assert_unauthorized!(timer::start_work(window.clone()));
                assert_unauthorized!(timer::resolve_post_break_decision(
                    window.clone(),
                    command_argument(PostBreakDecisionAction::Continue),
                ));
                assert_unauthorized!(timer::set_away_during_break(
                    window.clone(),
                    command_argument(true),
                ));
                assert_unauthorized!(timer::emergency_stop(window.clone()));
                assert_unauthorized!(timer::update_settings(
                    window.clone(),
                    command_argument(SettingsChanges::default()),
                ));
                assert_unauthorized!(timer::get_full_state(window.clone()));
                assert_unauthorized!(timer::retry_failed_timer_effects(window.clone()));
                assert_unauthorized!(timer::resolve_boot_auto_start_prompt(
                    window.clone(),
                    command_argument(true),
                ));
            });
        }
    }

    #[test]
    fn main_window_can_read_application_status() {
        use crate::external_adapters::emergency_shortcut::{
            EmergencyShortcutState, EmergencyShortcutStatus,
        };

        let app = mock_builder()
            .manage(EmergencyShortcutState(parking_lot::Mutex::new(
                EmergencyShortcutStatus::InputMonitor,
            )))
            .build(crate::app_context())
            .expect("app should build");
        let window = WebviewWindowBuilder::new(&app, "main", WebviewUrl::default())
            .build()
            .expect("window should build");

        tauri::async_runtime::block_on(async {
            assert_eq!(
                serde_json::to_value(
                    application::get_emergency_shortcut_status(window.clone()).await
                )
                .expect("shortcut status should serialize"),
                json!({ "status": "accepted", "value": { "kind": "input_monitor" } }),
            );
            assert_eq!(
                serde_json::to_value(application::is_tray_available(window).await)
                    .expect("tray status should serialize"),
                json!({ "status": "accepted", "value": false }),
            );
        });
    }
}
