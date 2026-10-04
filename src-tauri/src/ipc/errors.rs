use tauri::ipc::InvokeError;

use crate::error::PomoliceError;
use pomolice_core::settings::input::SettingsValidationIssue;

#[derive(serde::Serialize, specta::Type)]
#[serde(tag = "code", rename_all = "snake_case")]
pub(crate) enum BackendErrorResponse {
    UnauthorizedWindow,
    InvalidStateTransition,
    RateLimited,
    InvalidSettings {
        issues: Vec<SettingsValidationIssue>,
    },
    WindowControlError,
    PersistenceError,
}

impl From<&PomoliceError> for BackendErrorResponse {
    fn from(error: &PomoliceError) -> Self {
        match error {
            PomoliceError::Unauthorized(_) => Self::UnauthorizedWindow,
            PomoliceError::InvalidStateTransition { .. } => Self::InvalidStateTransition,
            PomoliceError::RateLimited { .. } => Self::RateLimited,
            PomoliceError::InvalidSettings(errors) => Self::InvalidSettings {
                issues: errors.issues().to_vec(),
            },
            PomoliceError::WindowControl(_) => Self::WindowControlError,
            PomoliceError::PersistenceSave(_) | PomoliceError::PersistenceTaskJoin { .. } => {
                Self::PersistenceError
            }
        }
    }
}

pub(crate) fn public_command_argument_rejection_error() -> InvokeError {
    InvokeError::from(serde_json::json!({
        "code": "invalid_command_input",
        "message": "Invalid command input"
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    use crate::error::PersistenceFailure;
    use crate::shared_tests::join_error::join_error_from_panic;
    use pomolice_core::settings::input::{DaySlotInput, SettingsInput};
    use pomolice_core::settings::UserSettings;
    use pomolice_core::timer_core::TimerStatus;

    fn persistence_failure(detail: &'static str) -> PersistenceFailure {
        PersistenceFailure::from_error(std::io::Error::other(detail))
    }

    #[test]
    fn command_errors_expose_codes_without_diagnostic_details() {
        let cases = [
            (
                PomoliceError::Unauthorized("settings".to_string()),
                "unauthorized_window",
                "Unauthorized window",
            ),
            (
                PomoliceError::Unauthorized("break-overlay-1".to_string()),
                "unauthorized_window",
                "Unauthorized window",
            ),
            (
                PomoliceError::InvalidStateTransition {
                    action: "start work".to_string(),
                    current_state: TimerStatus::Working,
                },
                "invalid_state_transition",
                "Invalid state transition",
            ),
            (
                PomoliceError::InvalidStateTransition {
                    action: "skip protected break".to_string(),
                    current_state: TimerStatus::OnBreak,
                },
                "invalid_state_transition",
                "Invalid state transition",
            ),
            (
                PomoliceError::RateLimited { remaining_ms: 1 },
                "rate_limited",
                "Rate limited",
            ),
            (
                PomoliceError::RateLimited {
                    remaining_ms: 8_675_309,
                },
                "rate_limited",
                "Rate limited",
            ),
            (
                PomoliceError::WindowControl(
                    "Failed to set fullscreen: platform API returned access denied".to_string(),
                ),
                "window_control_error",
                "Window control error",
            ),
            (
                PomoliceError::WindowControl("Main window not found".to_string()),
                "window_control_error",
                "Window control error",
            ),
            (
                PomoliceError::PersistenceSave(persistence_failure(
                    "failed to write timer_state.json: disk full",
                )),
                "persistence_error",
                "Persistence error",
            ),
            (
                PomoliceError::PersistenceSave(persistence_failure(
                    r"failed to commit C:\Users\example\private\timer_state.json: access denied",
                )),
                "persistence_error",
                "Persistence error",
            ),
            (
                PomoliceError::PersistenceTaskJoin {
                    source: join_error_from_panic("first private worker failure"),
                },
                "persistence_error",
                "Persistence error",
            ),
            (
                PomoliceError::PersistenceTaskJoin {
                    source: join_error_from_panic("second private worker failure"),
                },
                "persistence_error",
                "Persistence error",
            ),
        ];

        for (error, code, _message) in cases {
            assert_eq!(
                serde_json::to_value(BackendErrorResponse::from(&error))
                    .expect("command error should serialize"),
                json!({ "code": code }),
                "unexpected public response for {error:?}"
            );
        }
    }

    #[test]
    fn rejected_settings_expose_every_issue_alongside_the_public_envelope() {
        let mut invalid = SettingsInput {
            work_duration_secs: 0,
            ..UserSettings::default().into()
        };
        invalid.weekly_auto_run_config.mon = DaySlotInput {
            enabled: true,
            start_time: "22:00".into(),
            end_time: "02:00".into(),
        };
        invalid.weekly_auto_run_config.tue = DaySlotInput {
            enabled: true,
            start_time: "01:00".into(),
            end_time: "03:00".into(),
        };
        let error = PomoliceError::InvalidSettings(
            UserSettings::try_from(invalid).expect_err("invalid duration and overlapping schedule"),
        );

        assert_eq!(
            serde_json::to_value(BackendErrorResponse::from(&error))
                .expect("command error should serialize"),
            json!({
                "code": "invalid_settings",
                "issues": [
                    { "code": "out_of_range", "field": "work_duration_secs", "min": 1, "max": 7200 },
                    {
                        "code": "overlapping_time_ranges",
                        "primary_weekday": "mon",
                        "secondary_weekday": "tue"
                    },
                ],
            })
        );
    }

    #[test]
    fn invalid_command_arguments_expose_a_dedicated_public_error() {
        assert_eq!(
            public_command_argument_rejection_error().0,
            json!({
                "code": "invalid_command_input",
                "message": "Invalid command input",
            })
        );
    }
}
