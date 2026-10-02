// Countdown

use super::*;

#[test]
fn work_remaining_time_follows_the_wall_clock_across_skipped_ticks() {
    let started_at = monday(10, 0, 0);
    let mut core = idle_without_schedule(started_at);
    start_work(&mut core, started_at, Durations::default());

    apply(
        &mut core,
        Event::TimeObserved {
            credited_break_seconds: 1,
        },
        seconds_after(started_at, 3),
    );

    assert_eq!(
        remaining(&core, seconds_after(started_at, 3)),
        default_work_secs() - 3
    );
}

// The application credits one second per tick; a wall-clock gap alone must
// not consume a break.
#[test]
fn a_gap_without_ticks_does_not_consume_break_time_unlike_work_time() {
    let started_at = monday(10, 0, 0);
    let mut core = idle_without_schedule(started_at);
    reach_break_after_one_second_of_work(&mut core, started_at, Durations::default());

    let after_a_gap_without_ticks = seconds_after(started_at, 5);
    apply(
        &mut core,
        Event::TimeObserved {
            credited_break_seconds: 1,
        },
        after_a_gap_without_ticks,
    );

    assert_eq!(
        remaining(&core, after_a_gap_without_ticks),
        default_short_break_secs() - SECONDS_PER_TICK
    );
}

#[test]
fn idle_ticks_change_nothing() {
    let mut core = idle_without_schedule(monday(10, 0, 0));
    let before = core.tick_payload(monday(10, 0, 1));

    let idle_tick = apply(
        &mut core,
        Event::TimeObserved {
            credited_break_seconds: 1,
        },
        monday(10, 0, 1),
    );

    assert!(!starts_work(&idle_tick));
    assert_eq!(core.tick_payload(monday(10, 0, 1)), before);
}
