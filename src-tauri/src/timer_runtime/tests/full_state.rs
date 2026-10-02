use super::*;

#[tokio::test]
async fn an_unusable_settings_file_is_reported_until_a_successful_save() {
    let clock = fixed_clock();
    let fail = Arc::new(std::sync::atomic::AtomicBool::new(true));
    let settings_persistence = PersistenceManager::new(Some(create_temp_dir("settings-status")))
        .with_injected_save_error_hook({
            let fail = Arc::clone(&fail);
            Arc::new(move || {
                fail.load(std::sync::atomic::Ordering::SeqCst)
                    .then_some(SaveError::PathUnavailable)
            })
        });
    let clock_trait: Arc<dyn Clock + Send + Sync> = Arc::<FixedClock>::clone(&clock);
    let runtime = TimerRuntime::new(
        TimerCore::restore(None, UserSettings::default(), clock.now_utc(), Local).0,
        clock_trait,
        PersistenceManager::new(Some(create_temp_dir("settings-status-timer"))),
        settings_persistence,
        SettingsFileStatus::Unusable,
    );
    let sink = RecordingSink::with_lock_outcome(EffectStatus::Succeeded);
    let before = runtime.read_full_state().await;
    assert_eq!(
        before.timer.settings_file_status,
        SettingsFileStatus::Unusable
    );
    let encoded = serde_json::to_value(&before).expect("snapshot serializes");
    assert_eq!(encoded["timer"]["status"], "idle");
    assert_eq!(encoded["timer"]["settings_file_status"], "unusable");
    assert_eq!(
        encoded["timer"]["settings"]["work_duration_secs"],
        UserSettings::default().work_duration_secs.get()
    );
    assert!(encoded["timer"].get("state").is_none());
    assert!(runtime
        .update_settings(&sink, one_second_work())
        .await
        .is_err());
    assert_eq!(runtime.read_full_state().await, before);
    assert!(sink.emitted_names().is_empty());

    fail.store(false, std::sync::atomic::Ordering::SeqCst);
    let saved = runtime
        .update_settings(&sink, one_second_work())
        .await
        .expect("settings saved");
    assert_eq!(saved.timer.settings_file_status, SettingsFileStatus::Ok);
    assert_eq!(saved.timer.state.settings.work_duration_secs, 1);
    let notifications = sink.emitted_payloads("settings_changed");
    assert_eq!(notifications.len(), 1);
    assert_eq!(notifications[0]["settings_file_status"], "ok");
}

#[tokio::test]
async fn reading_full_state_does_not_create_a_boot_offer_or_write_files() {
    let now = Local
        .with_ymd_and_hms(2025, 2, 3, 8, 30, 0)
        .single()
        .expect("valid local datetime");
    let clock = Arc::new(FixedClock::new(now));
    let data_dir = create_temp_dir("read-only-state");
    let mut settings = SettingsInput::from(UserSettings::default());
    settings.auto_run_feature_enabled = true;
    settings.weekly_auto_run_config.mon = daytime_slot(true);
    let clock_trait: Arc<dyn Clock + Send + Sync> = Arc::<FixedClock>::clone(&clock);
    let runtime = TimerRuntime::new(
        TimerCore::restore(
            None,
            UserSettings::try_from(settings).expect("valid settings"),
            now.with_timezone(&chrono::Utc),
            Local,
        )
        .0,
        clock_trait,
        PersistenceManager::new(Some(data_dir.clone())),
        PersistenceManager::new(Some(data_dir)),
        SettingsFileStatus::Ok,
    );
    let sink = RecordingSink::with_lock_outcome(EffectStatus::Succeeded);

    assert!(runtime
        .read_full_state()
        .await
        .timer
        .state
        .boot_auto_start_offer
        .is_none());
    clock.set_now_local(now + chrono::Duration::hours(1));
    for _ in 0..2 {
        let state = runtime.read_full_state().await.timer.state;
        assert_eq!(state.status, TimerStatus::Idle);
        assert!(state.active_window_summary.is_some());
        assert!(state.boot_auto_start_offer.is_none());
    }
    assert!(saved_state(&runtime).is_none());
    assert!(saved_settings(&runtime).is_none());

    runtime
        .dispatch(
            &sink,
            Event::TimeObserved {
                credited_break_seconds: 1,
            },
        )
        .await
        .expect("tick should start the scheduled work");
    assert_eq!(
        runtime.read_full_state().await.timer.state.status,
        TimerStatus::Working
    );
    assert!(saved_state(&runtime).is_some());
}

#[tokio::test]
async fn a_tick_closes_the_startup_offer_and_announces_it_once() {
    let now = Local
        .with_ymd_and_hms(2025, 2, 3, 17, 59, 59)
        .single()
        .expect("valid local datetime");
    let clock = Arc::new(FixedClock::new(now));
    let data_dir = create_temp_dir("expire-startup-offer");
    let mut settings = SettingsInput::from(UserSettings::default());
    settings.auto_run_feature_enabled = true;
    settings.weekly_auto_run_config.mon = daytime_slot(true);
    let clock_trait: Arc<dyn Clock + Send + Sync> = Arc::<FixedClock>::clone(&clock);
    let runtime = TimerRuntime::new(
        TimerCore::restore(
            None,
            UserSettings::try_from(settings).expect("valid settings"),
            now.with_timezone(&chrono::Utc),
            Local,
        )
        .0,
        clock_trait,
        PersistenceManager::new(Some(data_dir.clone())),
        PersistenceManager::new(Some(data_dir)),
        SettingsFileStatus::Ok,
    );
    let sink = RecordingSink::with_lock_outcome(EffectStatus::Succeeded);
    let offer = runtime
        .read_full_state()
        .await
        .timer
        .state
        .boot_auto_start_offer;
    assert!(offer.is_some());
    clock.set_now_local(now + chrono::Duration::seconds(1));
    assert_eq!(
        runtime
            .read_full_state()
            .await
            .timer
            .state
            .boot_auto_start_offer,
        offer
    );
    assert!(sink.emitted_names().is_empty());
    assert!(saved_state(&runtime).is_none());
    assert!(saved_settings(&runtime).is_none());

    for _ in 0..2 {
        runtime
            .dispatch(
                &sink,
                Event::TimeObserved {
                    credited_break_seconds: 1,
                },
            )
            .await
            .expect("tick succeeds");
    }

    assert!(runtime
        .read_full_state()
        .await
        .timer
        .state
        .boot_auto_start_offer
        .is_none());
    assert_eq!(
        sink.emitted_payloads("boot_auto_start_offer_closed"),
        vec![serde_json::Value::Null]
    );
}

#[tokio::test]
async fn reading_full_state_does_not_complete_an_expired_work_session() {
    let clock = fixed_clock();
    let runtime = idle_runtime(&create_temp_dir("read-expired-work"), &clock);
    let sink = RecordingSink::with_lock_outcome(EffectStatus::Succeeded);
    start_one_second_work(&runtime, &sink).await;
    let saved_before_reads = saved_state(&runtime).expect("work should be saved");
    let emitted_before_reads = sink.emitted_names();
    clock.set_now_local(clock.now_local() + chrono::Duration::seconds(1));

    for _ in 0..2 {
        let state = runtime.read_full_state().await.timer.state;
        assert_eq!(state.status, TimerStatus::Working);
        assert_eq!(state.remaining_seconds, 0);
        assert_eq!(state.completed_pomodoros, 0);
    }
    let saved_after_reads = saved_state(&runtime).expect("saved work should remain");
    assert_eq!(
        saved_after_reads.timer.status,
        saved_before_reads.timer.status
    );
    assert_eq!(
        saved_after_reads.timer.saved_at,
        saved_before_reads.timer.saved_at
    );
    assert_eq!(
        saved_after_reads.timer.remaining_seconds,
        saved_before_reads.timer.remaining_seconds
    );
    assert_eq!(sink.emitted_names(), emitted_before_reads);

    runtime
        .dispatch(
            &sink,
            Event::TimeObserved {
                credited_break_seconds: 1,
            },
        )
        .await
        .expect("tick should complete the work");
    assert_eq!(
        runtime.read_full_state().await.timer.state.status,
        TimerStatus::OnBreak
    );
}

#[tokio::test(start_paused = true)]
async fn reading_full_state_waits_for_the_break_lock_outcome() {
    let clock = fixed_clock();
    let runtime = idle_runtime(&create_temp_dir("read-during-break-entry"), &clock);
    let sink = RecordingSink::with_lock_outcome(EffectStatus::Succeeded)
        .with_slow_lock(std::time::Duration::from_millis(50));
    start_one_second_work(&runtime, &sink).await;
    clock.set_now_local(clock.now_local() + chrono::Duration::seconds(1));

    let (tick, state) = tokio::join!(
        biased;
        runtime.dispatch(&sink, Event::TimeObserved { credited_break_seconds: 1 }),
        runtime.read_full_state(),
    );

    tick.expect("tick should succeed");
    assert_eq!(state.timer.state.status, TimerStatus::OnBreak);
    assert_eq!(
        state
            .effects
            .break_lock
            .last_attempt
            .expect("lock outcome")
            .status,
        EffectStatus::Succeeded
    );
}
