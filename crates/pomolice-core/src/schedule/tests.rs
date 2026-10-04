use super::*;

fn enabled_slot(start_time: &str, end_time: &str) -> DayAutoRunSlot {
    DayAutoRunSlot::try_new(
        true,
        HhMm::try_from(start_time.to_string()).expect("start time should be valid"),
        HhMm::try_from(end_time.to_string()).expect("end time should be valid"),
    )
    .expect("slot should be valid")
}

fn disabled_slot(start_time: &str, end_time: &str) -> DayAutoRunSlot {
    DayAutoRunSlot::try_new(
        false,
        HhMm::try_from(start_time.to_string()).expect("start time should be valid"),
        HhMm::try_from(end_time.to_string()).expect("end time should be valid"),
    )
    .expect("slot should be valid")
}

#[test]
fn test_hh_mm_accepts_valid_zero_padded_time() {
    let value = HhMm::try_from("08:05".to_string()).expect("HH:MM should parse");
    assert_eq!(value.hour(), 8);
    assert_eq!(value.minute(), 5);
    assert_eq!(value.minutes_since_midnight(), 485);
    assert_eq!(value.to_string(), "08:05");
}

#[test]
fn test_hh_mm_rejects_malformed_value() {
    let result = HhMm::try_from("8:5".to_string());
    assert!(result.is_err());
}

#[test]
fn test_day_auto_run_slot_default() {
    let slot = DayAutoRunSlot::default();
    assert!(!slot.enabled());
    assert_eq!(slot.start_time().to_string(), "09:00");
    assert_eq!(slot.end_time().to_string(), "18:00");
    assert!(!slot.crosses_midnight());
}

#[test]
fn test_day_auto_run_slot_try_new_rejects_equal_start_and_end() {
    let start_time = HhMm::try_new(9, 0).expect("start time should be valid");
    let end_time = HhMm::try_new(9, 0).expect("end time should be valid");

    let result = DayAutoRunSlot::try_new(true, start_time, end_time);
    assert_eq!(result, Err(EmptyTimeRangeError));
}

#[test]
fn test_day_auto_run_slot_try_new_accepts_equal_times_on_disabled_slot() {
    let time = HhMm::try_new(9, 0).expect("time should be valid");

    assert!(DayAutoRunSlot::try_new(false, time, time).is_ok());
}

#[test]
fn test_weekly_auto_run_config_default_all_days_disabled() {
    let config = WeeklyAutoRunConfig::default();
    assert!(!config.mon().enabled());
    assert!(!config.tue().enabled());
    assert!(!config.wed().enabled());
    assert!(!config.thu().enabled());
    assert!(!config.fri().enabled());
    assert!(!config.sat().enabled());
    assert!(!config.sun().enabled());
}

#[test]
fn test_weekly_auto_run_config_try_new_rejects_overlapping_enabled_windows() {
    let result = WeeklyAutoRunConfig::try_new(
        enabled_slot("22:00", "02:00"),
        enabled_slot("01:00", "05:00"),
        DayAutoRunSlot::default(),
        DayAutoRunSlot::default(),
        DayAutoRunSlot::default(),
        DayAutoRunSlot::default(),
        DayAutoRunSlot::default(),
    );

    assert_eq!(
        result,
        Err(OverlappingSlotsError(vec![(
            WeekdayKey::Mon,
            WeekdayKey::Tue
        )]))
    );
}

#[test]
fn test_weekly_auto_run_config_try_new_reports_every_overlapping_pair() {
    let result = WeeklyAutoRunConfig::try_new(
        enabled_slot("00:00", "18:00"),
        DayAutoRunSlot::default(),
        DayAutoRunSlot::default(),
        DayAutoRunSlot::default(),
        DayAutoRunSlot::default(),
        enabled_slot("22:00", "02:00"),
        enabled_slot("01:00", "00:30"),
    );

    assert_eq!(
        result,
        Err(OverlappingSlotsError(vec![
            (WeekdayKey::Mon, WeekdayKey::Sun),
            (WeekdayKey::Sat, WeekdayKey::Sun),
        ]))
    );
}

#[test]
fn test_weekly_auto_run_config_rejects_sunday_overnight_overlap_with_monday() {
    let result = WeeklyAutoRunConfig::try_new(
        enabled_slot("01:00", "05:00"),
        DayAutoRunSlot::default(),
        DayAutoRunSlot::default(),
        DayAutoRunSlot::default(),
        DayAutoRunSlot::default(),
        DayAutoRunSlot::default(),
        enabled_slot("22:00", "02:00"),
    );

    assert_eq!(
        result,
        Err(OverlappingSlotsError(vec![(
            WeekdayKey::Mon,
            WeekdayKey::Sun
        )]))
    );
}

#[test]
fn test_weekly_auto_run_config_try_new_accepts_non_overlapping_enabled_weekday_slots() {
    let result = WeeklyAutoRunConfig::try_new(
        enabled_slot("09:00", "12:00"),
        enabled_slot("13:00", "18:00"),
        DayAutoRunSlot::default(),
        DayAutoRunSlot::default(),
        DayAutoRunSlot::default(),
        DayAutoRunSlot::default(),
        DayAutoRunSlot::default(),
    );

    assert!(result.is_ok());
}

#[test]
fn test_weekly_auto_run_config_allows_sunday_overnight_adjacent_to_monday() {
    let result = WeeklyAutoRunConfig::try_new(
        enabled_slot("02:00", "05:00"),
        DayAutoRunSlot::default(),
        DayAutoRunSlot::default(),
        DayAutoRunSlot::default(),
        DayAutoRunSlot::default(),
        DayAutoRunSlot::default(),
        enabled_slot("22:00", "02:00"),
    );

    assert!(result.is_ok());
}

#[test]
fn test_weekly_auto_run_config_ignores_disabled_monday_during_sunday_wrap_overlap_check() {
    let result = WeeklyAutoRunConfig::try_new(
        disabled_slot("01:00", "05:00"),
        DayAutoRunSlot::default(),
        DayAutoRunSlot::default(),
        DayAutoRunSlot::default(),
        DayAutoRunSlot::default(),
        DayAutoRunSlot::default(),
        enabled_slot("22:00", "02:00"),
    );

    assert!(result.is_ok());
}
