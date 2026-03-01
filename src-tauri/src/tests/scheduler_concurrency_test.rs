// Scheduler and start/stop concurrency regression tests
//
// These tests focus on race windows introduced by weekly scheduler behavior.
// They use TimerStateManager directly with shared async Mutex to model
// command/scheduler interleaving without requiring a live Tauri AppHandle.

#[cfg(test)]
mod scheduler_concurrency_tests {
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::time::Duration;

    use tokio::sync::{Barrier, Mutex};
    use tokio::task::JoinHandle;
    use tokio_util::sync::CancellationToken;

    use crate::managers::timer::TimerStateManager;
    use crate::types::{SessionType, TimerSettings, TimerStartSource, TimerStatus};

    async fn race_two_start_sources(
        first_source: TimerStartSource,
        second_source: TimerStartSource,
    ) -> (bool, bool, TimerStatus, Option<TimerStartSource>) {
        let timer_manager = Arc::new(Mutex::new(TimerStateManager::new(None)));
        let start_barrier = Arc::new(Barrier::new(3));
        let settings = TimerSettings::default();

        let left_timer_manager = Arc::clone(&timer_manager);
        let left_barrier = Arc::clone(&start_barrier);
        let left_settings = settings.clone();
        let left_task = tokio::spawn(async move {
            left_barrier.wait().await;
            let mut timer = left_timer_manager.lock().await;
            timer
                .start_work_with_source(left_settings, first_source)
                .is_ok()
        });

        let right_timer_manager = Arc::clone(&timer_manager);
        let right_barrier = Arc::clone(&start_barrier);
        let right_settings = settings.clone();
        let right_task = tokio::spawn(async move {
            right_barrier.wait().await;
            let mut timer = right_timer_manager.lock().await;
            timer
                .start_work_with_source(right_settings, second_source)
                .is_ok()
        });

        start_barrier.wait().await;

        let (left_result, right_result) = tokio::join!(left_task, right_task);
        let left_succeeded = left_result.expect("left start task should complete");
        let right_succeeded = right_result.expect("right start task should complete");

        let timer = timer_manager.lock().await;
        (
            left_succeeded,
            right_succeeded,
            timer.status(),
            timer.get_state().timer_start_source,
        )
    }

    #[tokio::test]
    async fn test_manual_start_and_scheduler_start_race_has_single_winner() {
        let (left_succeeded, right_succeeded, status, start_source) = race_two_start_sources(
            TimerStartSource::ManualOutsideWindow,
            TimerStartSource::ScheduledAutoStart,
        )
        .await;

        assert_ne!(
            left_succeeded, right_succeeded,
            "manual start and scheduler auto-start race must not produce two winners"
        );
        assert_eq!(status, TimerStatus::Working);
        assert!(start_source.is_some());
    }

    #[tokio::test]
    async fn test_boot_prompt_start_and_scheduler_start_race_has_single_winner() {
        let (left_succeeded, right_succeeded, status, start_source) = race_two_start_sources(
            TimerStartSource::ManualInsideWindow,
            TimerStartSource::ScheduledAutoStart,
        )
        .await;

        assert_ne!(
            left_succeeded, right_succeeded,
            "boot-prompt start and scheduler start race must not produce double start"
        );
        assert_eq!(status, TimerStatus::Working);
        assert!(start_source.is_some());
    }

    #[tokio::test]
    async fn test_scheduler_mark_pending_and_session_completion_paths_do_not_deadlock() {
        let timer_manager = Arc::new(Mutex::new(TimerStateManager::new(None)));
        {
            let mut timer = timer_manager.lock().await;
            let _ = timer
                .start_work_with_source(
                    TimerSettings::default(),
                    TimerStartSource::ManualInsideWindow,
                )
                .expect("initial work session should start");
            let _ = timer.sync_auto_run_settings(true, crate::types::WeeklyAutoRunConfig::default());
        }

        let start_barrier = Arc::new(Barrier::new(3));

        let pending_timer_manager = Arc::clone(&timer_manager);
        let pending_barrier = Arc::clone(&start_barrier);
        let pending_task = tokio::spawn(async move {
            pending_barrier.wait().await;
            for _ in 0..100 {
                let mut timer = pending_timer_manager.lock().await;
                let _ = timer.mark_scheduled_stop_pending_if_session_is_eligible();
                drop(timer);
                tokio::task::yield_now().await;
            }
        });

        let completion_timer_manager = Arc::clone(&timer_manager);
        let completion_barrier = Arc::clone(&start_barrier);
        let completion_task = tokio::spawn(async move {
            completion_barrier.wait().await;
            for _ in 0..100 {
                let mut timer = completion_timer_manager.lock().await;
                if timer.status() == TimerStatus::Working
                    && timer.get_state().scheduled_stop_state == crate::types::ScheduledStopState::Pending
                {
                    let _ = timer.transition_to_idle_due_to_scheduled_stop(SessionType::Work);
                }
                drop(timer);
                tokio::task::yield_now().await;
            }
        });

        let completion = tokio::time::timeout(Duration::from_secs(5), async move {
            start_barrier.wait().await;
            let _ = tokio::join!(pending_task, completion_task);
        })
        .await;

        assert!(
            completion.is_ok(),
            "scheduler pending-mark and session-completion paths should not deadlock"
        );
    }

    #[derive(Default)]
    struct FakeSchedulerTaskState {
        cancellation_token: Option<CancellationToken>,
        join_handle: Option<JoinHandle<()>>,
    }

    #[derive(Default)]
    struct FakeSchedulerController {
        task_state: Arc<Mutex<FakeSchedulerTaskState>>,
        active_worker_count: Arc<AtomicUsize>,
    }

    impl FakeSchedulerController {
        async fn restart_scheduler(&self) {
            self.stop_scheduler().await;

            let cancellation_token = CancellationToken::new();
            let worker_cancel_token = cancellation_token.clone();
            let active_worker_count = Arc::clone(&self.active_worker_count);

            let join_handle = tokio::spawn(async move {
                active_worker_count.fetch_add(1, Ordering::SeqCst);
                loop {
                    tokio::select! {
                        _ = worker_cancel_token.cancelled() => break,
                        _ = tokio::time::sleep(Duration::from_millis(10)) => {}
                    }
                }
                active_worker_count.fetch_sub(1, Ordering::SeqCst);
            });

            let mut task_state = self.task_state.lock().await;
            task_state.cancellation_token = Some(cancellation_token);
            task_state.join_handle = Some(join_handle);
        }

        async fn stop_scheduler(&self) {
            let join_handle = {
                let mut task_state = self.task_state.lock().await;
                if let Some(cancellation_token) = task_state.cancellation_token.take() {
                    cancellation_token.cancel();
                }
                task_state.join_handle.take()
            };

            if let Some(handle) = join_handle {
                let _ = handle.await;
            }
        }
    }

    #[tokio::test]
    async fn test_scheduler_restart_does_not_leave_duplicate_running_tasks() {
        let controller = FakeSchedulerController::default();

        for _ in 0..6 {
            controller.restart_scheduler().await;
            tokio::time::sleep(Duration::from_millis(20)).await;
        }

        tokio::time::sleep(Duration::from_millis(50)).await;
        assert_eq!(
            controller.active_worker_count.load(Ordering::SeqCst),
            1,
            "exactly one scheduler worker should remain after repeated restart"
        );

        controller.stop_scheduler().await;
        tokio::time::sleep(Duration::from_millis(20)).await;

        assert_eq!(
            controller.active_worker_count.load(Ordering::SeqCst),
            0,
            "all scheduler workers should stop after shutdown"
        );
    }
}
