// Error types for Pomolice application
// Provides structured error handling across all modules

use tauri::ipc::InvokeError;
use thiserror::Error;

use crate::managers::persistence::SaveError;
use crate::types::TimerStatus;

/// Main error type for the Pomolice application
#[derive(Error, Debug, strum::EnumDiscriminants)]
#[strum_discriminants(derive(strum::EnumIter, PartialOrd, Ord))]
pub(crate) enum PomoliceError {
    #[error("Unauthorized window: {0}")]
    Unauthorized(String),

    #[error("Invalid state transition: cannot {action} from {current_state:?}")]
    InvalidStateTransition {
        action: String,
        current_state: TimerStatus,
    },

    #[error("Rate limited: please wait {remaining_ms}ms")]
    RateLimited { remaining_ms: u64 },

    #[error("Window control error: {0}")]
    WindowControl(String),

    #[error(transparent)]
    PersistenceSave(#[from] SaveError),

    #[error("failed to join persistence worker task: {source}")]
    PersistenceTaskJoin {
        #[source]
        source: tokio::task::JoinError,
    },
}

/// Result type alias for Pomolice operations
pub(crate) type Result<T> = std::result::Result<T, PomoliceError>;

#[derive(
    Clone, Copy, Debug, Eq, PartialEq, serde::Serialize, strum::EnumCount, strum::EnumIter,
)]
#[serde(rename_all = "snake_case")]
enum BackendErrorCode {
    UnauthorizedWindow,
    InvalidStateTransition,
    RateLimited,
    WindowControlError,
    PersistenceError,
    InvalidCommandInput,
}

#[derive(serde::Serialize)]
struct BackendErrorResponse {
    code: BackendErrorCode,
    message: &'static str,
}

impl BackendErrorCode {
    fn public_message(self) -> &'static str {
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
    fn from_code(code: BackendErrorCode) -> Self {
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

impl serde::Serialize for PomoliceError {
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

    use crate::logging_event::{
        render_persistence_write_failure_message, PERSISTENCE_WRITE_TRIGGER_RUN_EVENT_EXIT,
    };
    use crate::types::TimerState;
    use strum::{EnumCount, IntoEnumIterator};

    struct ExpectedPublicError {
        code: BackendErrorCode,
        code_name: &'static str,
        message: &'static str,
    }

    const PRESERVED_PUBLIC_ERRORS: &[ExpectedPublicError] = &[
        ExpectedPublicError {
            code: BackendErrorCode::UnauthorizedWindow,
            code_name: "unauthorized_window",
            message: "Unauthorized window",
        },
        ExpectedPublicError {
            code: BackendErrorCode::InvalidStateTransition,
            code_name: "invalid_state_transition",
            message: "Invalid state transition",
        },
        ExpectedPublicError {
            code: BackendErrorCode::RateLimited,
            code_name: "rate_limited",
            message: "Rate limited",
        },
        ExpectedPublicError {
            code: BackendErrorCode::WindowControlError,
            code_name: "window_control_error",
            message: "Window control error",
        },
        ExpectedPublicError {
            code: BackendErrorCode::PersistenceError,
            code_name: "persistence_error",
            message: "Persistence error",
        },
        ExpectedPublicError {
            code: BackendErrorCode::InvalidCommandInput,
            code_name: "invalid_command_input",
            message: "Invalid command input",
        },
    ];

    const RATE_LIMIT_SENTINEL_REMAINING_MS: u64 = 8_675_309;
    const RATE_LIMIT_SENTINEL_FRAGMENT: &str = "8675309";

    fn preserved_public_error(code: BackendErrorCode) -> &'static ExpectedPublicError {
        let Some(expected_error) = PRESERVED_PUBLIC_ERRORS
            .iter()
            .find(|expected_error| expected_error.code == code)
        else {
            panic!("expected public error table should contain {code:?}");
        };

        expected_error
    }

    fn serialize_backend_error_code(code: BackendErrorCode) -> String {
        let Ok(serialized) = serde_json::to_value(code) else {
            panic!("backend error code should serialize");
        };
        let Some(code_name) = serialized.as_str() else {
            panic!("backend error code should serialize as a string");
        };

        code_name.to_string()
    }

    fn serialize_public_error(error: &PomoliceError) -> serde_json::Value {
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

    fn preserved_error_occurrences() -> Vec<(PomoliceError, BackendErrorCode)> {
        vec![
            (
                PomoliceError::Unauthorized("settings".to_string()),
                BackendErrorCode::UnauthorizedWindow,
            ),
            (
                PomoliceError::InvalidStateTransition {
                    action: "skip protected break".to_string(),
                    current_state: TimerStatus::OnBreak,
                },
                BackendErrorCode::InvalidStateTransition,
            ),
            (
                PomoliceError::RateLimited {
                    remaining_ms: 1_500,
                },
                BackendErrorCode::RateLimited,
            ),
            (
                PomoliceError::WindowControl("Failed to enter fullscreen".to_string()),
                BackendErrorCode::WindowControlError,
            ),
            (
                PomoliceError::from(SaveError::Commit {
                    path: "C:\\Users\\nak\\AppData\\Roaming\\Pomolice\\timer_state.json".into(),
                    source: std::io::Error::other("disk full"),
                }),
                BackendErrorCode::PersistenceError,
            ),
            (
                PomoliceError::PersistenceTaskJoin {
                    source: join_error_from_panic("simulated persistence worker panic"),
                },
                BackendErrorCode::PersistenceError,
            ),
        ]
    }

    fn message_stability_occurrences() -> Vec<(PomoliceError, BackendErrorCode)> {
        vec![
            (
                PomoliceError::Unauthorized("main".to_string()),
                BackendErrorCode::UnauthorizedWindow,
            ),
            (
                PomoliceError::Unauthorized("break-overlay-1".to_string()),
                BackendErrorCode::UnauthorizedWindow,
            ),
            (
                PomoliceError::InvalidStateTransition {
                    action: "start work".to_string(),
                    current_state: TimerStatus::Working,
                },
                BackendErrorCode::InvalidStateTransition,
            ),
            (
                PomoliceError::InvalidStateTransition {
                    action: "resolve post-break decision".to_string(),
                    current_state: TimerStatus::Idle,
                },
                BackendErrorCode::InvalidStateTransition,
            ),
            (
                PomoliceError::RateLimited { remaining_ms: 1 },
                BackendErrorCode::RateLimited,
            ),
            (
                PomoliceError::RateLimited {
                    remaining_ms: 9_999,
                },
                BackendErrorCode::RateLimited,
            ),
            (
                PomoliceError::WindowControl("Failed to show: access denied".to_string()),
                BackendErrorCode::WindowControlError,
            ),
            (
                PomoliceError::WindowControl("Main window not found".to_string()),
                BackendErrorCode::WindowControlError,
            ),
            (
                PomoliceError::from(SaveError::Open {
                    path: "first.json".into(),
                    source: std::io::Error::other("first source"),
                }),
                BackendErrorCode::PersistenceError,
            ),
            (
                PomoliceError::PersistenceTaskJoin {
                    source: join_error_from_panic("second source"),
                },
                BackendErrorCode::PersistenceError,
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
                PomoliceError::from(SaveError::CreateDir {
                    path: "C:\\sentinel\\createdir\\timer_state.json".into(),
                    source: std::io::Error::other("sentinel-createdir-secret"),
                }),
                vec![
                    "C:\\sentinel\\createdir",
                    "timer_state.json",
                    "sentinel-createdir-secret",
                ],
            ),
            (
                PomoliceError::from(SaveError::Serialize {
                    source: serde_json::Error::io(std::io::Error::other(
                        "sentinel-serialize-secret",
                    )),
                }),
                vec!["sentinel-serialize-secret"],
            ),
            (
                PomoliceError::from(SaveError::Open {
                    path: "C:\\sentinel\\open\\timer_state.json".into(),
                    source: std::io::Error::other("sentinel-open-secret"),
                }),
                vec![
                    "C:\\sentinel\\open",
                    "timer_state.json",
                    "sentinel-open-secret",
                ],
            ),
            (
                PomoliceError::from(SaveError::Write {
                    path: "C:\\sentinel\\write\\timer_state.json".into(),
                    source: std::io::Error::other("sentinel-write-secret"),
                }),
                vec![
                    "C:\\sentinel\\write",
                    "timer_state.json",
                    "sentinel-write-secret",
                ],
            ),
            (
                PomoliceError::from(SaveError::Commit {
                    path: "C:\\sentinel\\commit\\timer_state.json".into(),
                    source: std::io::Error::other("sentinel-commit-secret"),
                }),
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

    fn join_error_from_panic(message: &'static str) -> tokio::task::JoinError {
        let Ok(runtime) = tokio::runtime::Runtime::new() else {
            panic!("test runtime should be created");
        };

        runtime.block_on(async move {
            let join_result = tokio::spawn(async move {
                panic!("{message}");
            })
            .await;
            let Err(join_error) = join_result else {
                panic!("spawned panic should surface as join error");
            };

            join_error
        })
    }

    fn assert_all_pomolice_error_variants_are_represented(
        occurrences: &[(PomoliceError, Vec<&'static str>)],
    ) {
        let actual_variants = occurrences
            .iter()
            .map(|(error, _)| PomoliceErrorDiscriminants::from(error))
            .collect::<BTreeSet<_>>();
        let expected_variants = PomoliceErrorDiscriminants::iter().collect::<BTreeSet<_>>();

        assert_eq!(
            actual_variants, expected_variants,
            "sentinel coverage should include every public command error variant"
        );
    }

    fn assert_display_includes_diagnostic_details(
        error: &PomoliceError,
        expected_details: &[&str],
    ) {
        let message = error.to_string();

        for &expected_detail in expected_details {
            assert!(
                message.contains(expected_detail),
                "display message {message:?} should include diagnostic detail {expected_detail:?}"
            );
        }
    }

    #[test]
    fn test_unauthorized_error_display_keeps_window_label() {
        let err = PomoliceError::Unauthorized("test-window".to_string());

        assert_display_includes_diagnostic_details(&err, &["test-window"]);
    }

    #[test]
    fn test_invalid_state_transition_error_display_keeps_action_and_state() {
        let err = PomoliceError::InvalidStateTransition {
            action: "start work".to_string(),
            current_state: TimerStatus::Working,
        };

        assert_display_includes_diagnostic_details(
            &err,
            &["Invalid state transition", "start work", "Working"],
        );
    }

    #[test]
    fn test_rate_limited_error_display_keeps_remaining_ms() {
        let err = PomoliceError::RateLimited { remaining_ms: 500 };

        assert_display_includes_diagnostic_details(&err, &["500"]);
    }

    #[test]
    fn test_window_control_error_display_keeps_diagnostic_detail() {
        let err = PomoliceError::WindowControl("Failed to enter fullscreen".to_string());

        assert_display_includes_diagnostic_details(&err, &["Failed to enter fullscreen"]);
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
    fn test_error_serialization_uses_public_allowlisted_response() {
        let err = PomoliceError::RateLimited { remaining_ms: 300 };

        assert_public_backend_error_response(&err, "rate_limited", "Rate limited");
    }

    #[test]
    fn test_error_serializes_to_only_code_and_message() {
        for (error, _) in preserved_error_occurrences() {
            let serialized = serialize_public_error(&error);

            assert_public_response_has_only_code_and_message(&serialized);
        }

        let command_input_error = public_command_argument_rejection_error();
        assert_public_response_has_only_code_and_message(&command_input_error.0);
    }

    #[test]
    fn test_serialized_codes_and_messages_are_preserved() {
        assert_eq!(
            PRESERVED_PUBLIC_ERRORS.len(),
            BackendErrorCode::COUNT,
            "preserved public error table should cover every public code"
        );

        for expected_error in PRESERVED_PUBLIC_ERRORS {
            assert_eq!(
                serialize_backend_error_code(expected_error.code),
                expected_error.code_name,
                "public error code string should remain stable"
            );
        }

        for (error, code) in preserved_error_occurrences() {
            let serialized = serialize_public_error(&error);
            let expected_error = preserved_public_error(code);

            assert_eq!(public_response_code(&serialized), expected_error.code_name);
            assert_eq!(public_response_message(&serialized), expected_error.message);
        }

        let command_input_error = public_command_argument_rejection_error();
        let expected_error = preserved_public_error(BackendErrorCode::InvalidCommandInput);

        assert_eq!(
            public_response_code(&command_input_error.0),
            expected_error.code_name
        );
        assert_eq!(
            public_response_message(&command_input_error.0),
            expected_error.message
        );
    }

    #[test]
    fn test_every_error_code_has_a_public_message() {
        for code in BackendErrorCode::iter() {
            assert!(
                !code.public_message().is_empty(),
                "public code {code:?} should have a non-empty public message"
            );
        }
    }

    #[test]
    fn test_public_message_is_stable_per_code() {
        let mut messages_by_code = BTreeMap::new();

        for (error, expected_code) in message_stability_occurrences() {
            let serialized = serialize_public_error(&error);
            let code_name = serialize_backend_error_code(expected_code);
            let public_message = public_response_message(&serialized);
            let catalog_message = expected_code.public_message();

            assert_eq!(public_response_code(&serialized), code_name);
            assert_eq!(public_message, catalog_message);

            if let Some(previous_message) =
                messages_by_code.insert(code_name.clone(), public_message.to_string())
            {
                assert_eq!(
                    previous_message, public_message,
                    "public message should not vary by error occurrence for {code_name}"
                );
            }
        }

        for expected_error in PRESERVED_PUBLIC_ERRORS {
            let response = BackendErrorResponse::from_code(expected_error.code);

            assert_eq!(
                response.message, expected_error.message,
                "public response should preserve the catalog message for {}",
                expected_error.code_name
            );
        }
    }

    #[test]
    fn test_no_variant_leaks_internal_detail_into_message() {
        let occurrences = sentinel_error_occurrences();

        assert_all_pomolice_error_variants_are_represented(&occurrences);

        for (error, hidden_fragments) in occurrences {
            let serialized = serialize_public_error(&error);

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
    fn test_persistence_save_error_conversion() {
        let error = SaveError::Commit {
            path: "timer_state.json".into(),
            source: std::io::Error::other("disk full"),
        };

        let pomolice_error: PomoliceError = error.into();

        assert!(matches!(pomolice_error, PomoliceError::PersistenceSave(_)));
        assert!(pomolice_error.to_string().contains("disk full"));
    }

    #[test]
    fn test_window_control_serialization_redacts_internal_detail() {
        let error = PomoliceError::WindowControl(
            "Failed to set fullscreen for main window: platform API returned access denied"
                .to_string(),
        );

        let serialized = assert_public_backend_error_response(
            &error,
            "window_control_error",
            "Window control error",
        );
        let serialized = serialized.to_string();

        assert!(!serialized.contains("Failed to set fullscreen"));
        assert!(!serialized.contains("main window"));
        assert!(!serialized.contains("platform API"));
        assert!(!serialized.contains("access denied"));
    }

    #[test]
    fn test_non_target_error_serialization_uses_explicit_safe_responses() {
        let unauthorized = PomoliceError::Unauthorized("settings".to_string());
        assert_public_backend_error_response(
            &unauthorized,
            "unauthorized_window",
            "Unauthorized window",
        );

        let invalid_transition = PomoliceError::InvalidStateTransition {
            action: "skip protected break".to_string(),
            current_state: TimerStatus::OnBreak,
        };
        assert_public_backend_error_response(
            &invalid_transition,
            "invalid_state_transition",
            "Invalid state transition",
        );

        let rate_limited = PomoliceError::RateLimited {
            remaining_ms: 1_500,
        };
        assert_public_backend_error_response(&rate_limited, "rate_limited", "Rate limited");

        let error = PomoliceError::from(SaveError::Commit {
            path: "C:\\Users\\nak\\AppData\\Roaming\\Pomolice\\timer_state.json".into(),
            source: std::io::Error::other("disk full"),
        });
        assert_public_backend_error_response(&error, "persistence_error", "Persistence error");

        let runtime = tokio::runtime::Runtime::new().expect("test runtime should be created");
        let join_error = runtime.block_on(async {
            tokio::spawn(async move {
                panic!("simulated persistence worker panic");
            })
            .await
            .expect_err("spawned panic should surface as join error")
        });
        let join_error = PomoliceError::PersistenceTaskJoin { source: join_error };
        assert_public_backend_error_response(&join_error, "persistence_error", "Persistence error");

        let serialized =
            serde_json::to_string(&[unauthorized, invalid_transition, rate_limited, error])
                .expect("errors should serialize");

        assert!(!serialized.contains("settings"));
        assert!(!serialized.contains("skip protected break"));
        assert!(!serialized.contains("OnBreak"));
        assert!(!serialized.contains("1500"));
        assert!(!serialized.contains("timer_state.json"));
        assert!(!serialized.contains("AppData"));
        assert!(!serialized.contains("disk full"));
    }

    #[test]
    fn test_tick_payload_does_not_use_public_command_error_envelope() {
        let timer_state = TimerState::default();
        let event_payload =
            serde_json::to_value(timer_state.to_tick_payload()).expect("event payload serializes");

        assert_not_public_command_error_envelope(&event_payload);
    }

    #[test]
    fn test_full_timer_payload_does_not_use_public_command_error_envelope() {
        let timer_state = TimerState::default();
        let success_payload = serde_json::to_value(timer_state.to_full_payload())
            .expect("success payload serializes");

        assert_not_public_command_error_envelope(&success_payload);
    }

    #[test]
    fn test_notification_error_does_not_use_public_command_error_envelope() {
        let notification_error =
            tauri_plugin_notification::Error::Io(std::io::Error::other("toast failed"));
        let notification_failure =
            serde_json::to_value(notification_error).expect("notification error should serialize");

        assert_not_public_command_error_envelope(&notification_failure);
    }

    #[test]
    fn test_log_message_does_not_use_public_command_error_envelope() {
        let log_storage_failure = serde_json::to_value(render_persistence_write_failure_message(
            PERSISTENCE_WRITE_TRIGGER_RUN_EVENT_EXIT,
            &std::io::Error::other("disk full"),
        ))
        .expect("log message should serialize");

        assert_not_public_command_error_envelope(&log_storage_failure);
    }

    #[test]
    fn test_persistence_save_error_display_keeps_storage_path_and_source() {
        let error = PomoliceError::from(SaveError::Write {
            path: "timer_state.json".into(),
            source: std::io::Error::other("permission denied"),
        });

        assert_display_includes_diagnostic_details(
            &error,
            &["timer_state.json", "permission denied"],
        );
    }

    #[test]
    fn test_persistence_save_serialization_redacts_storage_path_details() {
        let error = PomoliceError::from(SaveError::Commit {
            path: "C:\\Users\\nak\\AppData\\Roaming\\Pomolice\\timer_state.json".into(),
            source: std::io::Error::other("disk full"),
        });

        let serialized =
            assert_public_backend_error_response(&error, "persistence_error", "Persistence error")
                .to_string();

        assert!(!serialized.contains("timer_state.json"));
        assert!(!serialized.contains("AppData"));
        assert!(!serialized.contains("disk full"));
    }

    #[test]
    fn test_persistence_task_join_serialization_uses_redacted_message() {
        let runtime = tokio::runtime::Runtime::new().expect("test runtime should be created");
        let join_error = runtime.block_on(async {
            tokio::spawn(async move {
                panic!("simulated persistence worker panic");
            })
            .await
            .expect_err("spawned panic should surface as join error")
        });
        let error = PomoliceError::PersistenceTaskJoin { source: join_error };

        let serialized =
            assert_public_backend_error_response(&error, "persistence_error", "Persistence error")
                .to_string();

        assert!(!serialized.contains("simulated persistence worker panic"));
    }
}
