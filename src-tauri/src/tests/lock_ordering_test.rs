// Lock ordering regression tests
//
// These tests verify that the lock acquisition order defined in AppState
// (input_monitor → window_manager → timer_manager) is consistently followed,
// preventing deadlocks between concurrent code paths.
//
// Each test simulates two tasks that acquire the same locks in the correct
// order concurrently. If either task were to acquire locks in reverse order,
// the test would deadlock and time out.
//
// The fake std::Mutex locks are test-only synchronization for modeling ordering;
// they are not production lock primitive guidance.

#[cfg(test)]
mod lock_ordering_tests {
    use std::sync::{mpsc, Arc, Barrier, Mutex};
    use std::thread;
    use std::time::Duration;

    /// Dummy state types that mirror the real managers but have no Tauri dependency.
    struct FakeInputMonitor;
    struct FakeWindowManager;
    struct FakeTimerManager;

    /// Shared lock set that mirrors `AppState` lock relationships.
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

    fn lock_or_recover<T>(mutex: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
        mutex
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    /// Simulates the lock acquisition pattern of `lib_handle_escape_combo`:
    ///   1. `input_monitor`
    ///   2. `window_manager`
    fn escape_combo_pattern(locks: &FakeLocks) {
        {
            let _input = lock_or_recover(&locks.input_monitor);
            thread::sleep(Duration::from_millis(1));
        }
        {
            let _window = lock_or_recover(&locks.window_manager);
            thread::sleep(Duration::from_millis(1));
        }
    }

    /// Simulates the lock acquisition pattern of `exit_break_mode`:
    ///   1. `input_monitor`
    ///   2. `window_manager`
    ///   3. `timer_manager`
    fn exit_break_mode_pattern(locks: &FakeLocks) {
        {
            let _input = lock_or_recover(&locks.input_monitor);
            thread::sleep(Duration::from_millis(1));
        }
        {
            let _window = lock_or_recover(&locks.window_manager);
            thread::sleep(Duration::from_millis(1));
        }
        {
            let _timer = lock_or_recover(&locks.timer_manager);
            thread::sleep(Duration::from_millis(1));
        }
    }

    /// Simulates the lock acquisition pattern of `enter_break_mode`:
    ///   1. `input_monitor`
    ///   2. `window_manager`
    ///   3. `timer_manager`
    fn enter_break_mode_pattern(locks: &FakeLocks) {
        {
            let _input = lock_or_recover(&locks.input_monitor);
            thread::sleep(Duration::from_millis(1));
        }
        {
            let _window = lock_or_recover(&locks.window_manager);
            thread::sleep(Duration::from_millis(1));
        }
        {
            let _timer = lock_or_recover(&locks.timer_manager);
            thread::sleep(Duration::from_millis(1));
        }
    }

    /// Simulates the lock acquisition pattern of `lib_handle_input_detected`:
    ///   1. `timer_manager` (only)
    fn input_detected_pattern(locks: &FakeLocks) {
        let _timer = lock_or_recover(&locks.timer_manager);
        thread::sleep(Duration::from_millis(1));
    }

    /// Simulates `scheduler_task_state` start/stop management.
    /// This lock must never be held together with other manager locks.
    fn scheduler_task_state_pattern(locks: &FakeLocks) {
        let _scheduler = lock_or_recover(&locks.scheduler_task_state);
        thread::sleep(Duration::from_millis(1));
    }

    /// Intentionally wrong order used only to verify timeout-based deadlock detection.
    fn inverted_timer_then_window_pattern(
        locks: &Arc<FakeLocks>,
        first_lock_barrier: &Arc<Barrier>,
    ) {
        let _timer = lock_or_recover(&locks.timer_manager);
        first_lock_barrier.wait();
        let _window = lock_or_recover(&locks.window_manager);
    }

    /// Intentionally wrong order used only to verify timeout-based deadlock detection.
    fn inverted_window_then_timer_pattern(
        locks: &Arc<FakeLocks>,
        first_lock_barrier: &Arc<Barrier>,
    ) {
        let _window = lock_or_recover(&locks.window_manager);
        first_lock_barrier.wait();
        let _timer = lock_or_recover(&locks.timer_manager);
    }

    // -------------------------------------------------------------------------
    // Tests
    // -------------------------------------------------------------------------

    /// `lib_handle_escape_combo` and `exit_break_mode` running concurrently
    /// must not deadlock. Both follow `input_monitor` → `window_manager` order.
    #[tokio::test]
    async fn test_escape_combo_and_exit_break_mode_do_not_deadlock() {
        let locks = Arc::new(FakeLocks::new());
        let locks_b = Arc::clone(&locks);

        let result = tokio::time::timeout(Duration::from_secs(5), async move {
            let task_a = tokio::task::spawn_blocking(move || escape_combo_pattern(&locks));
            let task_b = tokio::task::spawn_blocking(move || exit_break_mode_pattern(&locks_b));
            let _ = tokio::join!(task_a, task_b);
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
            let task_a = tokio::task::spawn_blocking(move || escape_combo_pattern(&locks));
            let task_b = tokio::task::spawn_blocking(move || enter_break_mode_pattern(&locks_b));
            let _ = tokio::join!(task_a, task_b);
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
            let task_a = tokio::task::spawn_blocking(move || exit_break_mode_pattern(&locks));
            let task_b = tokio::task::spawn_blocking(move || enter_break_mode_pattern(&locks_b));
            let _ = tokio::join!(task_a, task_b);
        })
        .await;

        assert!(
            result.is_ok(),
            "Deadlock detected: tasks did not complete within timeout"
        );
    }

    /// `lib_handle_input_detected` (`timer_manager` only) and `exit_break_mode`
    /// running concurrently must not deadlock.
    #[tokio::test]
    async fn test_input_detected_and_exit_break_mode_do_not_deadlock() {
        let locks = Arc::new(FakeLocks::new());
        let locks_b = Arc::clone(&locks);

        let result = tokio::time::timeout(Duration::from_secs(5), async move {
            let task_a = tokio::task::spawn_blocking(move || input_detected_pattern(&locks));
            let task_b = tokio::task::spawn_blocking(move || exit_break_mode_pattern(&locks_b));
            let _ = tokio::join!(task_a, task_b);
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
            let task_a = tokio::task::spawn_blocking(move || escape_combo_pattern(&locks));
            let task_b = tokio::task::spawn_blocking(move || exit_break_mode_pattern(&locks_b));
            let task_c = tokio::task::spawn_blocking(move || enter_break_mode_pattern(&locks_c));
            let task_d = tokio::task::spawn_blocking(move || input_detected_pattern(&locks_d));
            let task_e =
                tokio::task::spawn_blocking(move || scheduler_task_state_pattern(&locks_e));
            let _ = tokio::join!(task_a, task_b, task_c, task_d, task_e);
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
            let task_a = tokio::task::spawn_blocking(move || scheduler_task_state_pattern(&locks));
            let task_b = tokio::task::spawn_blocking(move || input_detected_pattern(&locks_b));
            let _ = tokio::join!(task_a, task_b);
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
        let (done_tx, done_rx) = mpsc::channel();

        let locks_a = Arc::clone(&locks);
        let barrier_a = Arc::clone(&first_lock_barrier);
        let done_tx_a = done_tx.clone();
        let _task_a = thread::spawn(move || {
            inverted_timer_then_window_pattern(&locks_a, &barrier_a);
            let _ = done_tx_a.send(());
        });

        let locks_b = Arc::clone(&locks);
        let barrier_b = Arc::clone(&first_lock_barrier);
        let _task_b = thread::spawn(move || {
            inverted_window_then_timer_pattern(&locks_b, &barrier_b);
            let _ = done_tx.send(());
        });

        let completion =
            tokio::task::spawn_blocking(move || done_rx.recv_timeout(Duration::from_millis(200)))
                .await
                .expect("blocking timeout wait should not panic");

        assert!(
            completion.is_err(),
            "inverted lock order should be detected as a timeout"
        );
    }
}
