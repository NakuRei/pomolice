// Error types for Pomolice application
// Provides structured error handling across all modules

use tauri::ipc::InvokeError;
use thiserror::Error;

use crate::managers::persistence::SaveError;
use crate::types::TimerStatus;

/// Main error type for the Pomolice application
#[derive(Error, Debug)]
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

#[derive(serde::Serialize)]
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

impl From<&PomoliceError> for BackendErrorResponse {
    fn from(error: &PomoliceError) -> Self {
        match error {
            PomoliceError::Unauthorized(_) => Self {
                code: BackendErrorCode::UnauthorizedWindow,
                message: "Unauthorized window",
            },
            PomoliceError::InvalidStateTransition { .. } => Self {
                code: BackendErrorCode::InvalidStateTransition,
                message: "Invalid state transition",
            },
            PomoliceError::RateLimited { .. } => Self {
                code: BackendErrorCode::RateLimited,
                message: "Rate limited",
            },
            PomoliceError::WindowControl(_) => Self {
                code: BackendErrorCode::WindowControlError,
                message: "Window control error",
            },
            PomoliceError::PersistenceSave(_) | PomoliceError::PersistenceTaskJoin { .. } => Self {
                code: BackendErrorCode::PersistenceError,
                message: "Persistence error",
            },
        }
    }
}

pub(crate) fn public_command_argument_rejection_error() -> InvokeError {
    InvokeError::from(BackendErrorResponse {
        code: BackendErrorCode::InvalidCommandInput,
        message: "Invalid command input",
    })
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
    use crate::logging_event::{
        render_persistence_write_failure_message, PERSISTENCE_WRITE_TRIGGER_RUN_EVENT_EXIT,
    };
    use crate::types::TimerState;

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
        let response = serialized
            .as_object()
            .expect("serialized error should be a JSON object");

        let allowed_fields = ["code", "message"];
        assert_eq!(
            response.len(),
            allowed_fields.len(),
            "public backend errors must expose only allowlisted fields"
        );
        for allowed_field in allowed_fields {
            assert!(
                response.contains_key(allowed_field),
                "public backend errors should include allowlisted field {allowed_field:?}"
            );
        }
        assert_eq!(
            response
                .get("code")
                .expect("response should include code")
                .as_str(),
            Some(expected_code)
        );
        assert_eq!(
            response
                .get("message")
                .expect("response should include message")
                .as_str(),
            Some(expected_message)
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
    fn test_excluded_surfaces_do_not_use_public_command_error_envelope() {
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

        for excluded_surface in [
            event_payload,
            success_payload,
            notification_failure,
            log_storage_failure,
        ] {
            assert_not_public_command_error_envelope(&excluded_surface);
        }
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
