// Pomolice - Rust Test Module
// Phase 0: Basic test infrastructure
// Phase 4: Input monitor tests
// Phase 5: Lock ordering regression tests

#[cfg(test)]
mod input_monitor_test;

#[cfg(test)]
mod lock_ordering_test;

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
        assert!(app_name.len() > 0);
        assert_eq!(app_name, "Pomolice");
    }
}
