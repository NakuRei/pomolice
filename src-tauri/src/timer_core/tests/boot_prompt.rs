// Boot prompt

use super::*;

#[test]
fn boot_prompt_resolution_before_the_first_full_state_is_ignored() {
    let mut core = idle_without_schedule(monday(9, 30, 0));
    enable_schedule(&mut core, workday(), monday(9, 30, 0));

    let answered = core.apply(
        Event::BootPromptResolved { start_now: true },
        monday(9, 30, 0),
    );

    // Nothing has been offered yet, so there is nothing to answer: the answer
    // is dropped without an error. Compare the stale answer below, which
    // arrives after a session has started and is rejected.
    let effects = answered.expect("an answer to a prompt not yet offered is accepted");
    assert!(!starts_work(&effects));
    assert_eq!(core.status(), TimerStatus::Idle);
    apply(&mut core, Event::FirstFullStateObserved, monday(9, 30, 1));
    assert!(boot_prompt_is_offered(&core, monday(9, 30, 1)));
}

#[test]
fn the_first_full_state_inside_the_window_offers_the_boot_prompt_until_answered() {
    let mut core = idle_without_schedule(monday(9, 30, 0));
    enable_schedule(&mut core, workday(), monday(9, 30, 0));

    apply(&mut core, Event::FirstFullStateObserved, monday(9, 30, 0));

    let offered_window = core
        .full_state_payload(monday(9, 30, 5))
        .boot_auto_start_prompt_window
        .map(|window| window.weekday_key);
    assert_eq!(offered_window, Some("mon".to_string()));

    apply(
        &mut core,
        Event::BootPromptResolved { start_now: false },
        monday(9, 30, 6),
    );
    assert!(!boot_prompt_is_offered(&core, monday(9, 30, 7)));
}

#[test]
fn the_first_full_state_outside_the_window_offers_no_prompt() {
    let mut core = idle_without_schedule(monday(8, 0, 0));
    enable_schedule(&mut core, workday(), monday(8, 0, 0));

    apply(&mut core, Event::FirstFullStateObserved, monday(8, 0, 0));

    assert!(!boot_prompt_is_offered(&core, monday(8, 0, 0)));
}

#[test]
fn scheduled_starts_wait_while_the_boot_prompt_is_pending() {
    let mut core = idle_without_schedule(monday(9, 30, 0));
    enable_schedule(&mut core, workday(), monday(9, 30, 0));
    apply(&mut core, Event::FirstFullStateObserved, monday(9, 30, 0));
    assert!(boot_prompt_is_offered(&core, monday(9, 30, 0)));

    enable_schedule(&mut core, monday_slot((9, 45), (18, 0)), monday(9, 40, 0));
    let at_new_start = apply(&mut core, Event::Tick, monday(9, 45, 0));

    assert_eq!(core.status(), TimerStatus::Idle);
    assert!(!starts_work(&at_new_start));
}

#[test]
fn answering_the_boot_prompt_with_later_keeps_idle_and_the_next_boundary_still_fires() {
    let mut core = idle_without_schedule(monday(9, 30, 0));
    enable_schedule(&mut core, workday(), monday(9, 30, 0));
    apply(&mut core, Event::FirstFullStateObserved, monday(9, 30, 0));

    let answered = apply(
        &mut core,
        Event::BootPromptResolved { start_now: false },
        monday(9, 31, 0),
    );
    assert!(!starts_work(&answered));
    assert_eq!(core.status(), TimerStatus::Idle);

    enable_schedule(&mut core, monday_slot((9, 45), (18, 0)), monday(9, 40, 0));
    let at_new_start = apply(&mut core, Event::Tick, monday(9, 45, 0));
    assert!(starts_work(&at_new_start));
}

#[test]
fn answering_the_boot_prompt_with_start_begins_work_under_schedule_control() {
    let mut core = idle_without_schedule(seconds_before(workday_end(), 120));
    enable_schedule(&mut core, workday(), seconds_before(workday_end(), 120));
    apply(
        &mut core,
        Event::FirstFullStateObserved,
        seconds_before(workday_end(), 120),
    );

    let answered = apply(
        &mut core,
        Event::BootPromptResolved { start_now: true },
        seconds_before(workday_end(), 90),
    );

    assert!(starts_work(&answered));
    assert!(!boot_prompt_is_offered(
        &core,
        seconds_before(workday_end(), 89)
    ));
    let at_end = apply(&mut core, Event::Tick, workday_end());
    assert!(at_end.contains(&Effect::EmitAutoRunStopPending));
}

#[test]
fn a_stale_boot_prompt_resolution_does_not_replace_a_running_session() {
    let mut core = idle_without_schedule(monday(9, 30, 0));
    enable_schedule(&mut core, workday(), monday(9, 30, 0));
    apply(&mut core, Event::FirstFullStateObserved, monday(9, 30, 0));
    start_work(
        &mut core,
        monday(9, 31, 0),
        Durations {
            work: 600,
            ..Durations::default()
        },
    );

    let answered = core.apply(
        Event::BootPromptResolved { start_now: true },
        monday(9, 32, 0),
    );

    assert!(matches!(
        answered,
        Err(PomoliceError::InvalidStateTransition { .. })
    ));
    assert_eq!(remaining(&core, monday(9, 32, 0)), 600 - 60);
}

/// A work session saved just before the 09:00 start boundary, restored after
/// it, whose work already ran out while the app was closed and whose break
/// lasts one second. Returns the core once that break has ended.
fn restored_session_that_ends_after_the_start_boundary() -> (TimerCore, Vec<Effect>) {
    let saved_before_the_boundary = seconds_before(workday_start(), 10);
    let restarted_after_the_boundary = seconds_after(workday_start(), 30);
    let saved = saved_state(TimerStatus::Working, 5, saved_before_the_boundary);
    let settings = UserSettings {
        auto_run_feature_enabled: true,
        weekly_auto_run_config: workday(),
        ..with_durations(
            UserSettings::default(),
            Durations {
                work: 5,
                short_break: 1,
                ..Durations::default()
            },
        )
    };
    let mut core = TimerCore::restore(
        Some(saved),
        settings,
        SettingsFileStatus::Ok,
        restarted_after_the_boundary,
    );
    apply(&mut core, Event::Started, restarted_after_the_boundary);
    assert_eq!(core.status(), TimerStatus::OnBreak);
    dismiss_boot_prompt(&mut core, restarted_after_the_boundary);
    apply(
        &mut core,
        Event::BreakLockEntered,
        restarted_after_the_boundary,
    );

    let at_break_end = apply(
        &mut core,
        Event::Tick,
        seconds_after(restarted_after_the_boundary, 1),
    );
    assert_eq!(core.status(), TimerStatus::Idle);
    (core, at_break_end)
}

#[test]
fn a_start_boundary_that_passed_while_the_app_was_closed_does_not_restart_a_restored_session_once_it_ends(
) {
    let (_, at_break_end) = restored_session_that_ends_after_the_start_boundary();

    assert!(!starts_work(&at_break_end));
}

#[test]
fn a_later_start_boundary_still_fires_after_a_restored_session_has_ended() {
    let (mut core, _) = restored_session_that_ends_after_the_start_boundary();

    enable_schedule(
        &mut core,
        monday_slot((9, 2), (18, 0)),
        seconds_after(workday_start(), 60),
    );
    let at_next_boundary = apply(
        &mut core,
        Event::Tick,
        seconds_after(workday_start(), 2 * 60),
    );

    assert!(starts_work(&at_next_boundary));
}
