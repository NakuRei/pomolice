use super::*;

fn startup_with_schedule(now: DateTime<Local>, config: WeeklyAutoRunConfig) -> TimerCore {
    TimerCore::restore(
        None,
        UserSettings {
            auto_run_feature_enabled: true,
            weekly_auto_run_config: config,
            ..UserSettings::default()
        },
        SettingsFileStatus::Ok,
        now,
    )
}

fn monday_and_tuesday() -> WeeklyAutoRunConfig {
    let slot = DayAutoRunSlot::try_new(true, hh_mm(9, 0), hh_mm(18, 0)).expect("valid slot");
    WeeklyAutoRunConfig::try_new(
        slot,
        slot,
        DayAutoRunSlot::default(),
        DayAutoRunSlot::default(),
        DayAutoRunSlot::default(),
        DayAutoRunSlot::default(),
        DayAutoRunSlot::default(),
    )
    .expect("valid schedule")
}

#[test]
fn startup_inside_a_window_offers_it_without_a_frontend_command() {
    let core = startup_with_schedule(monday(10, 0, 0), workday());
    for now in [monday(10, 0, 0), monday(10, 30, 0)] {
        let offer = core
            .full_state_payload(now)
            .boot_auto_start_offer
            .expect("startup offer");
        assert_eq!(offer.anchor_local_date, monday(10, 0, 0).date_naive());
        assert_eq!(offer.window.weekday_key, "mon");
        assert_eq!(core.status(), TimerStatus::Idle);
    }
}

#[test]
fn startup_before_a_window_allows_its_start_without_a_frontend_connection() {
    let mut core = startup_with_schedule(monday(8, 30, 0), workday());
    let effects = apply(&mut core, Event::Tick, workday_start());
    assert!(starts_work(&effects));
    assert!(!boot_prompt_is_offered(&core, workday_start()));
}

#[test]
fn an_unanswered_monday_offer_expires_and_tuesday_starts_automatically() {
    let mut core = startup_with_schedule(monday(10, 0, 0), monday_and_tuesday());
    assert!(boot_prompt_is_offered(&core, monday(10, 0, 0)));
    assert_eq!(core.status(), TimerStatus::Idle);

    let expired = apply(&mut core, Event::Tick, workday_end());
    assert!(expired.contains(&Effect::EmitBootAutoStartOfferClosed));
    assert!(!boot_prompt_is_offered(&core, workday_end()));
    assert_eq!(core.status(), TimerStatus::Idle);

    let tuesday_start = workday_start() + ChronoDuration::days(1);
    let started = apply(&mut core, Event::Tick, tuesday_start);
    assert!(starts_work(&started));
    assert!(!boot_prompt_is_offered(&core, tuesday_start));
}

#[test]
fn jumping_to_the_next_window_does_not_carry_the_offer_forward() {
    let mut core = startup_with_schedule(monday(10, 0, 0), monday_and_tuesday());
    let tuesday_start = workday_start() + ChronoDuration::days(1);
    let effects = apply(&mut core, Event::Tick, tuesday_start);
    assert!(starts_work(&effects));
    assert!(effects.contains(&Effect::EmitBootAutoStartOfferClosed));
    assert!(!boot_prompt_is_offered(&core, tuesday_start));
}

#[test]
fn reading_after_the_deadline_keeps_the_offer_until_an_update_expires_it() {
    let mut core = startup_with_schedule(millis_before(workday_end(), 1), workday());
    assert!(boot_prompt_is_offered(&core, workday_end()));
    assert!(boot_prompt_is_offered(&core, workday_end()));

    let expired = apply(&mut core, Event::Tick, workday_end());
    assert!(expired.contains(&Effect::EmitBootAutoStartOfferClosed));
    assert!(!boot_prompt_is_offered(&core, workday_end()));
    let next_start = workday_start() + ChronoDuration::days(7);
    assert!(starts_work(&apply(&mut core, Event::Tick, next_start)));
}

#[test]
fn answering_later_consumes_the_offer_without_starting_work() {
    let mut core = startup_with_schedule(monday(10, 0, 0), workday());
    let effects = apply(
        &mut core,
        Event::BootPromptResolved { start_now: false },
        monday(10, 1, 0),
    );
    assert!(effects.contains(&Effect::EmitBootAutoStartOfferClosed));
    assert!(!starts_work(&effects));
    assert_eq!(core.status(), TimerStatus::Idle);
    assert!(!boot_prompt_is_offered(&core, monday(10, 2, 0)));
    apply(&mut core, Event::Tick, monday(10, 3, 0));
    assert!(!boot_prompt_is_offered(&core, monday(10, 3, 0)));
}

#[test]
fn accepting_the_offer_starts_work_under_schedule_control() {
    let mut core = startup_with_schedule(seconds_before(workday_end(), 120), workday());
    let effects = apply(
        &mut core,
        Event::BootPromptResolved { start_now: true },
        seconds_before(workday_end(), 90),
    );
    assert!(starts_work(&effects));
    assert!(effects.contains(&Effect::EmitBootAutoStartOfferClosed));
    assert!(!boot_prompt_is_offered(
        &core,
        seconds_before(workday_end(), 89)
    ));
    let at_end = apply(&mut core, Event::Tick, workday_end());
    assert!(at_end.contains(&Effect::EmitAutoRunStopPending));
}

#[test]
fn an_answer_after_expiry_does_not_start_work_even_before_the_next_tick() {
    let mut core = startup_with_schedule(monday(10, 0, 0), workday());
    let effects = apply(
        &mut core,
        Event::BootPromptResolved { start_now: true },
        workday_end(),
    );
    assert!(!starts_work(&effects));
    assert_eq!(core.status(), TimerStatus::Idle);
    assert!(effects.contains(&Effect::EmitBootAutoStartOfferClosed));
    assert!(!boot_prompt_is_offered(&core, workday_end()));
}

#[test]
fn changing_the_target_window_closes_the_offer_and_allows_the_new_start() {
    let mut core = startup_with_schedule(monday(10, 0, 0), workday());
    let changed = enable_schedule(&mut core, monday_slot((11, 0), (18, 0)), monday(10, 30, 0));
    assert!(changed.contains(&Effect::EmitBootAutoStartOfferClosed));
    assert!(!boot_prompt_is_offered(&core, monday(10, 30, 0)));
    assert!(starts_work(&apply(
        &mut core,
        Event::Tick,
        monday(11, 0, 0)
    )));
}

#[test]
fn disabling_and_reenabling_the_schedule_does_not_recreate_an_offer() {
    let mut core = startup_with_schedule(monday(10, 0, 0), workday());
    let disabled = disable_schedule(&mut core, monday(10, 1, 0));
    assert!(disabled.contains(&Effect::EmitBootAutoStartOfferClosed));
    enable_schedule(&mut core, workday(), monday(10, 2, 0));
    assert!(!boot_prompt_is_offered(&core, monday(10, 2, 0)));
}

#[test]
fn unrelated_settings_changes_keep_the_same_offer() {
    let mut core = startup_with_schedule(monday(10, 0, 0), workday());
    change_durations(
        &mut core,
        Durations {
            work: 600,
            ..Durations::default()
        },
        monday(10, 1, 0),
    );
    assert!(boot_prompt_is_offered(&core, monday(10, 1, 0)));
}

#[test]
fn manually_starting_work_consumes_the_offer_and_late_answers_do_nothing() {
    let mut core = startup_with_schedule(monday(10, 0, 0), workday());
    start_work(
        &mut core,
        monday(10, 1, 0),
        Durations {
            work: 600,
            ..Durations::default()
        },
    );
    assert!(!boot_prompt_is_offered(&core, monday(10, 1, 0)));
    let late = apply(
        &mut core,
        Event::BootPromptResolved { start_now: true },
        monday(10, 2, 0),
    );
    assert!(late.is_empty());
    assert_eq!(remaining(&core, monday(10, 2, 0)), 540);
}

#[test]
fn emergency_stop_consumes_the_offer() {
    let mut core = startup_with_schedule(monday(10, 0, 0), workday());
    let stopped = apply(&mut core, Event::EmergencyStopRequested, monday(10, 1, 0));
    assert!(stopped.contains(&Effect::EmitBootAutoStartOfferClosed));
    assert!(!boot_prompt_is_offered(&core, monday(10, 1, 0)));
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
    assert!(!boot_prompt_is_offered(&core, restarted_after_the_boundary));
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
