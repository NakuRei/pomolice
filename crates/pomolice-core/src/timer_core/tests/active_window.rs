use super::*;

#[test]
fn the_active_window_tracks_schedule_boundaries() {
    let before = seconds_before(workday_start(), 1);
    let mut core = idle_with_schedule(before, workday());
    assert!(core.snapshot(before).active_window_summary.is_none());
    apply(
        &mut core,
        Event::TimeObserved {
            credited_break_seconds: 1,
        },
        workday_start(),
    );
    let window = core
        .snapshot(workday_start())
        .active_window_summary
        .expect("active window");
    assert_eq!(window.weekday_key, crate::types::WeekdayKey::Mon);
    assert_eq!(
        core.snapshot(seconds_after(workday_start(), 1))
            .active_window_summary,
        Some(window)
    );
    apply(
        &mut core,
        Event::TimeObserved {
            credited_break_seconds: 1,
        },
        workday_end(),
    );
    assert!(core.snapshot(workday_end()).active_window_summary.is_none());
}

#[test]
fn disabling_the_schedule_immediately_clears_the_active_window() {
    let now = seconds_after(workday_start(), 1);
    let mut core = idle_with_schedule(now, workday());
    assert!(core.snapshot(now).active_window_summary.is_some());
    disable_schedule(&mut core, now);
    assert!(core.snapshot(now).active_window_summary.is_none());
}

#[test]
fn enabling_an_open_schedule_immediately_exposes_the_active_window() {
    let now = seconds_after(workday_start(), 60);
    let mut core = idle_without_schedule(now);
    enable_schedule(&mut core, workday(), now);
    assert_eq!(
        core.snapshot(now)
            .active_window_summary
            .expect("active window")
            .weekday_key,
        crate::types::WeekdayKey::Mon
    );
}

#[test]
fn changing_durations_keeps_the_same_active_window() {
    let now = seconds_after(workday_start(), 1);
    let mut core = idle_with_schedule(now, workday());
    let before = core.snapshot(now).active_window_summary;
    change_durations(
        &mut core,
        Durations {
            work: 600,
            ..Durations::default()
        },
        now,
    );
    assert_eq!(core.snapshot(now).active_window_summary, before);
}
