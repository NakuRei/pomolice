// Error types for Pomolice application
// Provides structured error handling across all modules

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

    #[test]
    fn test_unauthorized_error() {
        let err = PomoliceError::Unauthorized("test-window".to_string());
        assert_eq!(err.to_string(), "Unauthorized window: test-window");
    }

    #[test]
    fn test_invalid_state_transition_error() {
        let err = PomoliceError::InvalidStateTransition {
            action: "start work".to_string(),
            current_state: TimerStatus::Working,
        };
        let msg = err.to_string();
        assert!(msg.contains("Invalid state transition"));
        assert!(msg.contains("start work"));
        assert!(msg.contains("Working"));
    }

    #[test]
    fn test_rate_limited_error() {
        let err = PomoliceError::RateLimited { remaining_ms: 500 };
        assert_eq!(err.to_string(), "Rate limited: please wait 500ms");
    }

    #[test]
    fn test_window_control_error() {
        let err = PomoliceError::WindowControl("Failed to enter fullscreen".to_string());
        assert_eq!(
            err.to_string(),
            "Window control error: Failed to enter fullscreen"
        );
    }

    fn assert_serialized_response(
        error: &PomoliceError,
        expected_code: &str,
        expected_message: &str,
    ) -> serde_json::Value {
        let serialized = serde_json::to_value(error).expect("error should serialize");
        let response = serialized
            .as_object()
            .expect("serialized error should be a JSON object");

        assert_eq!(response.len(), 2);
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

    #[test]
    fn test_error_serialization_uses_structured_response() {
        let err = PomoliceError::RateLimited { remaining_ms: 300 };

        assert_serialized_response(&err, "rate_limited", "Rate limited");
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

        let serialized =
            assert_serialized_response(&error, "window_control_error", "Window control error");
        let serialized = serialized.to_string();

        assert!(!serialized.contains("Failed to set fullscreen"));
        assert!(!serialized.contains("main window"));
        assert!(!serialized.contains("platform API"));
        assert!(!serialized.contains("access denied"));
    }

    #[test]
    fn test_non_target_error_serialization_uses_explicit_safe_responses() {
        let unauthorized = PomoliceError::Unauthorized("settings".to_string());
        assert_serialized_response(&unauthorized, "unauthorized_window", "Unauthorized window");

        let invalid_transition = PomoliceError::InvalidStateTransition {
            action: "skip protected break".to_string(),
            current_state: TimerStatus::OnBreak,
        };
        assert_serialized_response(
            &invalid_transition,
            "invalid_state_transition",
            "Invalid state transition",
        );

        let rate_limited = PomoliceError::RateLimited {
            remaining_ms: 1_500,
        };
        assert_serialized_response(&rate_limited, "rate_limited", "Rate limited");

        let error = PomoliceError::from(SaveError::Commit {
            path: "C:\\Users\\nak\\AppData\\Roaming\\Pomolice\\timer_state.json".into(),
            source: std::io::Error::other("disk full"),
        });
        assert_serialized_response(&error, "persistence_error", "Persistence error");

        let runtime = tokio::runtime::Runtime::new().expect("test runtime should be created");
        let join_error = runtime.block_on(async {
            tokio::spawn(async move {
                panic!("simulated persistence worker panic");
            })
            .await
            .expect_err("spawned panic should surface as join error")
        });
        let join_error = PomoliceError::PersistenceTaskJoin { source: join_error };
        assert_serialized_response(&join_error, "persistence_error", "Persistence error");

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
    fn test_internal_error_display_keeps_diagnostic_details() {
        let window_control = PomoliceError::WindowControl("Failed to enter fullscreen".to_string());
        assert_eq!(
            window_control.to_string(),
            "Window control error: Failed to enter fullscreen"
        );

        let invalid_transition = PomoliceError::InvalidStateTransition {
            action: "start work".to_string(),
            current_state: TimerStatus::Working,
        };
        let invalid_transition = invalid_transition.to_string();
        assert!(invalid_transition.contains("start work"));
        assert!(invalid_transition.contains("Working"));

        let rate_limited = PomoliceError::RateLimited { remaining_ms: 500 };
        assert_eq!(rate_limited.to_string(), "Rate limited: please wait 500ms");

        let error = PomoliceError::from(SaveError::Write {
            path: "timer_state.json".into(),
            source: std::io::Error::other("permission denied"),
        });
        assert!(error.to_string().contains("timer_state.json"));
        assert!(error.to_string().contains("permission denied"));
    }

    #[test]
    fn test_persistence_save_serialization_redacts_storage_path_details() {
        let error = PomoliceError::from(SaveError::Commit {
            path: "C:\\Users\\nak\\AppData\\Roaming\\Pomolice\\timer_state.json".into(),
            source: std::io::Error::other("disk full"),
        });

        let serialized =
            assert_serialized_response(&error, "persistence_error", "Persistence error")
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
            assert_serialized_response(&error, "persistence_error", "Persistence error")
                .to_string();

        assert!(!serialized.contains("simulated persistence worker panic"));
    }
}
