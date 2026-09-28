// Restore semantics

use super::*;

#[test]
fn restored_work_deducts_the_time_the_app_was_closed() {
    let saved_at = monday(10, 0, 0);
    let saved = saved_state(TimerStatus::Working, 100, saved_at);

    let core = restore_with_default_settings(Some(saved), seconds_after(saved_at, 40));

    assert_eq!(remaining(&core, seconds_after(saved_at, 40)), 100 - 40);
}

#[test]
fn work_that_expired_while_the_app_was_closed_enters_its_break_on_start() {
    let saved_at = monday(10, 0, 0);
    let saved = saved_state(TimerStatus::Working, 3, saved_at);
    let restarted_at = seconds_after(saved_at, 60);
    let mut core = restore_with_default_settings(Some(saved), restarted_at);

    let on_start = apply(&mut core, Event::Started, restarted_at);

    assert_eq!(core.status(), TimerStatus::OnBreak);
    assert_eq!(
        completion_reason(&on_start),
        Some(SessionTransitionReason::WorkToBreak)
    );
    assert!(on_start.contains(&Effect::SetBreakLockTarget(BreakLockTarget::Locked)));
}

#[test]
fn a_restored_break_keeps_its_remaining_time_and_re_enters_enforcement() {
    let saved_at = monday(10, 0, 0);
    let mut saved = saved_state(TimerStatus::OnBreak, 180, saved_at);
    saved.session_type = SessionType::ShortBreak;
    let restarted_at = seconds_after(saved_at, 30 * 60);
    let mut core = restore_with_default_settings(Some(saved), restarted_at);

    let on_start = apply(&mut core, Event::Started, restarted_at);

    assert_eq!(core.status(), TimerStatus::OnBreak);
    assert_eq!(remaining(&core, restarted_at), 180);
    assert!(on_start.contains(&Effect::SetBreakLockTarget(BreakLockTarget::Locked)));
    assert!(completion_reason(&on_start).is_none());
}

#[test]
fn a_restored_work_countdown_never_exceeds_the_duration_it_started_with() {
    let saved_at = monday(10, 0, 0);
    let saved = saved_state(TimerStatus::Working, 99_999, saved_at);

    let core = restore_with_default_settings(Some(saved), saved_at);

    assert_eq!(remaining(&core, saved_at), default_work_secs());
}

#[test]
fn a_restored_break_countdown_never_exceeds_the_duration_it_started_with() {
    let saved_at = monday(10, 0, 0);
    let mut saved = saved_state(TimerStatus::OnBreak, 99_999, saved_at);
    saved.session_type = SessionType::ShortBreak;
    saved.session_duration_secs =
        SessionDurationSecs::try_new(default_short_break_secs()).expect("valid duration");

    let core = restore_with_default_settings(Some(saved), saved_at);

    assert_eq!(remaining(&core, saved_at), default_short_break_secs());
}

#[test]
fn a_restored_session_keeps_the_duration_it_started_with_under_changed_settings() {
    let saved_at = monday(10, 0, 0);
    let saved = saved_state(TimerStatus::Working, 1_200, saved_at);
    let shorter_work = with_durations(
        UserSettings::default(),
        Durations {
            work: 600,
            ..Durations::default()
        },
    );

    let core = TimerCore::restore(Some(saved), shorter_work, SettingsFileStatus::Ok, saved_at);

    let restored = core.tick_payload(saved_at);
    assert_eq!(restored.remaining_seconds, 1_200);
    assert_eq!(restored.session_duration_secs, default_work_secs());
}

#[test]
fn a_save_from_the_future_does_not_shorten_the_restored_work() {
    let restarted_at = monday(10, 0, 0);
    let saved_in_the_future = seconds_after(restarted_at, 5 * 60);
    let saved = saved_state(TimerStatus::Working, 1200, saved_in_the_future);

    let core = restore_with_default_settings(Some(saved), restarted_at);

    assert_eq!(remaining(&core, restarted_at), 1200);
}

#[test]
fn a_restored_pending_decision_still_requires_a_decision() {
    let saved_at = monday(10, 0, 0);
    let mut saved = saved_state(TimerStatus::OnBreak, 42, saved_at);
    saved.post_break_decision_pending = true;
    let restarted_at = seconds_after(saved_at, 5 * 60);
    let mut core = restore_with_default_settings(Some(saved), restarted_at);

    let on_start = apply(&mut core, Event::Started, restarted_at);

    assert!(!starts_work(&on_start));
    assert_eq!(core.status(), TimerStatus::Idle);
    assert!(core.post_break_decision_pending());
    assert_eq!(remaining(&core, restarted_at), default_work_secs());

    apply(
        &mut core,
        Event::PostBreakDecisionResolved {
            action: PostBreakDecisionAction::Continue,
        },
        seconds_after(restarted_at, 1),
    );
    assert_eq!(core.status(), TimerStatus::Working);
}

#[test]
fn a_saved_break_without_a_break_kind_restarts_idle() {
    let saved_at = monday(10, 0, 0);
    let mut saved = saved_state(TimerStatus::OnBreak, 42, saved_at);
    saved.session_type = SessionType::Work;
    let mut core = restore_with_default_settings(Some(saved), saved_at);

    let on_start = apply(&mut core, Event::Started, saved_at);

    assert!(!starts_work(&on_start));
    assert_eq!(core.status(), TimerStatus::Idle);
    assert!(!core.post_break_decision_pending());
    assert_eq!(remaining(&core, saved_at), default_work_secs());
}

#[test]
fn an_emergency_stop_before_exit_restarts_idle() {
    let started_at = monday(10, 0, 0);
    let stopped_at = seconds_after(started_at, 60);
    let mut core = idle_without_schedule(started_at);
    start_work(&mut core, started_at, Durations::default());
    apply(&mut core, Event::EmergencyStopRequested, stopped_at);
    let saved = core.persisted_state(stopped_at, stopped_at.with_timezone(&Utc));

    let restored = restore_with_default_settings(Some(saved), seconds_after(stopped_at, 60 * 60));

    assert_eq!(restored.status(), TimerStatus::Idle);
    assert_eq!(
        remaining(&restored, seconds_after(stopped_at, 60 * 60)),
        default_work_secs()
    );
}

#[test]
fn persisted_work_remaining_is_derived_from_the_wall_clock() {
    let started_at = monday(10, 0, 0);
    let saved_at = seconds_after(started_at, 10);
    let mut core = idle_without_schedule(started_at);
    start_work(&mut core, started_at, Durations::default());

    let saved = core.persisted_state(saved_at, saved_at.with_timezone(&Utc));

    assert_eq!(saved.status, TimerStatus::Working);
    assert_eq!(saved.remaining_seconds, default_work_secs() - 10);
}

#[test]
fn a_restored_break_longer_than_any_break_setting_takes_the_configured_length() {
    let saved_at = monday(10, 0, 0);
    let mut saved = saved_state(TimerStatus::OnBreak, 5_000, saved_at);
    saved.session_type = SessionType::ShortBreak;
    saved.session_duration_secs = SessionDurationSecs::try_new(7_200).expect("valid duration");

    let core = restore_with_default_settings(Some(saved), saved_at);

    let restored = core.tick_payload(saved_at);
    assert_eq!(restored.session_duration_secs, default_short_break_secs());
    assert_eq!(restored.remaining_seconds, default_short_break_secs());
}
