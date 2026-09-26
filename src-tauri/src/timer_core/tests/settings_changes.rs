// Settings saved while the timer is running

use super::*;

#[test]
fn saving_durations_while_working_keeps_the_running_session_and_applies_to_the_next() {
    let started_at = monday(10, 0, 0);
    let mut core = idle_without_schedule(started_at);
    start_work(&mut core, started_at, Durations::default());
    let saved_at = seconds_after(started_at, 60);

    change_durations(
        &mut core,
        Durations {
            work: 600,
            ..Durations::default()
        },
        saved_at,
    );

    let running = core.tick_payload(saved_at);
    assert_eq!(running.status, TimerStatus::Working);
    assert_eq!(running.remaining_seconds, default_work_secs() - 60);
    assert_eq!(running.session_duration_secs, default_work_secs());

    apply(&mut core, Event::EmergencyStopRequested, saved_at);
    let next_start = seconds_after(saved_at, 1);
    apply(&mut core, Event::StartWorkRequested, next_start);
    let next = core.tick_payload(next_start);
    assert_eq!(next.remaining_seconds, 600);
    assert_eq!(next.session_duration_secs, 600);
}

#[test]
fn a_shorter_break_saved_during_a_break_does_not_shorten_the_penalty_reset() {
    let started_at = monday(10, 0, 0);
    let mut core = idle_without_schedule(started_at);
    reach_break_after_one_second_of_work(&mut core, started_at, Durations::default());
    let during_the_break = seconds_after(started_at, 2);
    change_durations(
        &mut core,
        Durations {
            short_break: 1,
            ..Durations::default()
        },
        during_the_break,
    );

    apply(&mut core, Event::InputDetected, during_the_break);

    let penalized = core.tick_payload(during_the_break);
    assert_eq!(penalized.remaining_seconds, default_short_break_secs());
    assert_eq!(penalized.session_duration_secs, default_short_break_secs());
}

#[test]
fn lowering_the_interval_below_the_current_cycle_makes_the_next_break_long() {
    let saved_at = monday(10, 0, 0);
    let mut saved = saved_state(TimerStatus::Working, 5, saved_at);
    saved.current_cycle = 3;
    let mut core = restore_with_default_settings(Some(saved), saved_at);
    change_durations(
        &mut core,
        Durations {
            long_break_interval: 2,
            ..Durations::default()
        },
        seconds_after(saved_at, 1),
    );

    apply(&mut core, Event::Tick, seconds_after(saved_at, 5));

    assert_eq!(
        core.tick_payload(seconds_after(saved_at, 5)).session_type,
        SessionType::LongBreak
    );
}

#[test]
fn an_unusable_settings_file_is_reported_until_settings_are_saved() {
    let now = monday(10, 0, 0);
    let mut core = TimerCore::restore(
        None,
        UserSettings::default(),
        SettingsFileStatus::Unusable,
        now,
    );
    assert_eq!(
        core.full_state_payload(now).settings_file_status,
        SettingsFileStatus::Unusable
    );

    change_durations(&mut core, Durations::default(), now);

    assert_eq!(
        core.full_state_payload(now).settings_file_status,
        SettingsFileStatus::Ok
    );
}
