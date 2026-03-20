// Error types for Pomolice application
// Provides structured error handling across all modules

use thiserror::Error;

use crate::types::TimerStatus;

/// Main error type for the Pomolice application
#[derive(Error, Debug)]
pub enum PomoliceError {
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

    #[error("Input monitoring failed: {0}")]
    #[allow(dead_code)]
    InputMonitorFailed(String),

    #[error("Invalid auto-run schedule: {reason}")]
    InvalidAutoRunSchedule { reason: String },
}

/// Result type alias for Pomolice operations
pub type Result<T> = std::result::Result<T, PomoliceError>;

impl serde::Serialize for PomoliceError {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&self.to_string())
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
    fn test_result_type_ok() {
        let result = Ok::<i32, PomoliceError>(42);
        match result {
            Ok(value) => assert_eq!(value, 42),
            Err(error) => panic!("expected Ok(42), got error: {error}"),
        }
    }

    #[test]
    fn test_invalid_auto_run_schedule_error() {
        let err = PomoliceError::InvalidAutoRunSchedule {
            reason: "overlapping schedule windows between mon and tue".to_string(),
        };
        assert_eq!(
            err.to_string(),
            "Invalid auto-run schedule: overlapping schedule windows between mon and tue"
        );
    }
}
