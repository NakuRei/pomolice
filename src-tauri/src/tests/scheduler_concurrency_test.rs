// Scheduler and start/stop concurrency regression tests
//
// These tests focus on race windows introduced by weekly scheduler behavior.
// They use TimerStateManager directly with shared async Mutex to model
// command/scheduler interleaving without requiring a live Tauri AppHandle.

#[cfg(test)]
mod scheduler_concurrency_tests {
    use std::sync::Arc;
    use std::time::Duration;

    use tokio::sync::{Barrier, Mutex};

    use crate::managers::timer::TimerStateManager;
    use crate::types::{
        PostBreakDecisionAction, PostBreakTransitionPolicy, SessionTransitionReason, SessionType,
        TimerSettings, TimerStartSource, TimerStatus,
    };

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

    async fn prepare_break_session_with_auto_continue(
        timer_manager: &Arc<Mutex<TimerStateManager>>,
    ) {
        let mut timer = timer_manager.lock().await;
        let settings = TimerSettings {
            post_break_transition_policy: PostBreakTransitionPolicy::StartNextWorkAutomatically,
            ..TimerSettings::default()
        };
        timer
            .start_work_with_source(settings, TimerStartSource::ManualInsideWindow)
            .expect("initial work session should start");
        let _ = timer.transition_to_break();
    }

    async fn prepare_manual_decision_pending_state(timer_manager: &Arc<Mutex<TimerStateManager>>) {
        let mut timer = timer_manager.lock().await;
        let settings = TimerSettings {
            post_break_transition_policy: PostBreakTransitionPolicy::RequireManualContinue,
            ..TimerSettings::default()
        };
        timer
            .start_work_with_source(settings, TimerStartSource::ManualOutsideWindow)
            .expect("initial work session should start");
        let _ = timer.transition_to_break();
        let outcome = timer.complete_break_session_transition();
        assert_eq!(
            outcome.transition_reason,
            SessionTransitionReason::BreakWaitingDecision
        );
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
            let _ = timer.sync_automation_policies(
                true,
                crate::types::WeeklyAutoRunConfig::default(),
                crate::types::PostBreakTransitionPolicy::RequireManualContinue,
            );
        }

        let start_barrier = Arc::new(Barrier::new(3));

        let pending_timer_manager = Arc::clone(&timer_manager);
        let pending_barrier = Arc::clone(&start_barrier);
        let pending_task = tokio::spawn(async move {
            pending_barrier.wait().await;
            for _ in 0..100 {
                {
                    let mut timer = pending_timer_manager.lock().await;
                    let _ = timer.mark_scheduled_stop_pending_if_session_is_eligible();
                }
                tokio::task::yield_now().await;
            }
        });

        let completion_timer_manager = Arc::clone(&timer_manager);
        let completion_barrier = Arc::clone(&start_barrier);
        let completion_task = tokio::spawn(async move {
            completion_barrier.wait().await;
            for _ in 0..100 {
                {
                    let mut timer = completion_timer_manager.lock().await;
                    if timer.status() == TimerStatus::Working
                        && timer.get_state().scheduled_stop_state
                            == crate::types::ScheduledStopState::Pending
                    {
                        let _ = timer.complete_work_session_transition();
                    }
                }
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

    #[tokio::test]
    async fn test_break_auto_start_and_scheduler_start_collision_has_single_winner() {
        let timer_manager = Arc::new(Mutex::new(TimerStateManager::new(None)));
        prepare_break_session_with_auto_continue(&timer_manager).await;
        let start_barrier = Arc::new(Barrier::new(3));

        let break_completion_timer_manager = Arc::clone(&timer_manager);
        let break_completion_barrier = Arc::clone(&start_barrier);
        let break_completion_task = tokio::spawn(async move {
            break_completion_barrier.wait().await;
            let mut timer = break_completion_timer_manager.lock().await;
            timer.complete_break_session_transition().transition_reason
        });

        let scheduler_start_timer_manager = Arc::clone(&timer_manager);
        let scheduler_start_barrier = Arc::clone(&start_barrier);
        let scheduler_start_task = tokio::spawn(async move {
            scheduler_start_barrier.wait().await;
            let mut timer = scheduler_start_timer_manager.lock().await;
            timer
                .start_work_with_source(
                    TimerSettings::default(),
                    TimerStartSource::ScheduledAutoStart,
                )
                .is_ok()
        });

        start_barrier.wait().await;
        let (break_completion_result, scheduler_start_result) =
            tokio::join!(break_completion_task, scheduler_start_task);
        let break_completion_reason =
            break_completion_result.expect("break completion task should finish");
        let scheduler_start_succeeded =
            scheduler_start_result.expect("scheduler start task should finish");

        assert_eq!(
            break_completion_reason,
            SessionTransitionReason::BreakAutoStarted
        );
        assert!(
            !scheduler_start_succeeded,
            "scheduler start must lose once break auto-start commits the non-idle state"
        );

        let timer = timer_manager.lock().await;
        let state = timer.get_state();
        assert_eq!(timer.status(), TimerStatus::Working);
        assert_eq!(state.session_type, SessionType::Work);
        assert_eq!(state.current_cycle, 2);
        assert!(!state.post_break_decision_pending);
        assert_eq!(
            state.timer_start_source,
            Some(TimerStartSource::ManualInsideWindow)
        );
    }

    #[tokio::test]
    async fn test_manual_decision_continue_and_scheduler_start_collision_has_single_winner() {
        let timer_manager = Arc::new(Mutex::new(TimerStateManager::new(None)));
        prepare_manual_decision_pending_state(&timer_manager).await;
        let start_barrier = Arc::new(Barrier::new(3));

        let manual_decision_timer_manager = Arc::clone(&timer_manager);
        let manual_decision_barrier = Arc::clone(&start_barrier);
        let manual_decision_task = tokio::spawn(async move {
            manual_decision_barrier.wait().await;
            let mut timer = manual_decision_timer_manager.lock().await;
            timer
                .resolve_post_break_decision(PostBreakDecisionAction::Continue)
                .is_ok()
        });

        let scheduler_start_timer_manager = Arc::clone(&timer_manager);
        let scheduler_start_barrier = Arc::clone(&start_barrier);
        let scheduler_start_task = tokio::spawn(async move {
            scheduler_start_barrier.wait().await;
            let mut timer = scheduler_start_timer_manager.lock().await;
            timer
                .start_work_with_source(
                    TimerSettings::default(),
                    TimerStartSource::ScheduledAutoStart,
                )
                .is_ok()
        });

        start_barrier.wait().await;
        let (manual_decision_result, scheduler_start_result) =
            tokio::join!(manual_decision_task, scheduler_start_task);
        let manual_decision_succeeded =
            manual_decision_result.expect("manual decision task should finish");
        let scheduler_start_succeeded =
            scheduler_start_result.expect("scheduler start task should finish");

        assert_ne!(
            manual_decision_succeeded, scheduler_start_succeeded,
            "manual continue and scheduler start collision must have exactly one winner"
        );

        let timer = timer_manager.lock().await;
        let state = timer.get_state();
        assert_eq!(timer.status(), TimerStatus::Working);
        assert_eq!(state.session_type, SessionType::Work);
        assert!(!state.post_break_decision_pending);

        if manual_decision_succeeded {
            assert_eq!(state.current_cycle, 2);
            assert_eq!(state.timer_start_source, None);
        } else {
            assert_eq!(state.current_cycle, 1);
            assert_eq!(
                state.timer_start_source,
                Some(TimerStartSource::ScheduledAutoStart)
            );
        }
    }
}
