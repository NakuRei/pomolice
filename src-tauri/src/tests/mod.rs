// Pomolice - Rust Test Module
// Phase 0: Basic test infrastructure
// Phase 4: Input monitor tests
// Phase 5: Lock ordering regression tests

#[cfg(test)]
mod input_monitor_test;

#[cfg(test)]
mod lock_ordering_test;

#[cfg(test)]
mod scheduler_concurrency_test;

#[cfg(test)]
mod log_redaction_test;

#[cfg(test)]
mod logging_config_test;

#[cfg(test)]
mod logging_event_test;

#[cfg(test)]
mod log_rotation_test;

#[cfg(test)]
mod basic_tests {
    #[test]
    fn test_basic_assertion() {
        // Basic smoke test to verify test infrastructure works
        assert_eq!(2 + 2, 4);
    }

    #[test]
    fn test_string_operations() {
        let app_name = "Pomolice";
        assert!(!app_name.is_empty());
        assert_eq!(app_name, "Pomolice");
    }
}

#[cfg(test)]
mod scheduler_boundary_runtime_tests {
    use std::collections::HashSet;

    use chrono::{Local, TimeZone};

    use crate::types::BoundaryOccurrenceId;
    use crate::{extract_boundary_epoch_seconds, trim_old_processed_boundaries};

    fn boundary_id(
        slot_key: &str,
        boundary_type: &str,
        epoch_seconds: i64,
    ) -> BoundaryOccurrenceId {
        BoundaryOccurrenceId(format!("{slot_key}|{boundary_type}|{epoch_seconds}"))
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
    fn test_extract_boundary_epoch_seconds_parses_expected_suffix() {
        let valid = boundary_id("mon", "end", 1_736_154_400);
        let invalid = BoundaryOccurrenceId("invalid-boundary-id".to_string());

        assert_eq!(extract_boundary_epoch_seconds(&valid), Some(1_736_154_400));
        assert_eq!(extract_boundary_epoch_seconds(&invalid), None);
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
        let invalid_boundary = BoundaryOccurrenceId("invalid".to_string());

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
    use crate::types::WeeklyAutoRunConfig;

    fn make_enabled_monday_config() -> WeeklyAutoRunConfig {
        let mut config = WeeklyAutoRunConfig::default();
        config.mon.enabled = true;
        config.mon.start_time = "09:00".to_string();
        config.mon.end_time = "18:00".to_string();
        config
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
        let mut next_config = previous_config.clone();
        next_config.mon.end_time = "19:00".to_string();

        let requires_revision =
            did_scheduler_topology_change(true, &previous_config, true, &next_config);

        assert!(
            requires_revision,
            "scheduler revision must change when weekly schedule is updated"
        );
    }
}
