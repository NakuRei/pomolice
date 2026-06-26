// Pomolice - Rust Test Module
// Phase 0: Basic test infrastructure
// Phase 4: Input monitor tests

#[cfg(test)]
mod command_input_rejection_test;

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

#[cfg(test)]
mod scheduler_config_revision_scope_tests {
    use crate::commands::timer::did_scheduler_topology_change;
    use crate::types::{DayAutoRunSlot, HhMm, WeeklyAutoRunConfig};

    fn enabled_slot(start_time: &str, end_time: &str) -> DayAutoRunSlot {
        DayAutoRunSlot::try_new(
            true,
            HhMm::try_from(start_time.to_string()).expect("start time should be valid"),
            HhMm::try_from(end_time.to_string()).expect("end time should be valid"),
        )
        .expect("slot should be valid")
    }

    fn make_enabled_monday_config() -> WeeklyAutoRunConfig {
        WeeklyAutoRunConfig::try_new(
            enabled_slot("09:00", "18:00"),
            DayAutoRunSlot::default(),
            DayAutoRunSlot::default(),
            DayAutoRunSlot::default(),
            DayAutoRunSlot::default(),
            DayAutoRunSlot::default(),
            DayAutoRunSlot::default(),
        )
        .expect("weekly config should be valid")
    }

    #[test]
    fn test_post_break_policy_only_update_does_not_require_scheduler_revision() {
        let previous_config = make_enabled_monday_config();
        let next_config = previous_config.clone();

        let requires_revision =
            did_scheduler_topology_change(true, &previous_config, true, &next_config);

        assert!(
            !requires_revision,
            "scheduler revision must not change when schedule fields are unchanged"
        );
    }

    #[test]
    fn test_schedule_update_requires_scheduler_revision() {
        let previous_config = make_enabled_monday_config();
        let next_config = WeeklyAutoRunConfig::try_new(
            enabled_slot("09:00", "19:00"),
            DayAutoRunSlot::default(),
            DayAutoRunSlot::default(),
            DayAutoRunSlot::default(),
            DayAutoRunSlot::default(),
            DayAutoRunSlot::default(),
            DayAutoRunSlot::default(),
        )
        .expect("weekly config should be valid");

        let requires_revision =
            did_scheduler_topology_change(true, &previous_config, true, &next_config);

        assert!(
            requires_revision,
            "scheduler revision must change when weekly schedule is updated"
        );
    }
}
