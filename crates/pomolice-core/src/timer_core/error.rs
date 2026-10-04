use super::phase::TimerStatus;

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum TimerError {
    #[error("Invalid state transition: cannot {action} from {current_state:?}")]
    InvalidStateTransition {
        action: String,
        current_state: TimerStatus,
    },
    #[error("Rate limited: please wait {remaining_ms}ms")]
    RateLimited { remaining_ms: u64 },
}

pub(super) type Result<T> = std::result::Result<T, TimerError>;
