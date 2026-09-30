use serde_json::json;

use super::*;
use crate::settings::input::{SettingChange, SettingsChanges};

fn day_slot(enabled: bool, start_time: &str, end_time: &str) -> DaySlotInput {
    DaySlotInput {
        enabled,
        start_time: start_time.to_string(),
        end_time: end_time.to_string(),
    }
}

fn disabled_day() -> DaySlotInput {
    day_slot(false, "09:00", "18:00")
}

fn valid_input() -> SettingsInput {
    SettingsInput {
        work_duration_secs: 30 * 60,
        short_break_duration_secs: 4 * 60,
        long_break_duration_secs: 20 * 60,
        long_break_interval: 3,
        post_break_transition_policy: PostBreakTransitionPolicy::StartNextWorkAutomatically,
        auto_run_feature_enabled: true,
        weekly_auto_run_config: WeeklyScheduleInput {
            mon: day_slot(true, "09:00", "18:00"),
            tue: disabled_day(),
            wed: disabled_day(),
            thu: disabled_day(),
            fri: day_slot(true, "22:00", "02:00"),
            sat: disabled_day(),
            sun: disabled_day(),
        },
        notification_sound_volume_percent: 70,
        notification_sound_muted: true,
    }
}

fn issues_of(input: SettingsInput) -> Vec<SettingsValidationIssue> {
    match UserSettings::try_from(input) {
        Ok(settings) => panic!("input should be rejected, got {settings:?}"),
        Err(errors) => errors.issues().to_vec(),
    }
}

#[test]
fn defaults_are_a_standard_pomodoro_with_automation_off() {
    let defaults = UserSettings::default();

    assert_eq!(defaults.work_duration_secs.get(), 25 * 60);
    assert_eq!(defaults.short_break_duration_secs.get(), 5 * 60);
    assert_eq!(defaults.long_break_duration_secs.get(), 15 * 60);
    assert_eq!(defaults.long_break_interval.get(), 4);
    assert_eq!(
        defaults.post_break_transition_policy,
        PostBreakTransitionPolicy::RequireManualContinue
    );
    assert!(!defaults.auto_run_feature_enabled);
    assert_eq!(
        defaults.weekly_auto_run_config,
        WeeklyAutoRunConfig::default()
    );
    assert_eq!(defaults.notification_sound_volume_percent.get(), 50);
    assert!(!defaults.notification_sound_muted);
}

#[test]
fn valid_input_is_accepted_unchanged() {
    let settings = UserSettings::try_from(valid_input()).expect("input should be valid");

    assert_eq!(SettingsInput::from(settings), valid_input());
}

#[test]
fn out_of_range_values_name_the_field_and_its_allowed_range() {
    let input = SettingsInput {
        work_duration_secs: 7_201,
        short_break_duration_secs: 0,
        long_break_duration_secs: 3_601,
        long_break_interval: 1,
        notification_sound_volume_percent: 101,
        ..valid_input()
    };

    assert_eq!(
        issues_of(input),
        vec![
            SettingsValidationIssue::OutOfRange {
                field: RangedSettingField::WorkDurationSecs,
                min: 1,
                max: 7_200,
            },
            SettingsValidationIssue::OutOfRange {
                field: RangedSettingField::ShortBreakDurationSecs,
                min: 1,
                max: 3_600,
            },
            SettingsValidationIssue::OutOfRange {
                field: RangedSettingField::LongBreakDurationSecs,
                min: 1,
                max: 3_600,
            },
            SettingsValidationIssue::OutOfRange {
                field: RangedSettingField::LongBreakInterval,
                min: 2,
                max: 10,
            },
            SettingsValidationIssue::OutOfRange {
                field: RangedSettingField::NotificationSoundVolumePercent,
                min: 0,
                max: 100,
            },
        ]
    );
}

#[test]
fn unusable_times_are_reported_per_weekday_and_boundary() {
    let mut input = valid_input();
    input.weekly_auto_run_config.tue = day_slot(true, "24:00", "18:00");
    input.weekly_auto_run_config.wed = day_slot(false, "09:00", "");

    assert_eq!(
        issues_of(input),
        vec![
            SettingsValidationIssue::InvalidTime {
                weekday: WeekdayKey::Tue,
                boundary: TimeBoundary::Start,
            },
            SettingsValidationIssue::InvalidTime {
                weekday: WeekdayKey::Wed,
                boundary: TimeBoundary::End,
            },
        ]
    );
}

#[test]
fn an_enabled_day_needs_different_start_and_end_times() {
    let mut input = valid_input();
    input.weekly_auto_run_config.tue = day_slot(true, "12:00", "12:00");

    assert_eq!(
        issues_of(input),
        vec![SettingsValidationIssue::EmptyTimeRange {
            weekday: WeekdayKey::Tue
        }]
    );
}

#[test]
fn a_disabled_day_may_keep_equal_times() {
    let mut input = valid_input();
    input.weekly_auto_run_config.tue = day_slot(false, "12:00", "12:00");

    assert!(UserSettings::try_from(input).is_ok());
}

#[test]
fn every_overlapping_pair_of_enabled_days_is_reported() {
    let mut input = valid_input();
    input.weekly_auto_run_config.sat = day_slot(true, "01:00", "03:00");
    input.weekly_auto_run_config.sun = day_slot(true, "23:00", "10:00");

    assert_eq!(
        issues_of(input),
        vec![
            SettingsValidationIssue::OverlappingTimeRanges {
                primary_weekday: WeekdayKey::Mon,
                secondary_weekday: WeekdayKey::Sun,
            },
            SettingsValidationIssue::OverlappingTimeRanges {
                primary_weekday: WeekdayKey::Fri,
                secondary_weekday: WeekdayKey::Sat,
            },
        ]
    );
}

#[test]
fn a_day_with_an_unusable_time_is_left_out_of_the_overlap_check() {
    let mut input = valid_input();
    input.weekly_auto_run_config.tue = day_slot(true, "10:00", "10:00");
    input.weekly_auto_run_config.sat = day_slot(true, "01:00", "03:00");

    assert_eq!(
        issues_of(input),
        vec![
            SettingsValidationIssue::EmptyTimeRange {
                weekday: WeekdayKey::Tue
            },
            SettingsValidationIssue::OverlappingTimeRanges {
                primary_weekday: WeekdayKey::Fri,
                secondary_weekday: WeekdayKey::Sat,
            },
        ]
    );
}

#[test]
fn issues_serialize_to_the_public_contract() {
    let issues = vec![
        SettingsValidationIssue::OutOfRange {
            field: RangedSettingField::LongBreakInterval,
            min: 2,
            max: 10,
        },
        SettingsValidationIssue::InvalidTime {
            weekday: WeekdayKey::Mon,
            boundary: TimeBoundary::End,
        },
        SettingsValidationIssue::EmptyTimeRange {
            weekday: WeekdayKey::Wed,
        },
        SettingsValidationIssue::OverlappingTimeRanges {
            primary_weekday: WeekdayKey::Sat,
            secondary_weekday: WeekdayKey::Sun,
        },
    ];

    assert_eq!(
        serde_json::to_value(issues).expect("issues should serialize"),
        json!([
            { "code": "out_of_range", "field": "long_break_interval", "min": 2, "max": 10 },
            { "code": "invalid_time", "weekday": "mon", "boundary": "end" },
            { "code": "empty_time_range", "weekday": "wed" },
            { "code": "overlapping_time_ranges", "primary_weekday": "sat", "secondary_weekday": "sun" },
        ])
    );
}

#[test]
fn settings_round_trip_through_the_file_format() {
    let persisted = PersistedSettings {
        schema_version: SETTINGS_SCHEMA_VERSION,
        settings: UserSettings::try_from(valid_input()).expect("input should be valid"),
    };

    let json = serde_json::to_value(&persisted).expect("settings should serialize");
    let restored: PersistedSettings =
        serde_json::from_value(json.clone()).expect("settings should deserialize");

    assert_eq!(restored, persisted);
    assert_eq!(
        json["settings"]["weekly_auto_run_config"]["fri"]["start_time"],
        "22:00"
    );
}

#[test]
fn a_file_with_a_rule_violation_is_rejected_by_the_same_rules() {
    let mut settings_json =
        serde_json::to_value(SettingsInput::from(UserSettings::default())).expect("serialize");
    settings_json["long_break_interval"] = json!(1);

    assert!(serde_json::from_value::<UserSettings>(settings_json).is_err());
}

#[test]
fn changes_replace_only_the_fields_they_name() {
    let changes: SettingsChanges =
        serde_json::from_value(json!({ "notification_sound_muted": true }))
            .expect("changes should deserialize");

    let applied = changes.applied_to(valid_input());

    assert_eq!(
        applied,
        SettingsInput {
            notification_sound_muted: true,
            ..valid_input()
        }
    );
}

#[test]
fn a_changed_weekly_schedule_replaces_the_whole_week() {
    let changes = SettingsChanges {
        weekly_auto_run_config: SettingChange::Set(WeeklyScheduleInput {
            mon: disabled_day(),
            tue: day_slot(true, "10:00", "11:00"),
            wed: disabled_day(),
            thu: disabled_day(),
            fri: disabled_day(),
            sat: disabled_day(),
            sun: disabled_day(),
        }),
        ..SettingsChanges::default()
    };

    let applied = changes.applied_to(valid_input());

    assert_eq!(applied.weekly_auto_run_config.mon, disabled_day());
    assert_eq!(applied.weekly_auto_run_config.fri, disabled_day());
    assert_eq!(
        applied.weekly_auto_run_config.tue,
        day_slot(true, "10:00", "11:00")
    );
}

#[test]
fn changes_reject_an_unknown_field() {
    let result =
        serde_json::from_value::<SettingsChanges>(json!({ "notification_sound_mute": true }));

    assert!(result.is_err());
}

#[test]
fn changes_reject_null_instead_of_reading_it_as_unchanged() {
    let result =
        serde_json::from_value::<SettingsChanges>(json!({ "notification_sound_muted": null }));

    assert!(result.is_err());
}

#[test]
fn changes_naming_nothing_are_empty() {
    let changes: SettingsChanges =
        serde_json::from_value(json!({})).expect("empty changes should deserialize");

    assert!(changes.is_empty());
}

#[test]
fn a_negative_number_is_reported_as_out_of_range_for_its_field() {
    let input = SettingsInput {
        work_duration_secs: -300,
        long_break_interval: -1,
        ..valid_input()
    };

    assert_eq!(
        issues_of(input),
        vec![
            SettingsValidationIssue::OutOfRange {
                field: RangedSettingField::WorkDurationSecs,
                min: 1,
                max: 7_200,
            },
            SettingsValidationIssue::OutOfRange {
                field: RangedSettingField::LongBreakInterval,
                min: 2,
                max: 10,
            },
        ]
    );
}

#[test]
fn changes_reject_an_unknown_field_inside_the_weekly_schedule() {
    let mut weekly = serde_json::to_value(valid_input().weekly_auto_run_config)
        .expect("weekly schedule should serialize");
    weekly["mon"]["enabeld"] = json!(true);

    let result =
        serde_json::from_value::<SettingsChanges>(json!({ "weekly_auto_run_config": weekly }));

    assert!(result.is_err());
}
