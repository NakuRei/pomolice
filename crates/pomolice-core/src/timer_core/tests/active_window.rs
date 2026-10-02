// Active window publication

use super::*;

#[derive(Debug, PartialEq)]
enum PublishedWindow {
    NotPublished,
    NoneActive,
    Weekday(crate::types::WeekdayKey),
}

fn published_active_window(effects: &[Effect]) -> PublishedWindow {
    effects
        .iter()
        .find_map(|effect| match effect {
            Effect::EmitActiveWindowChanged(payload) => Some(
                payload
                    .active_window_summary
                    .as_ref()
                    .map_or(PublishedWindow::NoneActive, |window| {
                        PublishedWindow::Weekday(window.weekday_key)
                    }),
            ),
            _ => None,
        })
        .unwrap_or(PublishedWindow::NotPublished)
}

#[test]
fn the_active_window_is_published_when_it_changes_and_only_then() {
    let mut core = idle_with_schedule(seconds_before(workday_start(), 1), workday());

    let before_start = apply(
        &mut core,
        Event::TimeObserved {
            credited_break_seconds: 1,
        },
        seconds_before(workday_start(), 1),
    );
    let at_start = apply(
        &mut core,
        Event::TimeObserved {
            credited_break_seconds: 1,
        },
        workday_start(),
    );
    let still_inside = apply(
        &mut core,
        Event::TimeObserved {
            credited_break_seconds: 1,
        },
        seconds_after(workday_start(), 1),
    );

    assert_eq!(
        published_active_window(&before_start),
        PublishedWindow::NotPublished
    );
    assert_eq!(
        published_active_window(&at_start),
        PublishedWindow::Weekday(crate::types::WeekdayKey::Mon)
    );
    assert_eq!(
        published_active_window(&still_inside),
        PublishedWindow::NotPublished
    );
}

#[test]
fn disabling_the_schedule_tells_the_frontend_at_once_that_no_window_is_active() {
    let mut core = idle_with_schedule(seconds_before(workday_start(), 1), workday());
    apply(
        &mut core,
        Event::TimeObserved {
            credited_break_seconds: 1,
        },
        workday_start(),
    );

    let after_disable = disable_schedule(&mut core, seconds_after(workday_start(), 1));

    assert_eq!(
        published_active_window(&after_disable),
        PublishedWindow::NoneActive
    );
}

#[test]
fn enabling_a_schedule_whose_window_is_open_tells_the_frontend_at_once() {
    let mut core = idle_without_schedule(seconds_after(workday_start(), 60));

    let after_enable = enable_schedule(&mut core, workday(), seconds_after(workday_start(), 61));

    assert_eq!(
        published_active_window(&after_enable),
        PublishedWindow::Weekday(crate::types::WeekdayKey::Mon)
    );
}

#[test]
fn a_settings_change_that_keeps_the_window_publishes_nothing_about_it() {
    let mut core = idle_with_schedule(seconds_before(workday_start(), 1), workday());
    apply(
        &mut core,
        Event::TimeObserved {
            credited_break_seconds: 1,
        },
        workday_start(),
    );

    let after_duration_change = change_durations(
        &mut core,
        Durations {
            work: 600,
            ..Durations::default()
        },
        seconds_after(workday_start(), 1),
    );

    assert_eq!(
        published_active_window(&after_duration_change),
        PublishedWindow::NotPublished
    );
}
