// Schedule changes and schedule-driven starts are ordered events

use super::*;

#[test]
fn disabling_the_schedule_right_before_the_start_boundary_prevents_the_auto_start() {
    let mut core = idle_with_schedule(seconds_before(workday_start(), 2), workday());

    disable_schedule(&mut core, seconds_before(workday_start(), 1));
    let at_boundary = apply(
        &mut core,
        Event::TimeObserved {
            credited_break_seconds: 1,
        },
        workday_start(),
    );

    assert_eq!(core.status(), TimerStatus::Idle);
    assert!(!at_boundary.contains(&Effect::Persist));
}

#[test]
fn a_manual_start_inside_the_window_is_scheduled_to_stop_at_the_schedule_end() {
    let mut core = idle_with_schedule(seconds_before(workday_end(), 60), workday());
    start_work(
        &mut core,
        seconds_before(workday_end(), 60),
        Durations::default(),
    );

    let at_end = apply(
        &mut core,
        Event::TimeObserved {
            credited_break_seconds: 1,
        },
        workday_end(),
    );

    assert!(at_end.contains(&Effect::EmitAutoRunStopPending));
}

#[test]
fn a_manual_start_outside_the_window_is_not_scheduled_to_stop_at_the_schedule_end() {
    let one_hour_window = monday_slot((9, 0), (10, 0));
    let before_the_window = monday(8, 30, 0);
    let mut core = idle_with_schedule(before_the_window, one_hour_window);
    let long_enough_to_outlast_the_window = Durations {
        work: 2 * 60 * 60,
        ..Durations::default()
    };
    start_work(
        &mut core,
        before_the_window,
        long_enough_to_outlast_the_window,
    );

    let at_end = apply(
        &mut core,
        Event::TimeObserved {
            credited_break_seconds: 1,
        },
        monday(10, 0, 0),
    );

    assert!(!at_end.contains(&Effect::EmitAutoRunStopPending));
}

#[test]
fn a_manual_start_while_the_schedule_is_disabled_stays_manual_after_enabling_it() {
    let mut core = idle_without_schedule(seconds_before(workday_end(), 60));
    start_work(
        &mut core,
        seconds_before(workday_end(), 60),
        Durations::default(),
    );

    enable_schedule(&mut core, workday(), seconds_before(workday_end(), 30));
    let at_end = apply(
        &mut core,
        Event::TimeObserved {
            credited_break_seconds: 1,
        },
        workday_end(),
    );

    assert!(!at_end.contains(&Effect::EmitAutoRunStopPending));
}

#[test]
fn moving_the_end_time_later_moves_the_scheduled_stop_with_it() {
    let mut core = idle_with_schedule(monday(11, 59, 0), monday_slot((9, 0), (12, 0)));
    start_work(&mut core, monday(11, 59, 0), Durations::default());

    enable_schedule(&mut core, monday_slot((9, 0), (13, 0)), monday(11, 59, 30));
    let at_old_end = apply(
        &mut core,
        Event::TimeObserved {
            credited_break_seconds: 1,
        },
        monday(12, 0, 0),
    );
    let at_new_end = apply(
        &mut core,
        Event::TimeObserved {
            credited_break_seconds: 1,
        },
        monday(13, 0, 0),
    );

    assert!(!at_old_end.contains(&Effect::EmitAutoRunStopPending));
    assert!(at_new_end.contains(&Effect::EmitAutoRunStopPending));
}

#[test]
fn the_end_boundary_lets_the_running_session_finish_and_then_stops() {
    let mut core = idle_with_schedule(seconds_before(workday_end(), 1), workday());
    start_work(
        &mut core,
        seconds_before(workday_end(), 1),
        Durations {
            work: 5,
            policy: PostBreakTransitionPolicy::StartNextWorkAutomatically,
            ..Durations::default()
        },
    );

    let at_end = apply(
        &mut core,
        Event::TimeObserved {
            credited_break_seconds: 1,
        },
        workday_end(),
    );
    assert!(at_end.contains(&Effect::EmitAutoRunStopPending));
    assert_eq!(core.status(), TimerStatus::Working);

    let at_work_end = apply(
        &mut core,
        Event::TimeObserved {
            credited_break_seconds: 1,
        },
        seconds_after(workday_end(), 4),
    );
    assert_eq!(core.status(), TimerStatus::Idle);
    assert!(!core.post_break_decision_pending());
    assert_eq!(
        completion_reason(&at_work_end),
        Some(SessionTransitionReason::ScheduledStopExecuted)
    );
    assert!(at_work_end.contains(&Effect::EmitAutoRunStopped));
    assert_eq!(
        core.snapshot(seconds_after(workday_end(), 4)).status,
        TimerStatus::Idle
    );
}

#[test]
fn enabling_the_schedule_after_the_start_boundary_does_not_replay_it() {
    let mut core = idle_without_schedule(monday(9, 30, 0));

    enable_schedule(&mut core, workday(), monday(9, 30, 0));
    let next_tick = apply(
        &mut core,
        Event::TimeObserved {
            credited_break_seconds: 1,
        },
        monday(9, 30, 1),
    );

    assert_eq!(core.status(), TimerStatus::Idle);
    assert!(!next_tick.contains(&Effect::Persist));
}

#[test]
fn enabling_the_schedule_before_the_start_boundary_starts_work_at_the_boundary() {
    let mut core = idle_with_schedule(seconds_before(workday_start(), 1), workday());

    apply(
        &mut core,
        Event::TimeObserved {
            credited_break_seconds: 1,
        },
        workday_start(),
    );

    assert_eq!(core.status(), TimerStatus::Working);
}

#[test]
fn a_clock_rollback_does_not_refire_a_boundary_already_crossed() {
    let mut core = idle_with_schedule(seconds_before(workday_start(), 1), workday());
    apply(
        &mut core,
        Event::TimeObserved {
            credited_break_seconds: 1,
        },
        workday_start(),
    );
    assert_eq!(core.status(), TimerStatus::Working);
    apply(
        &mut core,
        Event::EmergencyStopRequested,
        seconds_after(workday_start(), 1),
    );

    apply(
        &mut core,
        Event::TimeObserved {
            credited_break_seconds: 1,
        },
        seconds_before(workday_start(), 60),
    );
    let at_start_again = apply(
        &mut core,
        Event::TimeObserved {
            credited_break_seconds: 1,
        },
        workday_start(),
    );

    assert_eq!(core.status(), TimerStatus::Idle);
    assert!(!at_start_again.contains(&Effect::Persist));
}

#[test]
fn work_end_and_schedule_end_on_the_same_second_take_the_break_before_stopping() {
    let mut core = idle_with_schedule(seconds_before(workday_end(), 3), workday());
    start_work(
        &mut core,
        seconds_before(workday_end(), 3),
        Durations {
            work: 3,
            short_break: 2,
            policy: PostBreakTransitionPolicy::StartNextWorkAutomatically,
            ..Durations::default()
        },
    );

    let at_end = apply(
        &mut core,
        Event::TimeObserved {
            credited_break_seconds: 1,
        },
        workday_end(),
    );
    assert_eq!(core.status(), TimerStatus::OnBreak);
    assert_eq!(
        completion_reason(&at_end),
        Some(SessionTransitionReason::WorkToBreak)
    );
    assert!(at_end.contains(&Effect::EmitAutoRunStopPending));

    apply(
        &mut core,
        Event::TimeObserved {
            credited_break_seconds: 1,
        },
        seconds_after(workday_end(), 1),
    );
    let at_break_end = apply(
        &mut core,
        Event::TimeObserved {
            credited_break_seconds: 1,
        },
        seconds_after(workday_end(), 2),
    );
    assert_eq!(core.status(), TimerStatus::Idle);
    assert_eq!(
        completion_reason(&at_break_end),
        Some(SessionTransitionReason::ScheduledStopExecuted)
    );
}

#[test]
fn an_end_boundary_crossed_while_the_app_was_closed_fires_on_the_first_tick() {
    let mut saved = saved_state(
        TimerStatus::Working,
        default_work_secs(),
        seconds_before(workday_end(), 30 * 60),
    );
    saved.schedule_stop_state = ScheduleStopState::WaitingForScheduleEnd;
    let settings = UserSettings {
        auto_run_feature_enabled: true,
        weekly_auto_run_config: workday(),
        ..UserSettings::default()
    };
    let restarted_at = seconds_after(workday_end(), 5 * 60);
    let (mut core, _) = TimerCore::restore(Some(saved), settings, restarted_at, chrono_tz::UTC);

    let first_tick = apply(
        &mut core,
        Event::TimeObserved {
            credited_break_seconds: 1,
        },
        seconds_after(restarted_at, 1),
    );

    assert!(first_tick.contains(&Effect::EmitAutoRunStopPending));
}

#[test]
fn a_policy_change_keeps_a_pending_scheduled_stop() {
    let mut core = idle_with_schedule(seconds_before(workday_end(), 1), workday());
    start_work(
        &mut core,
        seconds_before(workday_end(), 1),
        Durations {
            work: 30,
            ..Durations::default()
        },
    );
    apply(
        &mut core,
        Event::TimeObserved {
            credited_break_seconds: 1,
        },
        workday_end(),
    );

    let settings = UserSettings {
        post_break_transition_policy: PostBreakTransitionPolicy::StartNextWorkAutomatically,
        ..core.settings.clone()
    };
    apply(
        &mut core,
        Event::SettingsChanged { settings },
        seconds_after(workday_end(), 1),
    );
    let at_work_end = apply(
        &mut core,
        Event::TimeObserved {
            credited_break_seconds: 1,
        },
        seconds_after(workday_end(), 29),
    );

    assert_eq!(
        completion_reason(&at_work_end),
        Some(SessionTransitionReason::ScheduledStopExecuted)
    );
}

#[test]
fn disabling_the_schedule_cancels_a_pending_scheduled_stop() {
    let mut core = idle_with_schedule(seconds_before(workday_end(), 1), workday());
    start_work(
        &mut core,
        seconds_before(workday_end(), 1),
        Durations {
            work: 30,
            ..Durations::default()
        },
    );
    apply(
        &mut core,
        Event::TimeObserved {
            credited_break_seconds: 1,
        },
        workday_end(),
    );

    disable_schedule(&mut core, seconds_after(workday_end(), 1));
    let at_work_end = apply(
        &mut core,
        Event::TimeObserved {
            credited_break_seconds: 1,
        },
        seconds_after(workday_end(), 29),
    );

    assert_eq!(
        completion_reason(&at_work_end),
        Some(SessionTransitionReason::WorkToBreak)
    );
}

#[test]
fn a_restored_scheduled_stop_does_not_end_a_session_while_the_schedule_is_disabled() {
    let saved_at = monday(10, 0, 0);
    let mut saved = saved_state(TimerStatus::Working, 5, saved_at);
    saved.schedule_stop_state = ScheduleStopState::StopAtNextSessionBoundary;
    let mut core = restore_with_default_settings(Some(saved), saved_at);

    let at_work_end = apply(
        &mut core,
        Event::TimeObserved {
            credited_break_seconds: 1,
        },
        seconds_after(saved_at, 5),
    );

    assert_eq!(
        completion_reason(&at_work_end),
        Some(SessionTransitionReason::WorkToBreak)
    );
}

#[test]
fn re_enabling_the_schedule_puts_the_session_back_under_its_control() {
    let mut core = idle_with_schedule(seconds_before(workday_end(), 1), workday());
    start_work(
        &mut core,
        seconds_before(workday_end(), 1),
        Durations {
            work: 30,
            ..Durations::default()
        },
    );
    apply(
        &mut core,
        Event::TimeObserved {
            credited_break_seconds: 1,
        },
        workday_end(),
    );
    disable_schedule(&mut core, seconds_after(workday_end(), 1));

    enable_schedule(&mut core, workday(), seconds_after(workday_end(), 2));
    let at_work_end = apply(
        &mut core,
        Event::TimeObserved {
            credited_break_seconds: 1,
        },
        seconds_after(workday_end(), 29),
    );

    assert_eq!(
        completion_reason(&at_work_end),
        Some(SessionTransitionReason::ScheduledStopExecuted)
    );
}

#[test]
fn saving_settings_between_ticks_does_not_skip_a_boundary_crossed_in_that_gap() {
    let last_tick = millis_before(workday_start(), 500);
    let mut core = idle_with_schedule(last_tick, workday());
    apply(
        &mut core,
        Event::TimeObserved {
            credited_break_seconds: 1,
        },
        last_tick,
    );

    change_durations(
        &mut core,
        Durations {
            work: 600,
            ..Durations::default()
        },
        millis_after(workday_start(), 200),
    );
    apply(
        &mut core,
        Event::TimeObserved {
            credited_break_seconds: 1,
        },
        millis_after(workday_start(), 500),
    );

    assert_eq!(core.status(), TimerStatus::Working);
}
