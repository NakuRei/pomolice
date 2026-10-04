use crate::schedule::WeekdayKey;
use crate::settings::PostBreakTransitionPolicy;
use serde::{Deserialize, Serialize};

/// Settings as they arrive, before any rule is applied. Numbers are signed
/// and times are text, so any whole number out of range and any unusable time
/// become validation issues that name the input; only a value of the wrong
/// kind, such as text where a number belongs, fails as malformed input.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
pub struct SettingsInput {
    pub work_duration_secs: i64,
    pub short_break_duration_secs: i64,
    pub long_break_duration_secs: i64,
    pub long_break_interval: i64,
    pub post_break_transition_policy: PostBreakTransitionPolicy,
    pub auto_run_feature_enabled: bool,
    pub weekly_auto_run_config: WeeklyScheduleInput,
    pub notification_sound_volume_percent: i64,
    pub notification_sound_muted: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(deny_unknown_fields)]
pub struct WeeklyScheduleInput {
    pub mon: DaySlotInput,
    pub tue: DaySlotInput,
    pub wed: DaySlotInput,
    pub thu: DaySlotInput,
    pub fri: DaySlotInput,
    pub sat: DaySlotInput,
    pub sun: DaySlotInput,
}

impl WeeklyScheduleInput {
    #[must_use]
    pub fn slot(&self, weekday: WeekdayKey) -> &DaySlotInput {
        match weekday {
            WeekdayKey::Mon => &self.mon,
            WeekdayKey::Tue => &self.tue,
            WeekdayKey::Wed => &self.wed,
            WeekdayKey::Thu => &self.thu,
            WeekdayKey::Fri => &self.fri,
            WeekdayKey::Sat => &self.sat,
            WeekdayKey::Sun => &self.sun,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(deny_unknown_fields)]
pub struct DaySlotInput {
    pub enabled: bool,
    pub start_time: String,
    pub end_time: String,
}

/// The settings a save changes; every field left out keeps its current
/// value. The weekly schedule is replaced as a whole because overlaps are
/// judged across weekdays. An unknown field or an explicit `null` is
/// rejected, so a misspelled or empty value never passes for "unchanged".
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize, specta::Type)]
#[serde(deny_unknown_fields)]
pub struct SettingsChanges {
    #[serde(default)]
    pub work_duration_secs: SettingChange<i64>,
    #[serde(default)]
    pub short_break_duration_secs: SettingChange<i64>,
    #[serde(default)]
    pub long_break_duration_secs: SettingChange<i64>,
    #[serde(default)]
    pub long_break_interval: SettingChange<i64>,
    #[serde(default)]
    pub post_break_transition_policy: SettingChange<PostBreakTransitionPolicy>,
    #[serde(default)]
    pub auto_run_feature_enabled: SettingChange<bool>,
    #[serde(default)]
    pub weekly_auto_run_config: SettingChange<WeeklyScheduleInput>,
    #[serde(default)]
    pub notification_sound_volume_percent: SettingChange<i64>,
    #[serde(default)]
    pub notification_sound_muted: SettingChange<bool>,
}

impl SettingsChanges {
    #[must_use]
    pub fn is_empty(&self) -> bool {
        *self == Self::default()
    }

    /// `current` with every changed field replaced. The result is validated
    /// as a whole, so a change is judged together with the values it joins.
    #[must_use]
    pub fn applied_to(self, current: SettingsInput) -> SettingsInput {
        SettingsInput {
            work_duration_secs: self.work_duration_secs.apply_to(current.work_duration_secs),
            short_break_duration_secs: self
                .short_break_duration_secs
                .apply_to(current.short_break_duration_secs),
            long_break_duration_secs: self
                .long_break_duration_secs
                .apply_to(current.long_break_duration_secs),
            long_break_interval: self
                .long_break_interval
                .apply_to(current.long_break_interval),
            post_break_transition_policy: self
                .post_break_transition_policy
                .apply_to(current.post_break_transition_policy),
            auto_run_feature_enabled: self
                .auto_run_feature_enabled
                .apply_to(current.auto_run_feature_enabled),
            weekly_auto_run_config: self
                .weekly_auto_run_config
                .apply_to(current.weekly_auto_run_config),
            notification_sound_volume_percent: self
                .notification_sound_volume_percent
                .apply_to(current.notification_sound_volume_percent),
            notification_sound_muted: self
                .notification_sound_muted
                .apply_to(current.notification_sound_muted),
        }
    }
}

/// Settings fields that are limited to a numeric range.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "snake_case")]
pub enum RangedSettingField {
    WorkDurationSecs,
    ShortBreakDurationSecs,
    LongBreakDurationSecs,
    LongBreakInterval,
    NotificationSoundVolumePercent,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "snake_case")]
pub enum TimeBoundary {
    Start,
    End,
}

/// One reason a settings save was rejected. Part of the public command
/// contract, so each variant names the input and nothing internal.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
#[serde(tag = "code", rename_all = "snake_case")]
pub enum SettingsValidationIssue {
    OutOfRange {
        field: RangedSettingField,
        min: u64,
        max: u64,
    },
    InvalidTime {
        weekday: WeekdayKey,
        boundary: TimeBoundary,
    },
    EmptyTimeRange {
        weekday: WeekdayKey,
    },
    OverlappingTimeRanges {
        primary_weekday: WeekdayKey,
        secondary_weekday: WeekdayKey,
    },
}

#[derive(Debug, Default, Clone, PartialEq, Eq, Deserialize, specta::Type)]
#[serde(untagged)]
pub enum SettingChange<T> {
    #[serde(skip)]
    #[default]
    Unchanged,
    Set(T),
}

impl<T> SettingChange<T> {
    fn apply_to(self, current: T) -> T {
        match self {
            Self::Unchanged => current,
            Self::Set(value) => value,
        }
    }
}
