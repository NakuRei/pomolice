// Guards

use super::*;

#[test]
fn a_manual_start_right_after_the_last_transition_is_rate_limited() {
    let stopped_at = monday(10, 0, 5);
    let mut core = idle_without_schedule(monday(10, 0, 0));
    start_work(&mut core, monday(10, 0, 0), TimerSettings::default());
    apply(&mut core, Event::EmergencyStopRequested, stopped_at);

    let rejected = core.apply(
        Event::StartWorkRequested {
            settings: TimerSettings::default(),
        },
        millis_after(stopped_at, 1),
    );

    assert!(matches!(
        rejected,
        Err(PomoliceError::RateLimited { remaining_ms }) if remaining_ms > 0
    ));
    assert_eq!(core.status(), TimerStatus::Idle);
}

#[test]
fn a_scheduled_start_is_not_rate_limited() {
    let mut core = idle_with_schedule(seconds_before(workday_start(), 60), workday());
    start_work(
        &mut core,
        seconds_before(workday_start(), 60),
        TimerSettings::default(),
    );
    let stopped_just_before_the_boundary = millis_before(workday_start(), 100);
    apply(
        &mut core,
        Event::EmergencyStopRequested,
        stopped_just_before_the_boundary,
    );

    let at_start = apply(&mut core, Event::Tick, workday_start());

    assert!(starts_work(&at_start));
}

#[test]
fn a_clock_rollback_does_not_rate_limit_forever() {
    let mut core = idle_without_schedule(monday(10, 0, 0));
    start_work(&mut core, monday(10, 0, 0), TimerSettings::default());
    apply(&mut core, Event::EmergencyStopRequested, monday(10, 0, 5));

    let started = core.apply(
        Event::StartWorkRequested {
            settings: TimerSettings::default(),
        },
        monday(9, 0, 0),
    );

    assert!(started.is_ok());
    assert_eq!(core.status(), TimerStatus::Working);
}

#[test]
fn an_input_penalty_resets_the_break_and_is_reported() {
    let started_at = monday(10, 0, 0);
    let mut core = idle_without_schedule(started_at);
    reach_break_after_one_second_of_work(&mut core, started_at, Durations::default());
    apply(&mut core, Event::Tick, seconds_after(started_at, 2));
    assert_eq!(
        remaining(&core, seconds_after(started_at, 2)),
        default_short_break_secs() - SECONDS_PER_TICK
    );

    let penalized = apply(
        &mut core,
        Event::InputDetected,
        seconds_after(started_at, 2),
    );

    assert_eq!(
        remaining(&core, seconds_after(started_at, 2)),
        default_short_break_secs()
    );
    assert_eq!(last_published(&penalized).break_interaction_count, 1);
}

#[test]
fn a_burst_of_input_counts_as_one_penalty() {
    let started_at = monday(10, 0, 0);
    let burst_at = seconds_after(started_at, 2);
    let mut core = idle_without_schedule(started_at);
    reach_break_after_one_second_of_work(&mut core, started_at, Durations::default());

    apply(&mut core, Event::InputDetected, burst_at);
    apply(&mut core, Event::InputDetected, burst_at);
    apply(&mut core, Event::InputDetected, millis_after(burst_at, 1));
    assert_eq!(core.tick_payload(burst_at).break_interaction_count, 1);

    apply(&mut core, Event::InputDetected, seconds_after(burst_at, 1));
    assert_eq!(
        core.tick_payload(seconds_after(burst_at, 1))
            .break_interaction_count,
        2
    );
}

#[test]
fn input_outside_a_break_changes_nothing() {
    let mut core = idle_without_schedule(monday(10, 0, 0));

    let while_idle = core.tick_payload(monday(10, 0, 1));
    apply(&mut core, Event::InputDetected, monday(10, 0, 1));
    assert_eq!(core.tick_payload(monday(10, 0, 1)), while_idle);

    start_work(&mut core, monday(10, 0, 1), TimerSettings::default());
    let while_working = core.tick_payload(monday(10, 0, 2));
    apply(&mut core, Event::InputDetected, monday(10, 0, 2));
    assert_eq!(core.tick_payload(monday(10, 0, 2)), while_working);
}

#[test]
fn a_post_break_decision_is_rejected_unless_one_is_pending() {
    let mut core = idle_without_schedule(monday(10, 0, 0));

    let rejected = core.apply(
        Event::PostBreakDecisionResolved {
            action: PostBreakDecisionAction::Continue,
        },
        monday(10, 0, 0),
    );

    assert!(matches!(
        rejected,
        Err(PomoliceError::InvalidStateTransition { .. })
    ));
}

#[test]
fn break_attendance_is_rejected_outside_a_break() {
    let mut core = idle_without_schedule(monday(10, 0, 0));

    let rejected = core.apply(
        Event::BreakAttendanceSet {
            attendance: BreakAttendance::Away,
        },
        monday(10, 0, 0),
    );

    assert!(matches!(
        rejected,
        Err(PomoliceError::InvalidStateTransition { .. })
    ));
}

#[test]
fn duration_changes_and_new_starts_are_rejected_while_working() {
    let mut core = idle_without_schedule(monday(10, 0, 0));
    start_work(&mut core, monday(10, 0, 0), TimerSettings::default());

    let durations_rejected = core.apply(
        durations_changed(Durations {
            work: 600,
            ..Durations::default()
        }),
        monday(10, 0, 1),
    );
    let start_rejected = core.apply(
        Event::StartWorkRequested {
            settings: TimerSettings::default(),
        },
        monday(10, 0, 1),
    );

    assert!(matches!(
        durations_rejected,
        Err(PomoliceError::InvalidStateTransition { .. })
    ));
    assert!(matches!(
        start_rejected,
        Err(PomoliceError::InvalidStateTransition { .. })
    ));
    let settings_in_effect = core.full_state_payload(monday(10, 0, 1)).settings;
    assert_eq!(
        settings_in_effect.work_duration_secs.get(),
        default_work_secs()
    );
}

#[test]
fn duration_changes_while_idle_update_the_work_preview() {
    let mut core = idle_without_schedule(monday(10, 0, 0));

    let changed = apply(
        &mut core,
        durations_changed(Durations {
            work: 600,
            ..Durations::default()
        }),
        monday(10, 0, 0),
    );

    assert_eq!(last_published(&changed).remaining_seconds, 600);
}
