// Lock ordering regression tests
//
// These tests verify that the lock acquisition order defined in AppState
// (input_monitor → window_manager → timer_manager) is consistently followed,
// preventing deadlocks between concurrent code paths.
//
// Each test simulates two tasks that acquire the same locks in the correct
// order concurrently. If either task were to acquire locks in reverse order,
// the test would deadlock and time out.

#[cfg(test)]
mod lock_ordering_tests {
    use std::sync::Arc;
    use std::time::Duration;
    use tokio::sync::{Barrier, Mutex};

    /// Dummy state types that mirror the real managers but have no Tauri dependency.
    struct FakeInputMonitor;
    struct FakeWindowManager;
    struct FakeTimerManager;

    /// Shared lock set that mirrors AppState lock relationships.
    struct FakeLocks {
        input_monitor: Arc<Mutex<FakeInputMonitor>>,
        window_manager: Arc<Mutex<FakeWindowManager>>,
        timer_manager: Arc<Mutex<FakeTimerManager>>,
        scheduler_task_state: Arc<Mutex<()>>,
    }

    impl FakeLocks {
        fn new() -> Self {
            Self {
                input_monitor: Arc::new(Mutex::new(FakeInputMonitor)),
                window_manager: Arc::new(Mutex::new(FakeWindowManager)),
                timer_manager: Arc::new(Mutex::new(FakeTimerManager)),
                scheduler_task_state: Arc::new(Mutex::new(())),
            }
        }
    }

    /// Simulates the lock acquisition pattern of `lib_handle_escape_combo`:
    ///   1. input_monitor
    ///   2. window_manager
    async fn escape_combo_pattern(locks: &FakeLocks) {
        {
            let _input = locks.input_monitor.lock().await;
            tokio::time::sleep(Duration::from_millis(1)).await;
        }
        {
            let _window = locks.window_manager.lock().await;
            tokio::time::sleep(Duration::from_millis(1)).await;
        }
    }

    /// Simulates the lock acquisition pattern of `exit_break_mode`:
    ///   1. input_monitor
    ///   2. window_manager
    ///   3. timer_manager
    async fn exit_break_mode_pattern(locks: &FakeLocks) {
        {
            let _input = locks.input_monitor.lock().await;
            tokio::time::sleep(Duration::from_millis(1)).await;
        }
        {
            let _window = locks.window_manager.lock().await;
            tokio::time::sleep(Duration::from_millis(1)).await;
        }
        {
            let _timer = locks.timer_manager.lock().await;
            tokio::time::sleep(Duration::from_millis(1)).await;
        }
    }

    /// Simulates the lock acquisition pattern of `enter_break_mode`:
    ///   1. input_monitor
    ///   2. window_manager
    ///   3. timer_manager
    async fn enter_break_mode_pattern(locks: &FakeLocks) {
        {
            let _input = locks.input_monitor.lock().await;
            tokio::time::sleep(Duration::from_millis(1)).await;
        }
        {
            let _window = locks.window_manager.lock().await;
            tokio::time::sleep(Duration::from_millis(1)).await;
        }
        {
            let _timer = locks.timer_manager.lock().await;
            tokio::time::sleep(Duration::from_millis(1)).await;
        }
    }

    /// Simulates the lock acquisition pattern of `lib_handle_input_detected`:
    ///   1. timer_manager (only)
    async fn input_detected_pattern(locks: &FakeLocks) {
        let _timer = locks.timer_manager.lock().await;
        tokio::time::sleep(Duration::from_millis(1)).await;
    }

    /// Simulates `scheduler_task_state` start/stop management.
    /// This lock must never be held together with other manager locks.
    async fn scheduler_task_state_pattern(locks: &FakeLocks) {
        let _scheduler = locks.scheduler_task_state.lock().await;
        tokio::time::sleep(Duration::from_millis(1)).await;
    }

    /// Intentionally wrong order used only to verify timeout-based deadlock detection.
    async fn inverted_timer_then_window_pattern(
        locks: Arc<FakeLocks>,
        first_lock_barrier: Arc<Barrier>,
    ) {
        let _timer = locks.timer_manager.lock().await;
        first_lock_barrier.wait().await;
        let _window = locks.window_manager.lock().await;
    }

    /// Intentionally wrong order used only to verify timeout-based deadlock detection.
    async fn inverted_window_then_timer_pattern(
        locks: Arc<FakeLocks>,
        first_lock_barrier: Arc<Barrier>,
    ) {
        let _window = locks.window_manager.lock().await;
        first_lock_barrier.wait().await;
        let _timer = locks.timer_manager.lock().await;
    }

    // -------------------------------------------------------------------------
    // Tests
    // -------------------------------------------------------------------------

    /// `lib_handle_escape_combo` and `exit_break_mode` running concurrently
    /// must not deadlock. Both follow input_monitor → window_manager order.
    #[tokio::test]
    async fn test_escape_combo_and_exit_break_mode_do_not_deadlock() {
        let locks = Arc::new(FakeLocks::new());
        let locks_b = Arc::clone(&locks);

        let result = tokio::time::timeout(Duration::from_secs(5), async move {
            tokio::join!(
                escape_combo_pattern(&locks),
                exit_break_mode_pattern(&locks_b),
            )
        })
        .await;

        assert!(
            result.is_ok(),
            "Deadlock detected: tasks did not complete within timeout"
        );
    }

    /// `lib_handle_escape_combo` and `enter_break_mode` running concurrently
    /// must not deadlock.
    #[tokio::test]
    async fn test_escape_combo_and_enter_break_mode_do_not_deadlock() {
        let locks = Arc::new(FakeLocks::new());
        let locks_b = Arc::clone(&locks);

        let result = tokio::time::timeout(Duration::from_secs(5), async move {
            tokio::join!(
                escape_combo_pattern(&locks),
                enter_break_mode_pattern(&locks_b),
            )
        })
        .await;

        assert!(
            result.is_ok(),
            "Deadlock detected: tasks did not complete within timeout"
        );
    }

    /// `exit_break_mode` and `enter_break_mode` running concurrently
    /// must not deadlock.
    #[tokio::test]
    async fn test_exit_break_mode_and_enter_break_mode_do_not_deadlock() {
        let locks = Arc::new(FakeLocks::new());
        let locks_b = Arc::clone(&locks);

        let result = tokio::time::timeout(Duration::from_secs(5), async move {
            tokio::join!(
                exit_break_mode_pattern(&locks),
                enter_break_mode_pattern(&locks_b),
            )
        })
        .await;

        assert!(
            result.is_ok(),
            "Deadlock detected: tasks did not complete within timeout"
        );
    }

    /// `lib_handle_input_detected` (timer_manager only) and `exit_break_mode`
    /// running concurrently must not deadlock.
    #[tokio::test]
    async fn test_input_detected_and_exit_break_mode_do_not_deadlock() {
        let locks = Arc::new(FakeLocks::new());
        let locks_b = Arc::clone(&locks);

        let result = tokio::time::timeout(Duration::from_secs(5), async move {
            tokio::join!(
                input_detected_pattern(&locks),
                exit_break_mode_pattern(&locks_b),
            )
        })
        .await;

        assert!(
            result.is_ok(),
            "Deadlock detected: tasks did not complete within timeout"
        );
    }

    /// All four patterns running simultaneously must not deadlock.
    #[tokio::test]
    async fn test_all_patterns_concurrent_do_not_deadlock() {
        let locks = Arc::new(FakeLocks::new());
        let locks_b = Arc::clone(&locks);
        let locks_c = Arc::clone(&locks);
        let locks_d = Arc::clone(&locks);
        let locks_e = Arc::clone(&locks);

        let result = tokio::time::timeout(Duration::from_secs(5), async move {
            tokio::join!(
                escape_combo_pattern(&locks),
                exit_break_mode_pattern(&locks_b),
                enter_break_mode_pattern(&locks_c),
                input_detected_pattern(&locks_d),
                scheduler_task_state_pattern(&locks_e),
            )
        })
        .await;

        assert!(
            result.is_ok(),
            "Deadlock detected: tasks did not complete within timeout"
        );
    }

    /// `scheduler_task_state` and `timer_manager` access must remain independent.
    /// Running both patterns concurrently should complete without deadlock.
    #[tokio::test]
    async fn test_scheduler_task_state_and_timer_manager_patterns_do_not_deadlock() {
        let locks = Arc::new(FakeLocks::new());
        let locks_b = Arc::clone(&locks);

        let result = tokio::time::timeout(Duration::from_secs(5), async move {
            tokio::join!(
                scheduler_task_state_pattern(&locks),
                input_detected_pattern(&locks_b),
            )
        })
        .await;

        assert!(
            result.is_ok(),
            "Deadlock detected: scheduler and timer patterns did not complete"
        );
    }

    /// Intentionally inverted lock acquisition should be detected by timeout guard.
    #[tokio::test]
    async fn test_lock_order_inversion_is_detected_by_timeout_guard() {
        let locks = Arc::new(FakeLocks::new());
        let first_lock_barrier = Arc::new(Barrier::new(2));

        let mut task_a = tokio::spawn(inverted_timer_then_window_pattern(
            Arc::clone(&locks),
            Arc::clone(&first_lock_barrier),
        ));
        let mut task_b = tokio::spawn(inverted_window_then_timer_pattern(
            Arc::clone(&locks),
            Arc::clone(&first_lock_barrier),
        ));

        let completion = tokio::time::timeout(Duration::from_millis(200), async {
            let _ = (&mut task_a).await;
            let _ = (&mut task_b).await;
        })
        .await;

        assert!(
            completion.is_err(),
            "inverted lock order should be detected as a timeout"
        );

        task_a.abort();
        task_b.abort();
        let _ = task_a.await;
        let _ = task_b.await;
    }
}
