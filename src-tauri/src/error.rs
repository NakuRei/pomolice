// Error types for Pomolice application
// Provides structured error handling across all modules

use thiserror::Error;

use crate::managers::persistence::SaveError;
use crate::types::TimerStatus;

const PERSISTENCE_SAVE_SERIALIZED_MESSAGE: &str = "Persistence error: failed to save timer state";

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

impl serde::Serialize for PomoliceError {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        match self {
            Self::PersistenceSave(_) | Self::PersistenceTaskJoin { .. } => {
                serializer.serialize_str(PERSISTENCE_SAVE_SERIALIZED_MESSAGE)
            }
            _ => serializer.serialize_str(&self.to_string()),
        }
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

    #[test]
    fn test_error_serialization() {
        let err = PomoliceError::RateLimited { remaining_ms: 300 };
        let serialized = serde_json::to_string(&err).unwrap();
        assert_eq!(serialized, "\"Rate limited: please wait 300ms\"");
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
    fn test_persistence_save_serialization_redacts_storage_path_details() {
        let error = PomoliceError::from(SaveError::Commit {
            path: "C:\\Users\\nak\\AppData\\Roaming\\Pomolice\\timer_state.json".into(),
            source: std::io::Error::other("disk full"),
        });

        let serialized = serde_json::to_string(&error).expect("persistence error should serialize");

        assert!(!serialized.contains("timer_state.json"));
        assert!(!serialized.contains("AppData"));
        assert!(!serialized.contains("disk full"));
    }

    #[test]
    fn test_persistence_save_serialization_keeps_frontend_prefix_contract() {
        let error = PomoliceError::from(SaveError::Write {
            path: "timer_state.json".into(),
            source: std::io::Error::other("permission denied"),
        });

        let serialized = serde_json::to_string(&error).expect("persistence error should serialize");
        let deserialized: String =
            serde_json::from_str(&serialized).expect("serialized error should deserialize");

        assert_eq!(deserialized, PERSISTENCE_SAVE_SERIALIZED_MESSAGE);
        assert!(deserialized.starts_with("Persistence error:"));
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

        let serialized = serde_json::to_string(&error).expect("join error should serialize");
        let deserialized: String =
            serde_json::from_str(&serialized).expect("serialized error should deserialize");

        assert_eq!(deserialized, PERSISTENCE_SAVE_SERIALIZED_MESSAGE);
    }

    #[test]
    fn test_result_type_ok() {
        let result = Ok::<i32, PomoliceError>(42);
        match result {
            Ok(value) => assert_eq!(value, 42),
            Err(error) => panic!("expected Ok(42), got error: {error}"),
        }
    }
}
