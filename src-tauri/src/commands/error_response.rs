use serde::Serialize;
use tauri::ipc::InvokeError;

use crate::error::PomoliceError;

#[derive(
    Clone, Copy, Debug, Eq, PartialEq, serde::Serialize, strum::EnumCount, strum::EnumIter,
)]
#[serde(rename_all = "snake_case")]
pub(crate) enum BackendErrorCode {
    UnauthorizedWindow,
    InvalidStateTransition,
    RateLimited,
    WindowControlError,
    PersistenceError,
    InvalidCommandInput,
}

#[derive(serde::Serialize)]
pub(crate) struct BackendErrorResponse {
    code: BackendErrorCode,
    message: &'static str,
}

impl BackendErrorCode {
    pub(crate) fn public_message(self) -> &'static str {
        match self {
            Self::UnauthorizedWindow => "Unauthorized window",
            Self::InvalidStateTransition => "Invalid state transition",
            Self::RateLimited => "Rate limited",
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
        }
    }
}

impl From<&PomoliceError> for BackendErrorResponse {
    fn from(error: &PomoliceError) -> Self {
        let code = match error {
            PomoliceError::Unauthorized(_) => BackendErrorCode::UnauthorizedWindow,
            PomoliceError::InvalidStateTransition { .. } => {
                BackendErrorCode::InvalidStateTransition
            }
            PomoliceError::RateLimited { .. } => BackendErrorCode::RateLimited,
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

impl Serialize for PomoliceError {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        BackendErrorResponse::from(self).serialize(serializer)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::{BTreeMap, BTreeSet};

    use strum::IntoEnumIterator;

    use crate::error::PersistenceFailure;
    use crate::logging_event::{
        render_persistence_write_failure_message, PERSISTENCE_WRITE_TRIGGER_RUN_EVENT_EXIT,
    };
    use crate::shared_tests::join_error::join_error_from_panic;
    use crate::types::{TimerState, TimerStatus};

    struct ExpectedPublicError {
        code_name: &'static str,
        message: &'static str,
    }

    const PRESERVED_PUBLIC_ERRORS: &[ExpectedPublicError] = &[
        ExpectedPublicError {
            code_name: "unauthorized_window",
            message: "Unauthorized window",
        },
        ExpectedPublicError {
            code_name: "invalid_state_transition",
            message: "Invalid state transition",
        },
        ExpectedPublicError {
            code_name: "rate_limited",
            message: "Rate limited",
        },
        ExpectedPublicError {
            code_name: "window_control_error",
            message: "Window control error",
        },
        ExpectedPublicError {
            code_name: "persistence_error",
            message: "Persistence error",
        },
        ExpectedPublicError {
            code_name: "invalid_command_input",
            message: "Invalid command input",
        },
    ];

    const RATE_LIMIT_SENTINEL_REMAINING_MS: u64 = 8_675_309;
    const RATE_LIMIT_SENTINEL_FRAGMENT: &str = "8675309";

    fn preserved_public_error(code_name: &str) -> &'static ExpectedPublicError {
        let Some(expected_error) = PRESERVED_PUBLIC_ERRORS
            .iter()
            .find(|expected_error| expected_error.code_name == code_name)
        else {
            panic!("expected public error table should contain {code_name}");
        };

        expected_error
    }

    fn persistence_failure(detail: &'static str) -> PersistenceFailure {
        PersistenceFailure::from_error(std::io::Error::other(detail))
    }

    fn serialize_public_command_error(error: &PomoliceError) -> serde_json::Value {
        let Ok(serialized) = serde_json::to_value(error) else {
            panic!("error should serialize");
        };

        serialized
    }

    fn public_response_object(
        value: &serde_json::Value,
    ) -> &serde_json::Map<String, serde_json::Value> {
        let Some(response) = value.as_object() else {
            panic!("public backend error should serialize as a JSON object");
        };

        response
    }

    fn assert_public_response_has_only_code_and_message(value: &serde_json::Value) {
        let response = public_response_object(value);
        let actual_fields = response.keys().map(String::as_str).collect::<BTreeSet<_>>();
        let expected_fields = BTreeSet::from(["code", "message"]);

        assert_eq!(
            actual_fields, expected_fields,
            "public backend errors must expose only code and message"
        );
        assert!(
            matches!(response.get("code"), Some(code) if code.is_string()),
            "public backend error code should be a string"
        );
        assert!(
            matches!(response.get("message"), Some(message) if message.is_string()),
            "public backend error message should be a string"
        );
    }

    fn public_response_code(value: &serde_json::Value) -> &str {
        let Some(code) = public_response_object(value)
            .get("code")
            .and_then(serde_json::Value::as_str)
        else {
            panic!("public backend error should include a string code");
        };

        code
    }

    fn public_response_message(value: &serde_json::Value) -> &str {
        let Some(message) = public_response_object(value)
            .get("message")
            .and_then(serde_json::Value::as_str)
        else {
            panic!("public backend error should include a string message");
        };

        message
    }

    fn preserved_error_occurrences() -> Vec<(PomoliceError, &'static str)> {
        vec![
            (
                PomoliceError::Unauthorized("settings".to_string()),
                "unauthorized_window",
            ),
            (
                PomoliceError::InvalidStateTransition {
                    action: "skip protected break".to_string(),
                    current_state: TimerStatus::OnBreak,
                },
                "invalid_state_transition",
            ),
            (
                PomoliceError::RateLimited {
                    remaining_ms: 1_500,
                },
                "rate_limited",
            ),
            (
                PomoliceError::WindowControl("Failed to enter fullscreen".to_string()),
                "window_control_error",
            ),
            (
                PomoliceError::PersistenceSave(persistence_failure(
                    "failed to commit C:\\Users\\nak\\AppData\\Roaming\\Pomolice\\timer_state.json: disk full",
                )),
                "persistence_error",
            ),
            (
                PomoliceError::PersistenceTaskJoin {
                    source: join_error_from_panic("simulated persistence worker panic"),
                },
                "persistence_error",
            ),
        ]
    }

    fn message_stability_occurrences() -> Vec<(PomoliceError, &'static str)> {
        vec![
            (
                PomoliceError::Unauthorized("main".to_string()),
                "unauthorized_window",
            ),
            (
                PomoliceError::Unauthorized("break-overlay-1".to_string()),
                "unauthorized_window",
            ),
            (
                PomoliceError::InvalidStateTransition {
                    action: "start work".to_string(),
                    current_state: TimerStatus::Working,
                },
                "invalid_state_transition",
            ),
            (
                PomoliceError::InvalidStateTransition {
                    action: "resolve post-break decision".to_string(),
                    current_state: TimerStatus::Idle,
                },
                "invalid_state_transition",
            ),
            (
                PomoliceError::RateLimited { remaining_ms: 1 },
                "rate_limited",
            ),
            (
                PomoliceError::RateLimited {
                    remaining_ms: 9_999,
                },
                "rate_limited",
            ),
            (
                PomoliceError::WindowControl("Failed to show: access denied".to_string()),
                "window_control_error",
            ),
            (
                PomoliceError::WindowControl("Main window not found".to_string()),
                "window_control_error",
            ),
            (
                PomoliceError::PersistenceSave(persistence_failure(
                    "failed to open first.json: first source",
                )),
                "persistence_error",
            ),
            (
                PomoliceError::PersistenceTaskJoin {
                    source: join_error_from_panic("second source"),
                },
                "persistence_error",
            ),
        ]
    }

    fn sentinel_error_occurrences() -> Vec<(PomoliceError, Vec<&'static str>)> {
        vec![
            (
                PomoliceError::Unauthorized("sentinel-window-secret".to_string()),
                vec!["sentinel-window-secret"],
            ),
            (
                PomoliceError::InvalidStateTransition {
                    action: "sentinel-action-secret".to_string(),
                    current_state: TimerStatus::OnBreak,
                },
                vec!["sentinel-action-secret", "OnBreak"],
            ),
            (
                PomoliceError::RateLimited {
                    remaining_ms: RATE_LIMIT_SENTINEL_REMAINING_MS,
                },
                vec![RATE_LIMIT_SENTINEL_FRAGMENT],
            ),
            (
                PomoliceError::WindowControl("sentinel-window-control-secret".to_string()),
                vec!["sentinel-window-control-secret"],
            ),
            (
                PomoliceError::PersistenceSave(persistence_failure(
                    "C:\\sentinel\\commit\\timer_state.json: sentinel-commit-secret",
                )),
                vec![
                    "C:\\sentinel\\commit",
                    "timer_state.json",
                    "sentinel-commit-secret",
                ],
            ),
            (
                PomoliceError::PersistenceTaskJoin {
                    source: join_error_from_panic("sentinel-join-secret"),
                },
                vec!["sentinel-join-secret"],
            ),
        ]
    }

    fn assert_public_backend_error_response(
        error: &PomoliceError,
        expected_code: &str,
        expected_message: &str,
    ) -> serde_json::Value {
        let serialized = serde_json::to_value(error).expect("error should serialize");
        assert_public_response_has_only_code_and_message(&serialized);
        assert_eq!(
            public_response_code(&serialized),
            expected_code,
            "serialized error should preserve the public error code"
        );
        assert_eq!(
            public_response_message(&serialized),
            expected_message,
            "serialized error should preserve the public error message"
        );

        serialized
    }

    fn assert_not_public_command_error_envelope(value: &serde_json::Value) {
        let Some(response) = value.as_object() else {
            return;
        };

        let has_code_field = matches!(response.get("code"), Some(code) if code.is_string());
        let has_message_field =
            matches!(response.get("message"), Some(message) if message.is_string());
        let has_public_command_error_fields = has_code_field && has_message_field;

        assert!(
            !has_public_command_error_fields,
            "excluded surface serialized as a public command error envelope: {value}"
        );
    }

    #[test]
    fn command_error_response_serializes_to_only_code_and_message() {
        for (error, _) in preserved_error_occurrences() {
            let serialized = serialize_public_command_error(&error);

            assert_public_response_has_only_code_and_message(&serialized);
        }

        let command_input_error = public_command_argument_rejection_error();
        assert_public_response_has_only_code_and_message(&command_input_error.0);
    }

    #[test]
    fn command_error_response_codes_and_messages_are_stable() {
        let mut observed_codes = BTreeSet::new();

        for (error, expected_code) in preserved_error_occurrences() {
            let serialized = serialize_public_command_error(&error);
            let expected_error = preserved_public_error(expected_code);

            assert_eq!(public_response_code(&serialized), expected_error.code_name);
            assert_eq!(public_response_message(&serialized), expected_error.message);
            observed_codes.insert(expected_error.code_name);
        }

        let command_input_error = public_command_argument_rejection_error();
        let expected_error = preserved_public_error("invalid_command_input");

        assert_eq!(
            public_response_code(&command_input_error.0),
            expected_error.code_name
        );
        assert_eq!(
            public_response_message(&command_input_error.0),
            expected_error.message
        );
        observed_codes.insert(expected_error.code_name);

        let expected_codes = PRESERVED_PUBLIC_ERRORS
            .iter()
            .map(|expected_error| expected_error.code_name)
            .collect::<BTreeSet<_>>();

        assert_eq!(
            observed_codes, expected_codes,
            "public command error fixtures should exercise every preserved public code"
        );
    }

    #[test]
    fn command_error_response_every_error_code_has_a_public_message() {
        for code in BackendErrorCode::iter() {
            let serialized = serde_json::to_value(BackendErrorResponse::from_code(code))
                .expect("public command error should serialize");

            assert_public_response_has_only_code_and_message(&serialized);
            assert!(
                !public_response_message(&serialized).is_empty(),
                "public code {code:?} should have a non-empty public message"
            );
        }
    }

    #[test]
    fn command_error_response_public_message_is_stable_per_code() {
        let mut messages_by_code = BTreeMap::new();

        for (error, expected_code) in message_stability_occurrences() {
            let serialized = serialize_public_command_error(&error);
            let expected_error = preserved_public_error(expected_code);
            let public_message = public_response_message(&serialized);

            assert_eq!(public_response_code(&serialized), expected_error.code_name);
            assert_eq!(public_message, expected_error.message);

            if let Some(previous_message) =
                messages_by_code.insert(expected_code.to_string(), public_message.to_string())
            {
                assert_eq!(
                    previous_message, public_message,
                    "public message should not vary by error occurrence for {expected_code}"
                );
            }
        }
    }

    #[test]
    fn command_error_response_redacts_internal_details() {
        for (error, hidden_fragments) in sentinel_error_occurrences() {
            let serialized = serialize_public_command_error(&error);

            assert_public_response_has_only_code_and_message(&serialized);
            let serialized_text = serialized.to_string();
            for hidden_fragment in hidden_fragments {
                assert!(
                    !serialized_text.contains(hidden_fragment),
                    "public backend error leaked diagnostic fragment {hidden_fragment:?}"
                );
            }
        }

        let command_input_error = public_command_argument_rejection_error();
        assert_public_response_has_only_code_and_message(&command_input_error.0);
    }

    #[test]
    fn command_error_response_invalid_command_input_uses_dedicated_public_code() {
        let command_input_error = public_command_argument_rejection_error();

        assert_eq!(
            public_response_code(&command_input_error.0),
            "invalid_command_input"
        );
        assert_eq!(
            public_response_message(&command_input_error.0),
            "Invalid command input"
        );
    }

    #[test]
    fn command_error_response_persistence_failure_is_classified_without_exposing_diagnostics() {
        let error = PomoliceError::PersistenceSave(persistence_failure(
            "failed to commit C:\\Users\\nak\\AppData\\Roaming\\Pomolice\\timer_state.json: disk full",
        ));

        assert!(
            error.to_string().contains("timer_state.json"),
            "backend diagnostic display should retain persistence context"
        );
        assert!(
            error.to_string().contains("disk full"),
            "backend diagnostic display should retain persistence source"
        );

        let serialized =
            assert_public_backend_error_response(&error, "persistence_error", "Persistence error")
                .to_string();

        assert!(!serialized.contains("timer_state.json"));
        assert!(!serialized.contains("AppData"));
        assert!(!serialized.contains("disk full"));
    }

    #[test]
    fn command_error_response_window_control_failure_is_classified_without_exposing_diagnostics() {
        let error = PomoliceError::WindowControl(
            "Failed to set fullscreen for main window: platform API returned access denied"
                .to_string(),
        );

        let serialized = assert_public_backend_error_response(
            &error,
            "window_control_error",
            "Window control error",
        )
        .to_string();

        assert!(!serialized.contains("Failed to set fullscreen"));
        assert!(!serialized.contains("main window"));
        assert!(!serialized.contains("platform API"));
        assert!(!serialized.contains("access denied"));
    }

    #[test]
    fn command_error_response_task_join_failure_uses_persistence_error_without_exposing_panic_detail(
    ) {
        let error = PomoliceError::PersistenceTaskJoin {
            source: join_error_from_panic("simulated persistence worker panic"),
        };

        let serialized =
            assert_public_backend_error_response(&error, "persistence_error", "Persistence error")
                .to_string();

        assert!(!serialized.contains("simulated persistence worker panic"));
    }

    #[test]
    fn command_error_response_is_not_used_by_non_command_payloads() {
        let timer_state = TimerState::default();
        let event_payload =
            serde_json::to_value(timer_state.to_tick_payload()).expect("event payload serializes");
        let success_payload = serde_json::to_value(timer_state.to_full_payload())
            .expect("success payload serializes");
        let notification_error =
            tauri_plugin_notification::Error::Io(std::io::Error::other("toast failed"));
        let notification_failure =
            serde_json::to_value(notification_error).expect("notification error should serialize");
        let log_storage_failure = serde_json::to_value(render_persistence_write_failure_message(
            PERSISTENCE_WRITE_TRIGGER_RUN_EVENT_EXIT,
            &std::io::Error::other("disk full"),
        ))
        .expect("log message should serialize");

        assert_not_public_command_error_envelope(&event_payload);
        assert_not_public_command_error_envelope(&success_payload);
        assert_not_public_command_error_envelope(&notification_failure);
        assert_not_public_command_error_envelope(&log_storage_failure);
    }
}
