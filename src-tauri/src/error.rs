// Error types for Pomolice application
// Provides structured error handling across all modules

use thiserror::Error;

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

    #[error("Persistence save error: {0}")]
    PersistenceSave(#[source] PersistenceFailure),

    #[error("failed to join persistence worker task: {source}")]
    PersistenceTaskJoin {
        #[source]
        source: tokio::task::JoinError,
    },
}

/// Result type alias for Pomolice operations
pub(crate) type Result<T> = std::result::Result<T, PomoliceError>;

#[derive(Debug)]
pub(crate) struct PersistenceFailure {
    detail: String,
    source: Box<dyn std::error::Error + Send + Sync + 'static>,
}

impl PersistenceFailure {
    pub(crate) fn from_error(source: impl std::error::Error + Send + Sync + 'static) -> Self {
        Self {
            detail: source.to_string(),
            source: Box::new(source),
        }
    }
}

impl std::fmt::Display for PersistenceFailure {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.detail)
    }
}

impl std::error::Error for PersistenceFailure {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&*self.source)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;

    use crate::shared_tests::join_error::join_error_from_panic;
    use strum::IntoEnumIterator;

    fn assert_all_pomolice_error_variants_are_represented(
        occurrences: &[(PomoliceError, &'static [&'static str])],
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

    fn persistence_failure(detail: &'static str) -> PersistenceFailure {
        PersistenceFailure::from_error(std::io::Error::other(detail))
    }

    fn error_occurrences_with_display_details() -> Vec<(PomoliceError, &'static [&'static str])> {
        vec![
            (
                PomoliceError::Unauthorized("test-window".to_string()),
                &["test-window"],
            ),
            (
                PomoliceError::InvalidStateTransition {
                    action: "start work".to_string(),
                    current_state: TimerStatus::Working,
                },
                &["Invalid state transition", "start work", "Working"],
            ),
            (PomoliceError::RateLimited { remaining_ms: 500 }, &["500"]),
            (
                PomoliceError::WindowControl("Failed to enter fullscreen".to_string()),
                &["Failed to enter fullscreen"],
            ),
            (
                PomoliceError::PersistenceSave(persistence_failure(
                    "failed to write timer_state.json: disk full",
                )),
                &["timer_state.json", "disk full"],
            ),
            (
                PomoliceError::PersistenceTaskJoin {
                    source: join_error_from_panic("simulated persistence worker panic"),
                },
                &["simulated persistence worker panic"],
            ),
        ]
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
    fn display_messages_keep_diagnostic_details() {
        let occurrences = error_occurrences_with_display_details();

        assert_all_pomolice_error_variants_are_represented(&occurrences);

        for (error, expected_details) in occurrences {
            assert_display_includes_diagnostic_details(&error, expected_details);
        }
    }

    #[test]
    fn persistence_failure_display_keeps_diagnostic_detail() {
        let failure = PersistenceFailure::from_error(std::io::Error::other(
            "failed to write timer_state.json: disk full",
        ));
        let message = failure.to_string();

        assert!(message.contains("timer_state.json"));
        assert!(message.contains("disk full"));
    }

    #[test]
    fn persistence_save_display_keeps_diagnostic_detail() {
        let error = PomoliceError::PersistenceSave(PersistenceFailure::from_error(
            std::io::Error::other("failed to write timer_state.json: disk full"),
        ));
        let message = error.to_string();

        assert!(message.contains("Persistence save error"));
        assert!(message.contains("timer_state.json"));
        assert!(message.contains("disk full"));
    }
}
