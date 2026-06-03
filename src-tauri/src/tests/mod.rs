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
mod scheduler_boundary_runtime_tests {
    use std::collections::HashSet;

    use chrono::{Local, TimeZone};

    use crate::trim_old_processed_boundaries;
    use crate::types::BoundaryOccurrenceId;

    fn boundary_id(
        slot_key: &str,
        boundary_type: &str,
        epoch_seconds: i64,
    ) -> BoundaryOccurrenceId {
        BoundaryOccurrenceId::for_schedule_boundary(slot_key, boundary_type, epoch_seconds)
    }

    #[test]
    fn test_processed_boundary_set_rejects_duplicate_boundary_occurrence_ids() {
        let mut processed = HashSet::new();
        let boundary = boundary_id("mon", "start", 1_736_154_000);

        assert!(processed.insert(boundary.clone()));
        assert!(!processed.insert(boundary));
        assert_eq!(processed.len(), 1);
    }

    #[test]
    fn test_boundary_occurrence_id_exposes_epoch_seconds() {
        let valid = boundary_id("mon", "end", 1_736_154_400);
        let invalid = BoundaryOccurrenceId::from_raw_for_tests("invalid-boundary-id");

        assert_eq!(valid.epoch_seconds(), Some(1_736_154_400));
        assert_eq!(invalid.epoch_seconds(), None);
    }

    #[test]
    fn test_trim_old_processed_boundaries_keeps_recent_and_removes_stale_or_invalid() {
        let now_local = Local
            .with_ymd_and_hms(2025, 1, 10, 12, 0, 0)
            .single()
            .expect("local datetime should be valid");
        let stale_epoch = (now_local - chrono::Duration::days(10)).timestamp();
        let recent_epoch = (now_local - chrono::Duration::days(2)).timestamp();

        let stale_boundary = boundary_id("mon", "start", stale_epoch);
        let recent_boundary = boundary_id("tue", "end", recent_epoch);
        let invalid_boundary = BoundaryOccurrenceId::from_raw_for_tests("invalid");

        let mut processed = HashSet::from([
            stale_boundary.clone(),
            recent_boundary.clone(),
            invalid_boundary,
        ]);

        trim_old_processed_boundaries(&mut processed, now_local);

        assert!(processed.contains(&recent_boundary));
        assert!(!processed.contains(&stale_boundary));
        assert_eq!(processed.len(), 1);
    }
}

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
