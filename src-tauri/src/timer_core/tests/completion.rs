// Session completion

use super::*;

#[test]
fn work_completion_announces_the_break_and_asks_for_the_lock() {
    let started_at = monday(10, 0, 0);
    let mut core = idle_without_schedule(started_at);
    start_work(
        &mut core,
        started_at,
        Durations {
            work: 2,
            ..Durations::default()
        },
    );

    let at_work_end = apply(&mut core, Event::Tick, seconds_after(started_at, 2));

    assert_eq!(core.status(), TimerStatus::OnBreak);
    assert_eq!(
        completion_reason(&at_work_end),
        Some(SessionTransitionReason::WorkToBreak)
    );
    assert!(at_work_end.contains(&Effect::SetBreakLockTarget(BreakLockTarget::Locked)));
    let break_state = core.tick_payload(seconds_after(started_at, 2));
    assert_eq!(break_state.completed_pomodoros, 1);
    assert_eq!(break_state.session_type, SessionType::ShortBreak);
    assert_eq!(break_state.remaining_seconds, default_short_break_secs());
}

#[test]
fn break_completion_announces_the_decision_and_releases_the_lock() {
    let started_at = monday(10, 0, 0);
    let mut core = idle_without_schedule(started_at);
    reach_break_after_one_second_of_work(
        &mut core,
        started_at,
        Durations {
            short_break: 2,
            ..Durations::default()
        },
    );
    apply(&mut core, Event::Tick, seconds_after(started_at, 2));

    let at_break_end = apply(&mut core, Event::Tick, seconds_after(started_at, 3));

    assert_eq!(core.status(), TimerStatus::Idle);
    assert!(core.post_break_decision_pending());
    assert_eq!(
        completion_reason(&at_break_end),
        Some(SessionTransitionReason::BreakWaitingDecision)
    );
    assert!(at_break_end.contains(&Effect::SetBreakLockTarget(BreakLockTarget::Unlocked)));

    let final_state = last_published(&at_break_end);
    assert_eq!(final_state.status, TimerStatus::Idle);
}

/// A one-second work session followed by a one-second break, with the given
/// policy and attendance, ticked to the end of the break.
fn complete_one_break(
    policy: PostBreakTransitionPolicy,
    attendance: BreakAttendance,
) -> (TimerCore, Vec<Effect>) {
    let started_at = monday(10, 0, 0);
    let mut core = idle_without_schedule(started_at);
    reach_break_after_one_second_of_work(
        &mut core,
        started_at,
        Durations {
            short_break: 1,
            policy,
            ..Durations::default()
        },
    );
    apply(
        &mut core,
        Event::BreakAttendanceSet { attendance },
        seconds_after(started_at, 1),
    );
    let at_break_end = apply(&mut core, Event::Tick, seconds_after(started_at, 2));
    (core, at_break_end)
}

#[test]
fn break_completion_starts_the_next_work_when_the_policy_says_so_and_the_user_stayed() {
    let (core, at_break_end) = complete_one_break(
        PostBreakTransitionPolicy::StartNextWorkAutomatically,
        BreakAttendance::AtDesk,
    );

    assert_eq!(core.status(), TimerStatus::Working);
    assert_eq!(core.tick_payload(monday(10, 0, 2)).current_cycle, 2);
    assert_eq!(
        completion_reason(&at_break_end),
        Some(SessionTransitionReason::BreakAutoStarted)
    );
}

#[test]
fn break_completion_waits_for_a_decision_when_the_user_was_away() {
    let (core, at_break_end) = complete_one_break(
        PostBreakTransitionPolicy::StartNextWorkAutomatically,
        BreakAttendance::Away,
    );

    assert_eq!(core.status(), TimerStatus::Idle);
    assert!(core.post_break_decision_pending());
    assert_eq!(
        completion_reason(&at_break_end),
        Some(SessionTransitionReason::BreakWaitingDecision)
    );
}

#[test]
fn a_scheduled_stop_at_break_end_wins_over_an_away_user() {
    let mut core = idle_with_schedule(seconds_before(workday_end(), 1), workday());
    reach_break_after_one_second_of_work(
        &mut core,
        seconds_before(workday_end(), 1),
        Durations {
            short_break: 1,
            policy: PostBreakTransitionPolicy::StartNextWorkAutomatically,
            ..Durations::default()
        },
    );
    apply(
        &mut core,
        Event::BreakAttendanceSet {
            attendance: BreakAttendance::Away,
        },
        workday_end(),
    );

    let at_break_end = apply(&mut core, Event::Tick, seconds_after(workday_end(), 1));

    assert_eq!(core.status(), TimerStatus::Idle);
    assert!(!core.post_break_decision_pending());
    assert_eq!(
        completion_reason(&at_break_end),
        Some(SessionTransitionReason::ScheduledStopExecuted)
    );
    assert!(at_break_end.contains(&Effect::EmitAutoRunStopped));
}

#[test]
fn a_long_break_resets_the_cycle() {
    let started_at = monday(10, 0, 0);
    let mut core = idle_without_schedule(started_at);
    start_work(
        &mut core,
        started_at,
        Durations {
            work: 1,
            short_break: 1,
            long_break: 1,
            long_break_interval: 2,
            policy: PostBreakTransitionPolicy::StartNextWorkAutomatically,
        },
    );
    apply(&mut core, Event::Tick, seconds_after(started_at, 1)); // work 1 -> short break
    apply(&mut core, Event::Tick, seconds_after(started_at, 2)); // short break -> work 2
    assert_eq!(
        core.tick_payload(seconds_after(started_at, 2))
            .current_cycle,
        2
    );
    apply(&mut core, Event::Tick, seconds_after(started_at, 3)); // work 2 -> long break
    assert_eq!(
        core.tick_payload(seconds_after(started_at, 3)).session_type,
        SessionType::LongBreak
    );

    apply(&mut core, Event::Tick, seconds_after(started_at, 4)); // long break -> work 1

    assert_eq!(core.status(), TimerStatus::Working);
    assert_eq!(
        core.tick_payload(seconds_after(started_at, 4))
            .current_cycle,
        1
    );
}
