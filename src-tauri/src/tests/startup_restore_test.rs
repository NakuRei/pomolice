#[cfg(test)]
mod startup_restore_tests {
    use std::future::Future;
    use std::path::{Path, PathBuf};
    use std::pin::Pin;
    use std::sync::Arc;
    use std::time::{SystemTime, UNIX_EPOCH};

    use chrono::{DateTime, Local, TimeZone, Utc};

    use crate::managers::clock::testing::FixedClock;
    use crate::managers::clock::Clock;
    use crate::managers::persistence::PersistenceManager;
    use crate::session_pipeline::{
        apply_startup_restore_with_effects, SessionPipelineRuntimeEffects,
    };
    use crate::state::AppState;
    use crate::types::{
        PostBreakTransitionPolicy, ScheduleStopState, SessionType, TimerSettings, TimerState,
        TimerStatus, TimerTickPayload,
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

    fn create_utc_datetime(
        year: i32,
        month: u32,
        day: u32,
        hour: u32,
        minute: u32,
        second: u32,
    ) -> DateTime<Utc> {
        Utc.with_ymd_and_hms(year, month, day, hour, minute, second)
            .single()
            .expect("valid UTC datetime")
    }

    fn fixed_clock(now_utc: DateTime<Utc>) -> Arc<FixedClock> {
        Arc::new(FixedClock::new(now_utc.with_timezone(&Local)))
    }

    fn as_clock_trait(clock: &Arc<FixedClock>) -> Arc<dyn Clock + Send + Sync> {
        Arc::<FixedClock>::clone(clock) as Arc<dyn Clock + Send + Sync>
    }

    fn create_temp_dir(test_name: &str) -> PathBuf {
        let timestamp_nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time should be after unix epoch")
            .as_nanos();

        let path = std::env::temp_dir().join(format!(
            "pomolice-startup-restore-{}-{}-{}",
            test_name,
            std::process::id(),
            timestamp_nanos
        ));
        std::fs::create_dir_all(&path).expect("temp directory should be created");
        path
    }

    fn cleanup_temp_dir(path: &Path) {
        let _ = std::fs::remove_dir_all(path);
    }

    fn save_state_to_dir(clock: &Arc<FixedClock>, dir: &Path, timer_state: TimerState) {
        let persistence = PersistenceManager::new(as_clock_trait(clock), Some(dir.to_path_buf()));
        persistence
            .save_state_sync(timer_state)
            .expect("sync save should succeed");
    }

    async fn run_startup_restore_from_dir(
        clock: Arc<FixedClock>,
        dir: &Path,
    ) -> (AppState, RecordingStartupRestoreRuntimeEffects) {
        let persistence = PersistenceManager::new(as_clock_trait(&clock), Some(dir.to_path_buf()));
        let (app_state, initialization_outcome) =
            AppState::build_startup_state_for_tests(as_clock_trait(&clock), persistence).await;
        let mut effects = RecordingStartupRestoreRuntimeEffects::default();

        apply_startup_restore_with_effects(&app_state, initialization_outcome, &mut effects).await;

        (app_state, effects)
    }

    async fn current_timer_state(app_state: &AppState) -> TimerState {
        app_state.timer_manager.lock().await.get_state().clone()
    }

    fn assert_no_startup_effects(effects: &RecordingStartupRestoreRuntimeEffects) {
        assert!(effects.emitted_event_names.is_empty());
        assert_eq!(effects.tray_update_count, 0);
        assert_eq!(effects.break_enter_count, 0);
        assert_eq!(effects.break_exit_count, 0);
        assert_eq!(effects.persist_count, 0);
        assert_eq!(effects.restart_timer_task_count, 0);
    }

    fn assert_emitted_event_set(
        effects: &RecordingStartupRestoreRuntimeEffects,
        expected_event_names: &[&'static str],
    ) {
        assert_eq!(
            effects.emitted_event_names.len(),
            expected_event_names.len()
        );
        for expected_event_name in expected_event_names {
            assert!(effects.emitted_event_names.contains(expected_event_name));
        }
    }

    #[tokio::test]
    async fn test_startup_with_no_saved_state_stays_idle_without_runtime_effects() {
        let temp_dir = create_temp_dir("no-saved-state");
        let restore_clock = fixed_clock(create_utc_datetime(2025, 2, 1, 9, 0, 0));

        let (app_state, effects) =
            run_startup_restore_from_dir(Arc::clone(&restore_clock), &temp_dir).await;

        let timer_state = current_timer_state(&app_state).await;
        assert_eq!(timer_state.status, TimerStatus::Idle);
        assert_eq!(timer_state.session_type, SessionType::Work);
        assert_eq!(
            timer_state.remaining_seconds,
            timer_state.settings.work_duration_secs.get()
        );
        assert!(app_state.restored_running_saved_at_local().is_none());
        assert_no_startup_effects(&effects);

        cleanup_temp_dir(&temp_dir);
    }

    #[tokio::test]
    async fn test_startup_with_unusable_persisted_state_uses_default_idle_state() {
        let temp_dir = create_temp_dir("unusable-persisted-state");
        let restore_clock = fixed_clock(create_utc_datetime(2025, 2, 1, 9, 0, 0));

        std::fs::write(temp_dir.join("timer_state.json"), "{ invalid json")
            .expect("unusable state fixture should be writable");

        let (app_state, effects) =
            run_startup_restore_from_dir(Arc::clone(&restore_clock), &temp_dir).await;

        let timer_state = current_timer_state(&app_state).await;
        assert_eq!(timer_state.status, TimerStatus::Idle);
        assert_eq!(timer_state.session_type, SessionType::Work);
        assert_eq!(
            timer_state.remaining_seconds,
            timer_state.settings.work_duration_secs.get()
        );
        assert!(app_state.restored_running_saved_at_local().is_none());
        assert_no_startup_effects(&effects);

        cleanup_temp_dir(&temp_dir);
    }

    #[tokio::test]
    async fn test_startup_with_saved_idle_state_restores_without_runtime_effects() {
        let temp_dir = create_temp_dir("saved-idle-state");
        let saved_at_utc = create_utc_datetime(2025, 2, 1, 9, 0, 0);
        let restore_at_utc = create_utc_datetime(2025, 2, 1, 9, 2, 0);
        let save_clock = fixed_clock(saved_at_utc);
        let restore_clock = fixed_clock(restore_at_utc);
        let restored_work_duration_secs = 20 * 60;
        let restored_settings = TimerSettings::try_from_raw(
            restored_work_duration_secs,
            5 * 60,
            15 * 60,
            4,
            PostBreakTransitionPolicy::RequireManualContinue,
        )
        .expect("test timer settings should be valid");

        save_state_to_dir(
            &save_clock,
            &temp_dir,
            TimerState {
                remaining_seconds: restored_work_duration_secs,
                settings: restored_settings,
                ..TimerState::default()
            },
        );

        let (app_state, effects) =
            run_startup_restore_from_dir(Arc::clone(&restore_clock), &temp_dir).await;

        let full_state = app_state.timer_manager.lock().await.get_full_state();
        assert_eq!(full_state.status, TimerStatus::Idle);
        assert_eq!(full_state.session_type, SessionType::Work);
        assert_eq!(full_state.remaining_seconds, restored_work_duration_secs);
        assert_eq!(
            full_state.settings.work_duration_secs.get(),
            restored_work_duration_secs
        );
        assert!(app_state.restored_running_saved_at_local().is_none());
        assert_no_startup_effects(&effects);

        cleanup_temp_dir(&temp_dir);
    }

    #[tokio::test]
    async fn test_startup_with_running_work_restores_remaining_time_and_restarts_timer() {
        let temp_dir = create_temp_dir("running-work");
        let saved_at_utc = create_utc_datetime(2025, 2, 1, 9, 0, 0);
        let restore_at_utc = create_utc_datetime(2025, 2, 1, 9, 2, 0);
        let save_clock = fixed_clock(saved_at_utc);
        let restore_clock = fixed_clock(restore_at_utc);
        let saved_remaining_seconds = 15 * 60;
        let elapsed_seconds_before_restore = 2 * 60;

        save_state_to_dir(
            &save_clock,
            &temp_dir,
            TimerState {
                status: TimerStatus::Working,
                remaining_seconds: saved_remaining_seconds,
                ..TimerState::default()
            },
        );

        let (app_state, effects) =
            run_startup_restore_from_dir(Arc::clone(&restore_clock), &temp_dir).await;

        let timer_state = current_timer_state(&app_state).await;
        assert_eq!(timer_state.status, TimerStatus::Working);
        assert_eq!(timer_state.session_type, SessionType::Work);
        assert_eq!(
            timer_state.remaining_seconds,
            saved_remaining_seconds - elapsed_seconds_before_restore
        );
        assert_eq!(
            app_state.restored_running_saved_at_local(),
            Some(saved_at_utc.with_timezone(&Local))
        );
        assert_eq!(effects.emitted_event_names, ["timer_tick"]);
        assert_eq!(effects.tray_update_count, 1);
        assert_eq!(effects.break_enter_count, 0);
        assert_eq!(effects.break_exit_count, 0);
        assert_eq!(effects.persist_count, 0);
        assert_eq!(effects.restart_timer_task_count, 1);

        cleanup_temp_dir(&temp_dir);
    }

    #[tokio::test]
    async fn test_startup_with_expired_work_enters_break_persists_and_restarts_break_timer() {
        let temp_dir = create_temp_dir("expired-work");
        let saved_at_utc = create_utc_datetime(2025, 2, 1, 9, 0, 0);
        let restore_at_utc = create_utc_datetime(2025, 2, 1, 9, 2, 0);
        let save_clock = fixed_clock(saved_at_utc);
        let restore_clock = fixed_clock(restore_at_utc);

        save_state_to_dir(
            &save_clock,
            &temp_dir,
            TimerState {
                status: TimerStatus::Working,
                remaining_seconds: 60,
                current_cycle: 1,
                ..TimerState::default()
            },
        );

        let (app_state, effects) =
            run_startup_restore_from_dir(Arc::clone(&restore_clock), &temp_dir).await;

        let timer_state = current_timer_state(&app_state).await;
        assert_eq!(timer_state.status, TimerStatus::OnBreak);
        assert_eq!(timer_state.session_type, SessionType::ShortBreak);
        assert_eq!(
            timer_state.remaining_seconds,
            timer_state.settings.short_break_duration_secs.get()
        );
        assert_eq!(
            app_state.restored_running_saved_at_local(),
            Some(saved_at_utc.with_timezone(&Local))
        );
        assert_emitted_event_set(&effects, &["session_completed", "timer_tick"]);
        assert_eq!(effects.tray_update_count, 1);
        assert_eq!(effects.break_enter_count, 1);
        assert_eq!(effects.break_exit_count, 0);
        assert_eq!(effects.persist_count, 1);
        assert_eq!(effects.restart_timer_task_count, 1);

        cleanup_temp_dir(&temp_dir);
    }

    #[tokio::test]
    async fn test_startup_with_expired_scheduled_stop_returns_idle_without_restarting_timer() {
        let temp_dir = create_temp_dir("expired-scheduled-stop");
        let saved_at_utc = create_utc_datetime(2025, 2, 1, 9, 0, 0);
        let restore_at_utc = create_utc_datetime(2025, 2, 1, 9, 2, 0);
        let save_clock = fixed_clock(saved_at_utc);
        let restore_clock = fixed_clock(restore_at_utc);

        save_state_to_dir(
            &save_clock,
            &temp_dir,
            TimerState {
                status: TimerStatus::Working,
                remaining_seconds: 60,
                schedule_stop_state: ScheduleStopState::StopAtNextSessionBoundary,
                auto_run_feature_enabled: true,
                ..TimerState::default()
            },
        );

        let (app_state, effects) =
            run_startup_restore_from_dir(Arc::clone(&restore_clock), &temp_dir).await;

        let timer_state = current_timer_state(&app_state).await;
        assert_eq!(timer_state.status, TimerStatus::Idle);
        assert_eq!(timer_state.session_type, SessionType::Work);
        assert_eq!(
            timer_state.remaining_seconds,
            timer_state.settings.work_duration_secs.get()
        );
        assert!(app_state.restored_running_saved_at_local().is_none());
        assert_emitted_event_set(
            &effects,
            &["session_completed", "auto_run_stopped", "timer_tick"],
        );
        assert_eq!(effects.tray_update_count, 1);
        assert_eq!(effects.break_enter_count, 0);
        assert_eq!(effects.break_exit_count, 0);
        assert_eq!(effects.persist_count, 1);
        assert_eq!(effects.restart_timer_task_count, 0);

        cleanup_temp_dir(&temp_dir);
    }
}
