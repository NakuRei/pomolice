use super::*;
use crate::timer_effects::{BreakLockAction, BreakLockOutcome, EffectStatus};
use std::sync::atomic::{AtomicBool, Ordering};

fn switchable_persistence(
    test_name: &str,
    fail: &Arc<AtomicBool>,
) -> PersistenceManager<PersistedState> {
    let fail = Arc::clone(fail);
    PersistenceManager::new(Some(create_temp_dir(test_name))).with_injected_save_error_hook(
        Arc::new(move || {
            fail.load(Ordering::SeqCst).then(|| SaveError::Write {
                path: PathBuf::from("private-timer-path"),
                source: std::io::Error::other("private diagnostic"),
            })
        }),
    )
}

#[tokio::test]
async fn failed_save_is_an_accepted_start_and_retry_saves_the_current_session() {
    let clock = fixed_clock();
    let fail = Arc::new(AtomicBool::new(true));
    let runtime = runtime_with(
        switchable_persistence("accepted-start", &fail),
        PersistenceManager::new(Some(create_temp_dir("outcome-settings"))),
        &clock,
    );
    let sink = RecordingSink::with_lock_outcome(EffectStatus::Succeeded);

    let outcome = runtime
        .dispatch(&sink, Event::StartWorkRequested)
        .await
        .expect("start accepted");
    assert_eq!(outcome.timer.status, TimerStatus::Working);
    assert_eq!(outcome.effects.last_timer_save, Some(EffectStatus::Failed));
    assert_eq!(outcome.effects.break_lock.last_attempt, None);
    let encoded = serde_json::to_string(&outcome).expect("serializable outcome");
    assert!(!encoded.contains("private"));

    for _ in 0..2 {
        assert_eq!(runtime.read_full_state().await.effects, outcome.effects);
    }
    assert!(saved_state(&runtime).is_none());
    fail.store(false, Ordering::SeqCst);
    clock.set_now_local(clock.now_local() + chrono::Duration::seconds(10));

    let retried = runtime.retry_failed_effects(&sink).await;
    assert_eq!(retried.timer.status, TimerStatus::Working);
    assert_eq!(
        retried.timer.remaining_seconds,
        outcome.timer.remaining_seconds - 10
    );
    assert_eq!(retried.timer.current_cycle, outcome.timer.current_cycle);
    assert_eq!(
        retried.effects.last_timer_save,
        Some(EffectStatus::Succeeded)
    );
    assert_eq!(
        saved_state(&runtime).expect("saved").remaining_seconds,
        retried.timer.remaining_seconds
    );
}

#[tokio::test]
async fn stop_reports_both_unlock_and_save_failures_and_keeps_them_until_recovery() {
    let clock = fixed_clock();
    let fail = Arc::new(AtomicBool::new(false));
    let runtime = runtime_with(
        switchable_persistence("stop-partial-failure", &fail),
        PersistenceManager::new(Some(create_temp_dir("outcome-settings"))),
        &clock,
    );
    let mut sink = RecordingSink::with_lock_outcome(EffectStatus::Succeeded);
    start_one_second_work(&runtime, &sink).await;
    clock.set_now_local(clock.now_local() + chrono::Duration::seconds(1));
    runtime
        .dispatch(&sink, Event::Tick)
        .await
        .expect("break starts");
    sink.release_fails = true;
    fail.store(true, Ordering::SeqCst);

    let stopped = runtime
        .dispatch(&sink, Event::EmergencyStopRequested)
        .await
        .expect("stop accepted");
    assert_eq!(stopped.timer.status, TimerStatus::Idle);
    assert_eq!(
        stopped.effects.break_lock.last_attempt,
        Some(BreakLockOutcome {
            action: BreakLockAction::Release,
            status: EffectStatus::Failed
        })
    );
    assert_eq!(stopped.effects.last_timer_save, Some(EffectStatus::Failed));
    runtime
        .dispatch(&sink, Event::Tick)
        .await
        .expect("idle tick");
    assert_eq!(runtime.read_full_state().await.effects, stopped.effects);
    assert_eq!(
        sink.emitted_payloads("timer_effects_changed")
            .last()
            .expect("failure announced")["break_lock"]["last_attempt"]["status"],
        "failed"
    );

    sink.release_fails = false;
    fail.store(false, Ordering::SeqCst);
    let recovered = runtime.retry_failed_effects(&sink).await;
    assert_eq!(recovered.timer.status, TimerStatus::Idle);
    assert_eq!(
        recovered.effects.break_lock.last_attempt,
        Some(BreakLockOutcome {
            action: BreakLockAction::Release,
            status: EffectStatus::Succeeded
        })
    );
    assert_eq!(
        recovered.effects.last_timer_save,
        Some(EffectStatus::Succeeded)
    );
    assert_eq!(
        saved_state(&runtime).expect("saved stop").status,
        TimerStatus::Idle
    );
}

#[tokio::test]
async fn failed_break_entry_is_observable_even_without_a_frontend_command() {
    let clock = fixed_clock();
    let runtime = idle_runtime(&create_temp_dir("failed-entry"), &clock);
    let failed_sink = RecordingSink::with_lock_outcome(EffectStatus::Failed);
    start_one_second_work(&runtime, &failed_sink).await;
    clock.set_now_local(clock.now_local() + chrono::Duration::seconds(1));

    let outcome = runtime
        .dispatch(&failed_sink, Event::Tick)
        .await
        .expect("break accepted");
    assert_eq!(outcome.timer.status, TimerStatus::OnBreak);
    assert_eq!(outcome.effects.break_lock.target, BreakLockTarget::Locked);
    assert_eq!(
        outcome.effects.break_lock.last_attempt,
        Some(BreakLockOutcome {
            action: BreakLockAction::Enter,
            status: EffectStatus::Failed
        })
    );
    assert_eq!(
        outcome.effects.last_timer_save,
        Some(EffectStatus::Succeeded)
    );

    let recovered = runtime
        .retry_failed_effects(&RecordingSink::with_lock_outcome(EffectStatus::Succeeded))
        .await;
    assert_eq!(
        recovered.timer.remaining_seconds,
        outcome.timer.remaining_seconds
    );
    assert_eq!(recovered.effects.break_lock.target, BreakLockTarget::Locked);
    assert_eq!(
        recovered
            .effects
            .break_lock
            .last_attempt
            .expect("entry retried")
            .status,
        EffectStatus::Succeeded
    );
}

#[tokio::test]
async fn recovery_never_relocks_a_break_that_has_already_ended() {
    let clock = fixed_clock();
    let runtime = idle_runtime(&create_temp_dir("obsolete-entry"), &clock);
    let sink = RecordingSink::with_lock_outcome(EffectStatus::Failed);
    start_one_second_work(&runtime, &sink).await;
    clock.set_now_local(clock.now_local() + chrono::Duration::seconds(1));
    runtime
        .dispatch(&sink, Event::Tick)
        .await
        .expect("break starts");
    runtime
        .dispatch(&sink, Event::EmergencyStopRequested)
        .await
        .expect("stop accepted");

    let recovered = runtime.retry_failed_effects(&sink).await;
    assert_eq!(recovered.timer.status, TimerStatus::Idle);
    assert_eq!(
        recovered.effects.break_lock.last_attempt,
        Some(BreakLockOutcome {
            action: BreakLockAction::Release,
            status: EffectStatus::Succeeded
        })
    );
}

#[tokio::test]
async fn recovery_with_no_failed_effects_is_a_noop() {
    let clock = fixed_clock();
    let runtime = idle_runtime(&create_temp_dir("nothing-to-retry"), &clock);
    let sink = RecordingSink::with_lock_outcome(EffectStatus::Succeeded);
    let before = runtime.read_full_state().await;
    let retried = runtime.retry_failed_effects(&sink).await;
    assert_eq!(retried.effects, before.effects);
    assert!(sink.emitted_names().is_empty());
    assert!(saved_state(&runtime).is_none());
}

#[tokio::test]
async fn retrying_entry_cleans_up_partial_windows_before_trying_to_lock_again() {
    let clock = fixed_clock();
    let runtime = idle_runtime(&create_temp_dir("retry-partial-entry"), &clock);
    let failed_entry = RecordingSink::with_lock_outcome(EffectStatus::Failed);
    start_one_second_work(&runtime, &failed_entry).await;
    clock.set_now_local(clock.now_local() + chrono::Duration::seconds(1));
    runtime
        .dispatch(&failed_entry, Event::Tick)
        .await
        .expect("break starts");
    let mut recovery = RecordingSink::with_lock_outcome(EffectStatus::Succeeded);
    recovery.release_fails = true;

    let blocked = runtime.retry_failed_effects(&recovery).await;
    assert_eq!(
        *recovery.lock_actions.lock().expect("actions"),
        vec![BreakLockAction::Release]
    );
    assert_eq!(
        blocked.effects.break_lock.last_attempt,
        Some(BreakLockOutcome {
            action: BreakLockAction::Release,
            status: EffectStatus::Failed
        })
    );
    assert_eq!(blocked.effects.break_lock.target, BreakLockTarget::Locked);

    recovery.release_fails = false;
    let recovered = runtime.retry_failed_effects(&recovery).await;
    assert_eq!(
        *recovery.lock_actions.lock().expect("actions"),
        vec![
            BreakLockAction::Release,
            BreakLockAction::Release,
            BreakLockAction::Enter
        ]
    );
    assert_eq!(recovered.effects.break_lock.target, BreakLockTarget::Locked);
}

#[tokio::test]
async fn unavailable_persistence_never_reports_a_successful_save() {
    let clock = fixed_clock();
    let runtime = runtime_with(
        PersistenceManager::new(None),
        PersistenceManager::new(Some(create_temp_dir("missing-timer-path-settings"))),
        &clock,
    );
    let sink = RecordingSink::with_lock_outcome(EffectStatus::Succeeded);
    let started = runtime
        .dispatch(&sink, Event::StartWorkRequested)
        .await
        .expect("accepted");
    assert_eq!(started.timer.status, TimerStatus::Working);
    assert_eq!(started.effects.last_timer_save, Some(EffectStatus::Failed));
    let retried = runtime.retry_failed_effects(&sink).await;
    assert_eq!(retried.effects.last_timer_save, Some(EffectStatus::Failed));
    assert_eq!(retried.timer.current_cycle, started.timer.current_cycle);
}

#[tokio::test]
async fn accepted_command_and_read_share_the_same_snapshot_contract() {
    let clock = fixed_clock();
    let runtime = idle_runtime(&create_temp_dir("shared-snapshot"), &clock);
    let sink = RecordingSink::with_lock_outcome(EffectStatus::Succeeded);
    let accepted = runtime
        .dispatch(&sink, Event::StartWorkRequested)
        .await
        .expect("accepted");
    let observed = runtime.read_full_state().await;
    assert_eq!(accepted, observed);
    let json = serde_json::to_value(observed).expect("snapshot serializes");
    assert!(json.get("status").is_none());
    assert!(json["timer"].get("is_fullscreen_break").is_none());
    assert_eq!(json["timer"]["status"], "working");
}

#[tokio::test]
async fn stopping_after_failed_entry_and_cleanup_retries_only_the_release() {
    let clock = fixed_clock();
    let runtime = idle_runtime(&create_temp_dir("stop-unknown-lock"), &clock);
    let mut sink = RecordingSink::with_lock_outcome(EffectStatus::Failed);
    start_one_second_work(&runtime, &sink).await;
    clock.set_now_local(clock.now_local() + chrono::Duration::seconds(1));
    runtime.dispatch(&sink, Event::Tick).await.expect("break");
    sink.release_fails = true;
    let stopped = runtime
        .dispatch(&sink, Event::EmergencyStopRequested)
        .await
        .expect("stop");
    assert_eq!(stopped.timer.status, TimerStatus::Idle);
    assert_eq!(stopped.effects.break_lock.target, BreakLockTarget::Unlocked);
    assert_eq!(
        stopped.effects.break_lock.last_attempt,
        Some(BreakLockOutcome {
            action: BreakLockAction::Release,
            status: EffectStatus::Failed,
        })
    );
    sink.release_fails = false;
    sink.lock_actions.lock().expect("actions").clear();
    let recovered = runtime.retry_failed_effects(&sink).await;
    assert_eq!(
        *sink.lock_actions.lock().expect("actions"),
        vec![BreakLockAction::Release]
    );
    assert_eq!(recovered.timer.status, TimerStatus::Idle);
    assert_eq!(
        recovered
            .effects
            .break_lock
            .last_attempt
            .expect("attempt")
            .status,
        EffectStatus::Succeeded
    );
}

#[tokio::test]
async fn escape_changes_the_lock_target_without_changing_the_break_or_saving_it_again() {
    let clock = fixed_clock();
    let fail = Arc::new(AtomicBool::new(false));
    let runtime = runtime_with(
        switchable_persistence("escape-target", &fail),
        PersistenceManager::new(Some(create_temp_dir("escape-settings"))),
        &clock,
    );
    let mut sink = RecordingSink::with_lock_outcome(EffectStatus::Succeeded);
    start_one_second_work(&runtime, &sink).await;
    clock.set_now_local(clock.now_local() + chrono::Duration::seconds(1));
    let before = runtime.dispatch(&sink, Event::Tick).await.expect("break");
    fail.store(true, Ordering::SeqCst);
    sink.release_fails = true;
    let escaped = runtime
        .dispatch(&sink, Event::EscapeComboPressed)
        .await
        .expect("escape");
    assert_eq!(escaped.timer, before.timer);
    assert_eq!(
        escaped.effects.last_timer_save,
        Some(EffectStatus::Succeeded)
    );
    assert_eq!(escaped.effects.break_lock.target, BreakLockTarget::Unlocked);
    sink.release_fails = false;
    let recovered = runtime.retry_failed_effects(&sink).await;
    assert_eq!(recovered.timer, before.timer);
    assert_eq!(
        recovered.effects.last_timer_save,
        Some(EffectStatus::Succeeded)
    );
    assert_eq!(
        recovered
            .effects
            .break_lock
            .last_attempt
            .expect("release")
            .action,
        BreakLockAction::Release
    );
}
