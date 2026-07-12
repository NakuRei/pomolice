// Pomolice - Rust Test Module
// Phase 0: Basic test infrastructure
// Phase 4: Input monitor tests

#[cfg(test)]
mod command_input_rejection_test;

#[cfg(test)]
mod backend_behavior_regression_test;

#[cfg(test)]
pub(crate) mod join_error;

#[cfg(test)]
mod scheduler_concurrency_test;

#[cfg(test)]
mod startup_restore_test;

#[cfg(test)]
mod log_redaction_test;

#[cfg(test)]
mod logging_config_test;

#[cfg(test)]
mod logging_event_test;
