use chrono::{TimeZone, Utc};
use chrono_tz::{America::New_York, Asia::Tokyo};
use pomolice_core::error::TimerError;
use pomolice_core::settings::{input::SettingsInput, UserSettings};
use pomolice_core::timer_core::{BreakLockTarget, Effect, Event, TimerCore};
use pomolice_core::types::TimerStatus;

#[test]
fn restoring_expired_work_returns_a_ready_break_and_its_initial_effects() {
    let start = Utc
        .with_ymd_and_hms(2025, 1, 6, 1, 0, 0)
        .single()
        .expect("valid time");
    let settings = UserSettings::try_from(SettingsInput {
        work_duration_secs: 1,
        short_break_duration_secs: 10,
        ..UserSettings::default().into()
    })
    .expect("valid settings");
    let (mut timer, initial_effects) = TimerCore::restore(None, settings.clone(), start, Tokyo);
    assert!(initial_effects.is_empty());
    timer
        .apply(Event::StartWorkRequested, start)
        .expect("start");
    let restart_at = start + chrono::Duration::minutes(30);
    let (mut restored, effects) =
        TimerCore::restore(Some(timer.checkpoint(start)), settings, restart_at, Tokyo);

    assert_eq!(restored.snapshot(restart_at).status, TimerStatus::OnBreak);
    assert_eq!(restored.snapshot(restart_at).remaining_seconds, 10);
    assert_eq!(restored.snapshot(restart_at).completed_pomodoros, 1);
    assert!(effects.contains(&Effect::SetBreakLockTarget(BreakLockTarget::Locked)));
    assert!(effects.contains(&Effect::Persist));
    for _ in 0..2 {
        let effects = restored
            .apply(
                Event::TimeObserved {
                    credited_break_seconds: 0,
                },
                restart_at,
            )
            .expect("observe restored break");
        assert!(!effects.iter().any(|effect| matches!(
            effect,
            Effect::SetBreakLockTarget(_) | Effect::EmitSessionCompleted(_)
        )));
    }
    restored
        .apply(
            Event::TimeObserved {
                credited_break_seconds: 1,
            },
            restart_at,
        )
        .expect("credit restored break");
    assert_eq!(restored.snapshot(restart_at).remaining_seconds, 9);
    assert_eq!(restored.snapshot(restart_at).completed_pomodoros, 1);
}

#[test]
fn wall_time_and_credited_break_seconds_are_independent() {
    let start = Utc
        .with_ymd_and_hms(2025, 1, 6, 1, 0, 0)
        .single()
        .expect("valid time");
    let input = SettingsInput {
        work_duration_secs: 1,
        short_break_duration_secs: 10,
        ..UserSettings::default().into()
    };
    let (mut timer, _) = TimerCore::restore(
        None,
        UserSettings::try_from(input).expect("valid settings"),
        start,
        Tokyo,
    );
    timer
        .apply(Event::StartWorkRequested, start)
        .expect("start");
    let later = start + chrono::Duration::seconds(60);
    timer
        .apply(
            Event::TimeObserved {
                credited_break_seconds: 0,
            },
            later,
        )
        .expect("advance");
    assert_eq!(timer.snapshot(later).status, TimerStatus::OnBreak);
    assert_eq!(timer.snapshot(later).remaining_seconds, 10);
    for _ in 0..2 {
        timer
            .apply(
                Event::TimeObserved {
                    credited_break_seconds: 0,
                },
                later,
            )
            .expect("reevaluate");
    }
    assert_eq!(timer.snapshot(later).remaining_seconds, 10);
    timer
        .apply(
            Event::TimeObserved {
                credited_break_seconds: 3,
            },
            later,
        )
        .expect("credit break");
    assert_eq!(timer.snapshot(later).remaining_seconds, 7);
    timer
        .apply(
            Event::TimeObserved {
                credited_break_seconds: u64::MAX,
            },
            later,
        )
        .expect("complete the break without underflow");
    assert_eq!(timer.snapshot(later).status, TimerStatus::Idle);
    assert!(timer.snapshot(later).post_break_decision_pending);
}

#[test]
fn a_dst_schedule_is_replayable_and_rollback_does_not_restart_it() {
    let start = New_York
        .with_ymd_and_hms(2025, 3, 9, 1, 59, 59)
        .single()
        .expect("before DST")
        .with_timezone(&Utc);
    let mut input = SettingsInput::from(UserSettings::default());
    input.auto_run_feature_enabled = true;
    input.weekly_auto_run_config.sun.enabled = true;
    input.weekly_auto_run_config.sun.start_time = "02:30".into();
    input.weekly_auto_run_config.sun.end_time = "04:00".into();
    let settings = UserSettings::try_from(input).expect("valid schedule");
    let replay = || {
        let (mut timer, _) = TimerCore::restore(None, settings.clone(), start, New_York);
        let after = start + chrono::Duration::seconds(2);
        let effects = timer
            .apply(
                Event::TimeObserved {
                    credited_break_seconds: 0,
                },
                after,
            )
            .expect("cross DST gap");
        assert_eq!(timer.snapshot(after).status, TimerStatus::Working);
        assert!(effects.contains(&Effect::Persist));
        timer
            .apply(Event::EmergencyStopRequested, after)
            .expect("stop");
        timer
            .apply(
                Event::TimeObserved {
                    credited_break_seconds: 0,
                },
                start,
            )
            .expect("rollback");
        timer
            .apply(
                Event::TimeObserved {
                    credited_break_seconds: 0,
                },
                after,
            )
            .expect("same boundary");
        assert_eq!(timer.snapshot(after).status, TimerStatus::Idle);
        (effects, timer.snapshot(after))
    };
    assert_eq!(replay(), replay());
}

#[test]
fn rejected_operations_leave_the_observable_state_unchanged() {
    let now = Utc
        .with_ymd_and_hms(2025, 1, 6, 1, 0, 0)
        .single()
        .expect("valid time");
    let (mut timer, _) = TimerCore::restore(None, UserSettings::default(), now, Tokyo);
    timer.apply(Event::StartWorkRequested, now).expect("start");
    let before = timer.snapshot(now);
    assert!(matches!(
        timer.apply(Event::StartWorkRequested, now),
        Err(TimerError::InvalidStateTransition {
            current_state: TimerStatus::Working,
            ..
        })
    ));
    assert_eq!(timer.snapshot(now), before);
    let invalid = SettingsInput {
        work_duration_secs: 0,
        ..timer.settings().clone().into()
    };
    assert!(UserSettings::try_from(invalid).is_err());
    assert_eq!(timer.snapshot(now), before);
}

#[test]
fn a_checkpoint_resumes_work_using_the_supplied_timezone_and_instant() {
    let start = Tokyo
        .with_ymd_and_hms(2025, 1, 6, 9, 0, 0)
        .single()
        .expect("valid Tokyo time")
        .with_timezone(&Utc);
    let mut input = SettingsInput::from(UserSettings::default());
    input.auto_run_feature_enabled = true;
    input.weekly_auto_run_config.mon.enabled = true;
    input.weekly_auto_run_config.mon.start_time = "09:00".into();
    input.weekly_auto_run_config.mon.end_time = "18:00".into();
    let settings = UserSettings::try_from(input).expect("valid schedule");
    let (mut timer, _) = TimerCore::restore(None, settings.clone(), start, Tokyo);
    timer
        .apply(Event::StartWorkRequested, start)
        .expect("start");
    let saved_at = start + chrono::Duration::minutes(5);
    let checkpoint = timer.checkpoint(saved_at);
    let restart_at = saved_at + chrono::Duration::minutes(2);
    let (restored, startup_effects) = TimerCore::restore(
        Some(checkpoint.clone()),
        settings.clone(),
        restart_at,
        Tokyo,
    );
    assert!(startup_effects.iter().any(|effect| matches!(effect, Effect::PublishTimerState(state) if state.remaining_seconds == 18 * 60)));
    assert_eq!(restored.snapshot(restart_at).status, TimerStatus::Working);
    assert_eq!(restored.snapshot(restart_at).remaining_seconds, 18 * 60);
    assert!(restored
        .snapshot(restart_at)
        .active_window_summary
        .is_some());

    let (elsewhere, _) = TimerCore::restore(Some(checkpoint), settings, restart_at, New_York);
    assert_eq!(elsewhere.snapshot(restart_at).remaining_seconds, 18 * 60);
    assert!(elsewhere
        .snapshot(restart_at)
        .active_window_summary
        .is_none());
}
