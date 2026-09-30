// The values the user chose. The backend is their only owner: they are
// stored in `settings.json`, and every way they enter the application — a
// save from the frontend or the file on disk — goes through `SettingsInput`,
// so a single set of rules decides what is valid.

use crate::ipc::settings::{
    DaySlotInput, RangedSettingField, SettingsInput, SettingsValidationIssue, TimeBoundary,
    WeeklyScheduleInput,
};
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::types::{
    DayAutoRunSlot, HhMm, LongBreakDurationSecs, LongBreakIntervalCount,
    NotificationSoundVolumePercent, OverlappingSlotsError, PostBreakTransitionPolicy,
    ShortBreakDurationSecs, WeekdayKey, WeeklyAutoRunConfig, WorkDurationSecs,
};

/// Settings file schema version currently supported by this binary.
pub(crate) const SETTINGS_SCHEMA_VERSION: u32 = 1;

/// Settings stored in `settings.json`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct PersistedSettings {
    pub schema_version: u32,
    pub settings: UserSettings,
}

/// Validated user settings. Each field carries its own range, and the weekly
/// schedule is free of overlaps.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "SettingsInput", into = "SettingsInput")]
pub(crate) struct UserSettings {
    pub work_duration_secs: WorkDurationSecs,
    pub short_break_duration_secs: ShortBreakDurationSecs,
    pub long_break_duration_secs: LongBreakDurationSecs,
    pub long_break_interval: LongBreakIntervalCount,
    pub post_break_transition_policy: PostBreakTransitionPolicy,
    pub auto_run_feature_enabled: bool,
    pub weekly_auto_run_config: WeeklyAutoRunConfig,
    pub notification_sound_volume_percent: NotificationSoundVolumePercent,
    pub notification_sound_muted: bool,
}

/// Every issue found in one settings input, never only the first.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
#[error("settings rejected: {issues:?}")]
pub(crate) struct SettingsValidationErrors {
    issues: Vec<SettingsValidationIssue>,
}

impl SettingsValidationErrors {
    pub(crate) fn issues(&self) -> &[SettingsValidationIssue] {
        &self.issues
    }

    #[cfg(test)]
    pub(crate) fn from_issues(issues: Vec<SettingsValidationIssue>) -> Self {
        Self { issues }
    }
}

impl Default for UserSettings {
    fn default() -> Self {
        Self {
            work_duration_secs: in_range_or_panic(WorkDurationSecs::try_new(25 * 60)),
            short_break_duration_secs: in_range_or_panic(ShortBreakDurationSecs::try_new(5 * 60)),
            long_break_duration_secs: in_range_or_panic(LongBreakDurationSecs::try_new(15 * 60)),
            long_break_interval: in_range_or_panic(LongBreakIntervalCount::try_new(4)),
            post_break_transition_policy: PostBreakTransitionPolicy::RequireManualContinue,
            auto_run_feature_enabled: false,
            weekly_auto_run_config: WeeklyAutoRunConfig::default(),
            notification_sound_volume_percent: in_range_or_panic(
                NotificationSoundVolumePercent::try_new(50),
            ),
            notification_sound_muted: false,
        }
    }
}

fn in_range_or_panic<T, E: std::fmt::Display>(result: Result<T, E>) -> T {
    match result {
        Ok(value) => value,
        Err(error) => panic!("default setting should be in range: {error}"),
    }
}

impl TryFrom<SettingsInput> for UserSettings {
    type Error = SettingsValidationErrors;

    fn try_from(input: SettingsInput) -> Result<Self, Self::Error> {
        let mut issues = Vec::new();

        let work_duration_secs = in_range(
            u64::try_from(input.work_duration_secs)
                .ok()
                .and_then(|value| WorkDurationSecs::try_new(value).ok()),
            RangedSettingField::WorkDurationSecs,
            (WorkDurationSecs::MIN, WorkDurationSecs::MAX),
            &mut issues,
        );
        let short_break_duration_secs = in_range(
            u64::try_from(input.short_break_duration_secs)
                .ok()
                .and_then(|value| ShortBreakDurationSecs::try_new(value).ok()),
            RangedSettingField::ShortBreakDurationSecs,
            (ShortBreakDurationSecs::MIN, ShortBreakDurationSecs::MAX),
            &mut issues,
        );
        let long_break_duration_secs = in_range(
            u64::try_from(input.long_break_duration_secs)
                .ok()
                .and_then(|value| LongBreakDurationSecs::try_new(value).ok()),
            RangedSettingField::LongBreakDurationSecs,
            (LongBreakDurationSecs::MIN, LongBreakDurationSecs::MAX),
            &mut issues,
        );
        let long_break_interval = in_range(
            u32::try_from(input.long_break_interval)
                .ok()
                .and_then(|value| LongBreakIntervalCount::try_new(value).ok()),
            RangedSettingField::LongBreakInterval,
            (
                u64::from(LongBreakIntervalCount::MIN),
                u64::from(LongBreakIntervalCount::MAX),
            ),
            &mut issues,
        );
        let notification_sound_volume_percent = in_range(
            u32::try_from(input.notification_sound_volume_percent)
                .ok()
                .and_then(|value| NotificationSoundVolumePercent::try_new(value).ok()),
            RangedSettingField::NotificationSoundVolumePercent,
            (
                u64::from(NotificationSoundVolumePercent::MIN),
                u64::from(NotificationSoundVolumePercent::MAX),
            ),
            &mut issues,
        );
        let weekly_auto_run_config =
            validated_weekly_schedule(&input.weekly_auto_run_config, &mut issues);

        let (
            Some(work_duration_secs),
            Some(short_break_duration_secs),
            Some(long_break_duration_secs),
            Some(long_break_interval),
            Some(notification_sound_volume_percent),
            Some(weekly_auto_run_config),
        ) = (
            work_duration_secs,
            short_break_duration_secs,
            long_break_duration_secs,
            long_break_interval,
            notification_sound_volume_percent,
            weekly_auto_run_config,
        )
        else {
            return Err(SettingsValidationErrors { issues });
        };

        Ok(Self {
            work_duration_secs,
            short_break_duration_secs,
            long_break_duration_secs,
            long_break_interval,
            post_break_transition_policy: input.post_break_transition_policy,
            auto_run_feature_enabled: input.auto_run_feature_enabled,
            weekly_auto_run_config,
            notification_sound_volume_percent,
            notification_sound_muted: input.notification_sound_muted,
        })
    }
}

fn in_range<T>(
    value: Option<T>,
    field: RangedSettingField,
    (min, max): (u64, u64),
    issues: &mut Vec<SettingsValidationIssue>,
) -> Option<T> {
    if value.is_none() {
        issues.push(SettingsValidationIssue::OutOfRange { field, min, max });
    }
    value
}

fn validated_weekly_schedule(
    input: &WeeklyScheduleInput,
    issues: &mut Vec<SettingsValidationIssue>,
) -> Option<WeeklyAutoRunConfig> {
    let slots =
        WeekdayKey::ALL.map(|weekday| validated_day_slot(weekday, input.slot(weekday), issues));
    let every_slot_is_valid = slots.iter().all(Option::is_some);
    // A rejected slot stands in as a disabled one, so the overlap check
    // still covers the slots that are valid.
    let [mon, tue, wed, thu, fri, sat, sun] = slots.map(Option::unwrap_or_default);

    match WeeklyAutoRunConfig::try_new(mon, tue, wed, thu, fri, sat, sun) {
        Ok(config) => every_slot_is_valid.then_some(config),
        Err(OverlappingSlotsError(overlapping_pairs)) => {
            issues.extend(overlapping_pairs.into_iter().map(
                |(primary_weekday, secondary_weekday)| {
                    SettingsValidationIssue::OverlappingTimeRanges {
                        primary_weekday,
                        secondary_weekday,
                    }
                },
            ));
            None
        }
    }
}

fn validated_day_slot(
    weekday: WeekdayKey,
    input: &DaySlotInput,
    issues: &mut Vec<SettingsValidationIssue>,
) -> Option<DayAutoRunSlot> {
    let start_time = parsed_time(&input.start_time, weekday, TimeBoundary::Start, issues);
    let end_time = parsed_time(&input.end_time, weekday, TimeBoundary::End, issues);
    let (Some(start_time), Some(end_time)) = (start_time, end_time) else {
        return None;
    };

    DayAutoRunSlot::try_new(input.enabled, start_time, end_time)
        .map_err(|_| issues.push(SettingsValidationIssue::EmptyTimeRange { weekday }))
        .ok()
}

fn parsed_time(
    time_text: &str,
    weekday: WeekdayKey,
    boundary: TimeBoundary,
    issues: &mut Vec<SettingsValidationIssue>,
) -> Option<HhMm> {
    HhMm::try_from(time_text.to_string())
        .map_err(|_| issues.push(SettingsValidationIssue::InvalidTime { weekday, boundary }))
        .ok()
}

/// Every bounded duration is far below `i64::MAX`, so the fallback is never
/// reached.
fn signed(value: u64) -> i64 {
    i64::try_from(value).unwrap_or(i64::MAX)
}

impl From<UserSettings> for SettingsInput {
    fn from(settings: UserSettings) -> Self {
        let day_slot_input = |weekday: WeekdayKey| {
            let slot = settings.weekly_auto_run_config.slot(weekday);
            DaySlotInput {
                enabled: slot.enabled(),
                start_time: slot.start_time().to_string(),
                end_time: slot.end_time().to_string(),
            }
        };

        Self {
            work_duration_secs: signed(settings.work_duration_secs.get()),
            short_break_duration_secs: signed(settings.short_break_duration_secs.get()),
            long_break_duration_secs: signed(settings.long_break_duration_secs.get()),
            long_break_interval: i64::from(settings.long_break_interval.get()),
            post_break_transition_policy: settings.post_break_transition_policy,
            auto_run_feature_enabled: settings.auto_run_feature_enabled,
            weekly_auto_run_config: WeeklyScheduleInput {
                mon: day_slot_input(WeekdayKey::Mon),
                tue: day_slot_input(WeekdayKey::Tue),
                wed: day_slot_input(WeekdayKey::Wed),
                thu: day_slot_input(WeekdayKey::Thu),
                fri: day_slot_input(WeekdayKey::Fri),
                sat: day_slot_input(WeekdayKey::Sat),
                sun: day_slot_input(WeekdayKey::Sun),
            },
            notification_sound_volume_percent: i64::from(
                settings.notification_sound_volume_percent.get(),
            ),
            notification_sound_muted: settings.notification_sound_muted,
        }
    }
}

#[cfg(test)]
mod tests;
