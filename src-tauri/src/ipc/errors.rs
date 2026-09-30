use tauri::ipc::InvokeError;

use crate::error::PomoliceError;
use crate::ipc::settings::SettingsValidationIssue;

#[derive(
    Clone,
    Copy,
    Debug,
    Eq,
    PartialEq,
    serde::Serialize,
    strum::EnumCount,
    strum::EnumIter,
    specta::Type,
)]
#[serde(rename_all = "snake_case")]
pub(crate) enum BackendErrorCode {
    UnauthorizedWindow,
    InvalidStateTransition,
    RateLimited,
    InvalidSettings,
    WindowControlError,
    PersistenceError,
    InvalidCommandInput,
}

#[derive(serde::Serialize, specta::Type)]
pub(crate) struct BackendErrorResponse {
    code: BackendErrorCode,
    message: &'static str,
    /// Present only for rejected settings, naming every input to fix.
    #[serde(skip_serializing_if = "Option::is_none")]
    issues: Option<Vec<SettingsValidationIssue>>,
}

impl BackendErrorCode {
    pub(crate) fn public_message(self) -> &'static str {
        match self {
            Self::UnauthorizedWindow => "Unauthorized window",
            Self::InvalidStateTransition => "Invalid state transition",
            Self::RateLimited => "Rate limited",
            Self::InvalidSettings => "Invalid settings",
            Self::WindowControlError => "Window control error",
            Self::PersistenceError => "Persistence error",
            Self::InvalidCommandInput => "Invalid command input",
        }
    }
}

impl BackendErrorResponse {
    pub(crate) fn from_code(code: BackendErrorCode) -> Self {
        Self {
            code,
            message: code.public_message(),
            issues: None,
        }
    }
}

impl From<&PomoliceError> for BackendErrorResponse {
    fn from(error: &PomoliceError) -> Self {
        if let PomoliceError::InvalidSettings(errors) = error {
            return Self {
                issues: Some(errors.issues().to_vec()),
                ..Self::from_code(BackendErrorCode::InvalidSettings)
            };
        }

        let code = match error {
            PomoliceError::Unauthorized(_) => BackendErrorCode::UnauthorizedWindow,
            PomoliceError::InvalidStateTransition { .. } => {
                BackendErrorCode::InvalidStateTransition
            }
            PomoliceError::RateLimited { .. } => BackendErrorCode::RateLimited,
            PomoliceError::InvalidSettings(_) => BackendErrorCode::InvalidSettings,
            PomoliceError::WindowControl(_) => BackendErrorCode::WindowControlError,
            PomoliceError::PersistenceSave(_) | PomoliceError::PersistenceTaskJoin { .. } => {
                BackendErrorCode::PersistenceError
            }
        };

        Self::from_code(code)
    }
}

pub(crate) fn public_command_argument_rejection_error() -> InvokeError {
    InvokeError::from(BackendErrorResponse::from_code(
        BackendErrorCode::InvalidCommandInput,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    use crate::error::PersistenceFailure;
    use crate::ipc::settings::RangedSettingField;
    use crate::settings::SettingsValidationErrors;
    use crate::shared_tests::join_error::join_error_from_panic;
    use crate::types::{TimerStatus, WeekdayKey};

    fn persistence_failure(detail: &'static str) -> PersistenceFailure {
        PersistenceFailure::from_error(std::io::Error::other(detail))
    }

    #[test]
    fn command_errors_expose_stable_codes_and_messages_without_diagnostic_details() {
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

        for (error, code, message) in cases {
            assert_eq!(
                serde_json::to_value(BackendErrorResponse::from(&error))
                    .expect("command error should serialize"),
                json!({ "code": code, "message": message }),
                "unexpected public response for {error:?}"
            );
        }
    }

    #[test]
    fn rejected_settings_expose_every_issue_alongside_the_public_envelope() {
        let error = PomoliceError::InvalidSettings(SettingsValidationErrors::from_issues(vec![
            SettingsValidationIssue::OutOfRange {
                field: RangedSettingField::WorkDurationSecs,
                min: 1,
                max: 7_200,
            },
            SettingsValidationIssue::OverlappingTimeRanges {
                primary_weekday: WeekdayKey::Mon,
                secondary_weekday: WeekdayKey::Tue,
            },
        ]));

        assert_eq!(
            serde_json::to_value(BackendErrorResponse::from(&error))
                .expect("command error should serialize"),
            json!({
                "code": "invalid_settings",
                "message": "Invalid settings",
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
