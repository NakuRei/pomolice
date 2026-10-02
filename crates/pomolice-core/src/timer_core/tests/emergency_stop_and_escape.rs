// Emergency stop and escape combo

use super::*;

#[test]
fn emergency_stop_from_a_fullscreen_break_releases_the_lock_and_reports_idle() {
    let started_at = monday(10, 0, 0);
    let mut core = idle_without_schedule(started_at);
    reach_break_after_one_second_of_work(&mut core, started_at, Durations::default());

    let stopped = apply(
        &mut core,
        Event::EmergencyStopRequested,
        seconds_after(started_at, 2),
    );

    assert_eq!(core.status(), TimerStatus::Idle);
    assert!(stopped.contains(&Effect::SetBreakLockTarget(BreakLockTarget::Unlocked)));

    assert_eq!(
        core.snapshot(seconds_after(started_at, 2)).status,
        TimerStatus::Idle
    );
}

#[test]
fn a_duplicate_emergency_stop_while_idle_is_harmless() {
    let mut core = idle_without_schedule(monday(10, 0, 0));

    let stopped = apply(&mut core, Event::EmergencyStopRequested, monday(10, 0, 1));

    assert_eq!(core.status(), TimerStatus::Idle);
    assert!(stopped.contains(&Effect::SetBreakLockTarget(BreakLockTarget::Unlocked)));
    assert!(completion_reason(&stopped).is_none());
}

#[test]
fn the_escape_combo_outside_a_break_releases_nothing() {
    let mut core = idle_without_schedule(monday(10, 0, 0));

    let escaped = apply(&mut core, Event::EscapeComboPressed, monday(10, 0, 0));

    assert!(!escaped.contains(&Effect::SetBreakLockTarget(BreakLockTarget::Unlocked)));
    assert_eq!(core.status(), TimerStatus::Idle);
}

#[test]
fn the_escape_combo_releases_the_lock_and_the_break_continues_without_fullscreen() {
    let started_at = monday(10, 0, 0);
    let mut core = idle_without_schedule(started_at);
    reach_break_after_one_second_of_work(&mut core, started_at, Durations::default());

    let escaped = apply(
        &mut core,
        Event::EscapeComboPressed,
        seconds_after(started_at, 2),
    );
    assert!(escaped.contains(&Effect::SetBreakLockTarget(BreakLockTarget::Unlocked)));

    assert_eq!(core.status(), TimerStatus::OnBreak);
}
