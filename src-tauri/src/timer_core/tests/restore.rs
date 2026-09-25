// Restore semantics

use super::*;

#[test]
fn restored_work_deducts_the_time_the_app_was_closed() {
    let saved_at = monday(10, 0, 0);
    let saved = saved_state(TimerStatus::Working, 100, saved_at);

    let core = TimerCore::restore(Some(saved), seconds_after(saved_at, 40));

    assert_eq!(remaining(&core, seconds_after(saved_at, 40)), 100 - 40);
}

#[test]
fn work_that_expired_while_the_app_was_closed_enters_its_break_on_start() {
    let saved_at = monday(10, 0, 0);
    let saved = saved_state(TimerStatus::Working, 3, saved_at);
    let restarted_at = seconds_after(saved_at, 60);
    let mut core = TimerCore::restore(Some(saved), restarted_at);

    let on_start = apply(&mut core, Event::Started, restarted_at);

    assert_eq!(core.status(), TimerStatus::OnBreak);
    assert_eq!(
        completion_reason(&on_start),
        Some(SessionTransitionReason::WorkToBreak)
    );
    assert!(on_start.contains(&Effect::EnterBreakLock));
}

#[test]
fn a_restored_break_keeps_its_remaining_time_and_re_enters_enforcement() {
    let saved_at = monday(10, 0, 0);
    let mut saved = saved_state(TimerStatus::OnBreak, 180, saved_at);
    saved.session_type = SessionType::ShortBreak;
    let restarted_at = seconds_after(saved_at, 30 * 60);
    let mut core = TimerCore::restore(Some(saved), restarted_at);

    let on_start = apply(&mut core, Event::Started, restarted_at);

    assert_eq!(core.status(), TimerStatus::OnBreak);
    assert_eq!(remaining(&core, restarted_at), 180);
    assert!(on_start.contains(&Effect::EnterBreakLock));
    assert!(completion_reason(&on_start).is_none());
}

#[test]
fn a_restored_work_countdown_never_exceeds_the_configured_duration() {
    let saved_at = monday(10, 0, 0);
    let saved = saved_state(TimerStatus::Working, 99_999, saved_at);

    let core = TimerCore::restore(Some(saved), saved_at);

    assert_eq!(remaining(&core, saved_at), default_work_secs());
}

#[test]
fn a_restored_break_countdown_never_exceeds_the_configured_duration() {
    let saved_at = monday(10, 0, 0);
    let mut saved = saved_state(TimerStatus::OnBreak, 99_999, saved_at);
    saved.session_type = SessionType::ShortBreak;

    let core = TimerCore::restore(Some(saved), saved_at);

    assert_eq!(remaining(&core, saved_at), default_short_break_secs());
}

#[test]
fn a_save_from_the_future_does_not_shorten_the_restored_work() {
    let restarted_at = monday(10, 0, 0);
    let saved_in_the_future = seconds_after(restarted_at, 5 * 60);
    let saved = saved_state(TimerStatus::Working, 1200, saved_in_the_future);

    let core = TimerCore::restore(Some(saved), restarted_at);

    assert_eq!(remaining(&core, restarted_at), 1200);
}

#[test]
fn a_restored_pending_decision_still_requires_a_decision() {
    let saved_at = monday(10, 0, 0);
    let mut saved = saved_state(TimerStatus::OnBreak, 42, saved_at);
    saved.post_break_decision_pending = true;
    let restarted_at = seconds_after(saved_at, 5 * 60);
    let mut core = TimerCore::restore(Some(saved), restarted_at);

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
    let mut core = TimerCore::restore(Some(saved), saved_at);

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
    start_work(&mut core, started_at, TimerSettings::default());
    apply(&mut core, Event::EmergencyStopRequested, stopped_at);
    let saved = core.persisted_state(stopped_at, stopped_at.with_timezone(&Utc));

    let restored = TimerCore::restore(Some(saved), seconds_after(stopped_at, 60 * 60));

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
    start_work(&mut core, started_at, TimerSettings::default());

    let saved = core.persisted_state(saved_at, saved_at.with_timezone(&Utc));

    assert_eq!(saved.status, TimerStatus::Working);
    assert_eq!(saved.remaining_seconds, default_work_secs() - 10);
}
