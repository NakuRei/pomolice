// Active window publication

use super::*;

#[derive(Debug, PartialEq)]
enum PublishedWindow {
    NotPublished,
    NoneActive,
    Weekday(String),
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
                        PublishedWindow::Weekday(window.weekday_key.clone())
                    }),
            ),
            _ => None,
        })
        .unwrap_or(PublishedWindow::NotPublished)
}

#[test]
fn the_active_window_is_published_when_it_changes_and_only_then() {
    let mut core = idle_with_schedule(seconds_before(workday_start(), 1), workday());

    let before_start = apply(&mut core, Event::Tick, seconds_before(workday_start(), 1));
    let at_start = apply(&mut core, Event::Tick, workday_start());
    let still_inside = apply(&mut core, Event::Tick, seconds_after(workday_start(), 1));

    assert_eq!(
        published_active_window(&before_start),
        PublishedWindow::NotPublished
    );
    assert_eq!(
        published_active_window(&at_start),
        PublishedWindow::Weekday("mon".to_string())
    );
    assert_eq!(
        published_active_window(&still_inside),
        PublishedWindow::NotPublished
    );
}

#[test]
fn disabling_the_schedule_tells_the_frontend_no_window_is_active() {
    let mut core = idle_with_schedule(seconds_before(workday_start(), 1), workday());
    apply(&mut core, Event::Tick, workday_start());

    // Whether the change itself or the next tick carries the news is not part
    // of the contract; the frontend must have it by the next tick.
    let mut after_disable = apply(
        &mut core,
        schedule_disabled(),
        seconds_after(workday_start(), 1),
    );
    after_disable.extend(apply(
        &mut core,
        Event::Tick,
        seconds_after(workday_start(), 2),
    ));

    assert_eq!(
        published_active_window(&after_disable),
        PublishedWindow::NoneActive
    );
}
