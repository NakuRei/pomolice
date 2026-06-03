#[cfg(test)]
mod startup_restore_tests {
    use std::future::Future;
    use std::pin::Pin;
    use std::sync::Arc;

    use chrono::{Local, TimeZone, Utc};

    use crate::managers::clock::testing::FixedClock;
    use crate::managers::clock::Clock;
    use crate::managers::persistence::PersistenceManager;
    use crate::session_pipeline::{
        apply_startup_restore_with_effects, SessionPipelineRuntimeEffects,
    };
    use crate::state::{
        AppInitializationOutcome, AppRestoreStatus, AppState, StartupRestoreAction,
    };
    use crate::types::{
        SessionTransitionOutcome, SessionType, TimerState, TimerStatus, TimerTickPayload,
    };

    #[derive(Default)]
    struct RecordingStartupRestoreRuntimeEffects {
        emitted_event_names: Vec<&'static str>,
        tray_update_count: usize,
        break_enter_count: usize,
        break_exit_count: usize,
        persist_count: usize,
        restart_timer_task_count: usize,
    }

    impl SessionPipelineRuntimeEffects for RecordingStartupRestoreRuntimeEffects {
        fn emit_event<T: serde::Serialize>(&mut self, event_name: &'static str, _payload: &T) {
            self.emitted_event_names.push(event_name);
        }

        fn update_tray(&mut self, _tick_payload: &TimerTickPayload) {
            self.tray_update_count += 1;
        }

        fn enter_break_mode<'a>(&'a mut self) -> Pin<Box<dyn Future<Output = ()> + Send + 'a>> {
            Box::pin(async move {
                self.break_enter_count += 1;
            })
        }

        fn exit_break_mode<'a>(&'a mut self) -> Pin<Box<dyn Future<Output = ()> + Send + 'a>> {
            Box::pin(async move {
                self.break_exit_count += 1;
            })
        }

        fn persist_timer_state<'a>(&'a mut self) -> Pin<Box<dyn Future<Output = ()> + Send + 'a>> {
            Box::pin(async move {
                self.persist_count += 1;
            })
        }

        fn restart_timer_task<'a>(&'a mut self) -> Pin<Box<dyn Future<Output = ()> + Send + 'a>> {
            Box::pin(async move {
                self.restart_timer_task_count += 1;
            })
        }
    }

    fn fixed_clock() -> Arc<FixedClock> {
        let now_utc = Utc
            .with_ymd_and_hms(2025, 2, 1, 9, 0, 0)
            .single()
            .expect("valid UTC datetime");
        Arc::new(FixedClock::new(now_utc.with_timezone(&Local)))
    }

    fn as_clock_trait(clock: &Arc<FixedClock>) -> Arc<dyn Clock + Send + Sync> {
        Arc::<FixedClock>::clone(clock) as Arc<dyn Clock + Send + Sync>
    }

    async fn build_test_state_without_persistence_path() -> AppState {
        let clock = fixed_clock();
        let persistence = PersistenceManager::new(as_clock_trait(&clock), None);
        let (app_state, _) =
            AppState::build_startup_state_for_tests(as_clock_trait(&clock), persistence).await;
        app_state
    }

    async fn replace_timer_state(app_state: &AppState, timer_state: TimerState) {
        let mut timer = app_state.timer_manager.lock().await;
        let _ = timer.apply_persisted_state(timer_state);
    }

    #[tokio::test]
    async fn test_startup_restored_running_timer_emits_tick_and_restarts_without_persisting() {
        let state = build_test_state_without_persistence_path().await;
        let restored_timer_state = TimerState {
            status: TimerStatus::Working,
            remaining_seconds: 780,
            ..TimerState::default()
        };
        let initialization_outcome = AppInitializationOutcome {
            initial_tick_payload: Some(restored_timer_state.to_tick_payload()),
            restore_status: AppRestoreStatus::Restored,
            startup_action: StartupRestoreAction::RestartRestoredTimerTask,
        };
        let mut effects = RecordingStartupRestoreRuntimeEffects::default();

        apply_startup_restore_with_effects(&state, initialization_outcome, &mut effects).await;

        assert_eq!(effects.emitted_event_names, ["timer_tick"]);
        assert_eq!(effects.tray_update_count, 1);
        assert_eq!(effects.break_enter_count, 0);
        assert_eq!(effects.break_exit_count, 0);
        assert_eq!(effects.persist_count, 0);
        assert_eq!(effects.restart_timer_task_count, 1);
    }

    #[tokio::test]
    async fn test_startup_expired_work_enters_break_persists_and_restarts_timer() {
        let state = build_test_state_without_persistence_path().await;
        replace_timer_state(
            &state,
            TimerState {
                status: TimerStatus::OnBreak,
                session_type: SessionType::ShortBreak,
                remaining_seconds: 300,
                ..TimerState::default()
            },
        )
        .await;
        let initialization_outcome = AppInitializationOutcome {
            initial_tick_payload: None,
            restore_status: AppRestoreStatus::Restored,
            startup_action: StartupRestoreAction::FinalizeExpiredRestoredWork(
                SessionTransitionOutcome::work_to_break(SessionType::ShortBreak),
            ),
        };
        let mut effects = RecordingStartupRestoreRuntimeEffects::default();

        apply_startup_restore_with_effects(&state, initialization_outcome, &mut effects).await;

        assert_eq!(
            effects.emitted_event_names,
            ["session_completed", "timer_tick"]
        );
        assert_eq!(effects.tray_update_count, 1);
        assert_eq!(effects.break_enter_count, 1);
        assert_eq!(effects.break_exit_count, 0);
        assert_eq!(effects.persist_count, 1);
        assert_eq!(effects.restart_timer_task_count, 1);
    }

    #[tokio::test]
    async fn test_startup_expired_work_scheduled_stop_emits_stop_without_break_or_restart() {
        let state = build_test_state_without_persistence_path().await;
        replace_timer_state(
            &state,
            TimerState {
                status: TimerStatus::Idle,
                session_type: SessionType::Work,
                remaining_seconds: 1500,
                ..TimerState::default()
            },
        )
        .await;
        let initialization_outcome = AppInitializationOutcome {
            initial_tick_payload: None,
            restore_status: AppRestoreStatus::Restored,
            startup_action: StartupRestoreAction::FinalizeExpiredRestoredWork(
                SessionTransitionOutcome::scheduled_stop_executed(SessionType::Work),
            ),
        };
        let mut effects = RecordingStartupRestoreRuntimeEffects::default();

        apply_startup_restore_with_effects(&state, initialization_outcome, &mut effects).await;

        assert_eq!(
            effects.emitted_event_names,
            ["session_completed", "auto_run_stopped", "timer_tick"]
        );
        assert_eq!(effects.tray_update_count, 1);
        assert_eq!(effects.break_enter_count, 0);
        assert_eq!(effects.break_exit_count, 0);
        assert_eq!(effects.persist_count, 1);
        assert_eq!(effects.restart_timer_task_count, 0);
    }
}
