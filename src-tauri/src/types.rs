// Common type definitions for the timer application

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::settings::{SettingsFileStatus, UserSettings};

/// Persisted state schema version currently supported by this binary.
pub(crate) const PERSISTED_STATE_SCHEMA_VERSION: u32 = 2;

/// What the timer was doing, stored in `timer_state.json`. User settings live
/// in their own file so that discarding this state never discards them.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct PersistedState {
    pub schema_version: u32,
    pub status: TimerStatus,
    pub remaining_seconds: u64,
    /// Length the running session started with; settings saved later do not
    /// change it.
    pub session_duration_secs: SessionDurationSecs,
    pub session_type: SessionType,
    pub current_cycle: u32,
    pub completed_pomodoros: u32,
    pub post_break_decision_pending: bool,
    pub saved_at: chrono::DateTime<chrono::Utc>,
    pub schedule_stop_state: ScheduleStopState,
}

/// Timer status representing the current phase
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum TimerStatus {
    Idle,
    Working,
    OnBreak,
}

/// Session type representing work or break sessions
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum SessionType {
    Work,
    ShortBreak,
    LongBreak,
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
#[error("invalid HH:MM value: {message}")]
pub(crate) struct HhMmParseError {
    message: String,
}

impl HhMmParseError {
    fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
#[error("{kind} must be between {min} and {max}, got {actual}")]
pub(crate) struct IntegerRangeValidationError {
    kind: &'static str,
    min: u64,
    max: u64,
    actual: u64,
}

impl IntegerRangeValidationError {
    fn new(kind: &'static str, actual: u64, min: u64, max: u64) -> Self {
        Self {
            kind,
            min,
            max,
            actual,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
#[error("start_time and end_time of an enabled slot must differ")]
pub(crate) struct EmptyTimeRangeError;

#[derive(Debug, Clone, PartialEq, Eq, Error)]
#[error("enabled schedule windows overlap: {0:?}")]
pub(crate) struct OverlappingSlotsError(pub Vec<(WeekdayKey, WeekdayKey)>);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct HhMm {
    hour: u8,
    minute: u8,
}

impl HhMm {
    pub(crate) fn try_new(hour: u8, minute: u8) -> std::result::Result<Self, HhMmParseError> {
        if hour >= 24 {
            return Err(HhMmParseError::new(format!(
                "hour {hour:02} is out of range"
            )));
        }

        if minute >= 60 {
            return Err(HhMmParseError::new(format!(
                "minute {minute:02} is out of range"
            )));
        }

        Ok(Self { hour, minute })
    }

    pub(crate) fn hour(self) -> u8 {
        self.hour
    }

    pub(crate) fn minute(self) -> u8 {
        self.minute
    }

    pub(crate) fn minutes_since_midnight(self) -> u32 {
        u32::from(self.hour()) * 60 + u32::from(self.minute())
    }
}

impl TryFrom<String> for HhMm {
    type Error = HhMmParseError;

    fn try_from(value: String) -> std::result::Result<Self, Self::Error> {
        let bytes = value.as_bytes();
        if bytes.len() != 5
            || bytes[2] != b':'
            || !bytes[0].is_ascii_digit()
            || !bytes[1].is_ascii_digit()
            || !bytes[3].is_ascii_digit()
            || !bytes[4].is_ascii_digit()
        {
            return Err(HhMmParseError::new(format!(
                "{value:?} must use zero-padded HH:MM format"
            )));
        }

        let hour = (bytes[0] - b'0') * 10 + (bytes[1] - b'0');
        let minute = (bytes[3] - b'0') * 10 + (bytes[4] - b'0');
        Self::try_new(hour, minute)
    }
}

impl std::fmt::Display for HhMm {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{:02}:{:02}", self.hour(), self.minute())
    }
}

macro_rules! bounded_u64_newtype {
    ($name:ident, $kind:literal, $min:expr, $max:expr) => {
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
        #[serde(try_from = "u64", into = "u64")]
        pub(crate) struct $name(u64);

        impl $name {
            pub(crate) const MIN: u64 = $min;
            pub(crate) const MAX: u64 = $max;

            pub(crate) fn try_new(
                value: u64,
            ) -> std::result::Result<Self, IntegerRangeValidationError> {
                if ($min..=$max).contains(&value) {
                    return Ok(Self(value));
                }

                Err(IntegerRangeValidationError::new($kind, value, $min, $max))
            }

            pub(crate) fn get(self) -> u64 {
                self.0
            }
        }

        impl std::fmt::Display for $name {
            fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                write!(formatter, "{}", self.0)
            }
        }

        impl TryFrom<u64> for $name {
            type Error = IntegerRangeValidationError;

            fn try_from(value: u64) -> std::result::Result<Self, Self::Error> {
                Self::try_new(value)
            }
        }

        impl From<$name> for u64 {
            fn from(value: $name) -> Self {
                value.0
            }
        }
    };
}

macro_rules! bounded_u32_newtype {
    ($name:ident, $kind:literal, $min:expr, $max:expr) => {
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
        #[serde(try_from = "u32", into = "u32")]
        pub(crate) struct $name(u32);

        impl $name {
            pub(crate) const MIN: u32 = $min;
            pub(crate) const MAX: u32 = $max;

            pub(crate) fn try_new(
                value: u32,
            ) -> std::result::Result<Self, IntegerRangeValidationError> {
                if ($min..=$max).contains(&value) {
                    return Ok(Self(value));
                }

                Err(IntegerRangeValidationError::new(
                    $kind,
                    u64::from(value),
                    $min as u64,
                    $max as u64,
                ))
            }

            pub(crate) fn get(self) -> u32 {
                self.0
            }
        }

        impl std::fmt::Display for $name {
            fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                write!(formatter, "{}", self.0)
            }
        }

        impl TryFrom<u32> for $name {
            type Error = IntegerRangeValidationError;

            fn try_from(value: u32) -> std::result::Result<Self, Self::Error> {
                Self::try_new(value)
            }
        }

        impl From<$name> for u32 {
            fn from(value: $name) -> Self {
                value.0
            }
        }
    };
}

// The 1-second minimums let a developer reach a break without waiting;
// raise them before release.
bounded_u64_newtype!(WorkDurationSecs, "work_duration_secs", 1, 7_200);
bounded_u64_newtype!(
    ShortBreakDurationSecs,
    "short_break_duration_secs",
    1,
    3_600
);
bounded_u64_newtype!(LongBreakDurationSecs, "long_break_duration_secs", 1, 3_600);
bounded_u32_newtype!(LongBreakIntervalCount, "long_break_interval", 2, 10);
bounded_u32_newtype!(
    NotificationSoundVolumePercent,
    "notification_sound_volume_percent",
    0,
    100
);
// Covers every session kind, so it is bounded by the longest one.
bounded_u64_newtype!(
    SessionDurationSecs,
    "session_duration_secs",
    1,
    WorkDurationSecs::MAX
);

// The conversions below are infallible only while every session kind fits
// inside the session range.
const _: () = assert!(
    SessionDurationSecs::MIN <= WorkDurationSecs::MIN
        && WorkDurationSecs::MAX <= SessionDurationSecs::MAX
        && SessionDurationSecs::MIN <= ShortBreakDurationSecs::MIN
        && ShortBreakDurationSecs::MAX <= SessionDurationSecs::MAX
        && SessionDurationSecs::MIN <= LongBreakDurationSecs::MIN
        && LongBreakDurationSecs::MAX <= SessionDurationSecs::MAX
);

impl From<WorkDurationSecs> for SessionDurationSecs {
    fn from(value: WorkDurationSecs) -> Self {
        Self(value.get())
    }
}

impl From<ShortBreakDurationSecs> for SessionDurationSecs {
    fn from(value: ShortBreakDurationSecs) -> Self {
        Self(value.get())
    }
}

impl From<LongBreakDurationSecs> for SessionDurationSecs {
    fn from(value: LongBreakDurationSecs) -> Self {
        Self(value.get())
    }
}

fn hh_mm_or_panic(hour: u8, minute: u8) -> HhMm {
    match HhMm::try_new(hour, minute) {
        Ok(value) => value,
        Err(error) => panic!("default HH:MM should be valid: {error}"),
    }
}

/// Monday-first weekday keys shared by the schedule and its validation issues.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum WeekdayKey {
    Mon,
    Tue,
    Wed,
    Thu,
    Fri,
    Sat,
    Sun,
}

impl WeekdayKey {
    pub(crate) const ALL: [Self; 7] = [
        Self::Mon,
        Self::Tue,
        Self::Wed,
        Self::Thu,
        Self::Fri,
        Self::Sat,
        Self::Sun,
    ];

    fn index_from_monday(self) -> u32 {
        match self {
            Self::Mon => 0,
            Self::Tue => 1,
            Self::Wed => 2,
            Self::Thu => 3,
            Self::Fri => 4,
            Self::Sat => 5,
            Self::Sun => 6,
        }
    }
}

/// A single weekday schedule slot for automatic timer execution.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct DayAutoRunSlot {
    enabled: bool,
    start_time: HhMm,
    end_time: HhMm,
}

impl DayAutoRunSlot {
    /// A disabled slot keeps its times only so they can be edited later, so
    /// it is never checked for an empty range.
    pub(crate) fn try_new(
        enabled: bool,
        start_time: HhMm,
        end_time: HhMm,
    ) -> std::result::Result<Self, EmptyTimeRangeError> {
        if enabled && start_time == end_time {
            return Err(EmptyTimeRangeError);
        }

        Ok(Self {
            enabled,
            start_time,
            end_time,
        })
    }

    pub(crate) fn enabled(self) -> bool {
        self.enabled
    }

    pub(crate) fn start_time(self) -> HhMm {
        self.start_time
    }

    pub(crate) fn end_time(self) -> HhMm {
        self.end_time
    }

    pub(crate) fn crosses_midnight(self) -> bool {
        self.start_time.minutes_since_midnight() > self.end_time.minutes_since_midnight()
    }

    pub(crate) fn parsed_time_range(self) -> (u32, u32, bool) {
        (
            self.start_time.minutes_since_midnight(),
            self.end_time.minutes_since_midnight(),
            self.crosses_midnight(),
        )
    }
}

impl Default for DayAutoRunSlot {
    fn default() -> Self {
        Self {
            enabled: false,
            start_time: hh_mm_or_panic(9, 0),
            end_time: hh_mm_or_panic(18, 0),
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct WeekMinuteRange {
    start_minute_of_week: u32,
    end_minute_of_week: u32,
}

/// Weekly schedule configuration normalized to Monday-Sunday keys.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct WeeklyAutoRunConfig {
    mon: DayAutoRunSlot,
    tue: DayAutoRunSlot,
    wed: DayAutoRunSlot,
    thu: DayAutoRunSlot,
    fri: DayAutoRunSlot,
    sat: DayAutoRunSlot,
    sun: DayAutoRunSlot,
}

impl WeeklyAutoRunConfig {
    /// Rejects a schedule whose enabled slots overlap, reporting every
    /// overlapping pair.
    pub(crate) fn try_new(
        mon: DayAutoRunSlot,
        tue: DayAutoRunSlot,
        wed: DayAutoRunSlot,
        thu: DayAutoRunSlot,
        fri: DayAutoRunSlot,
        sat: DayAutoRunSlot,
        sun: DayAutoRunSlot,
    ) -> std::result::Result<Self, OverlappingSlotsError> {
        let config = Self {
            mon,
            tue,
            wed,
            thu,
            fri,
            sat,
            sun,
        };

        let overlapping_pairs = find_overlapping_weekday_pairs(&config);
        if !overlapping_pairs.is_empty() {
            return Err(OverlappingSlotsError(overlapping_pairs));
        }

        Ok(config)
    }

    pub(crate) fn slot(&self, weekday: WeekdayKey) -> DayAutoRunSlot {
        match weekday {
            WeekdayKey::Mon => self.mon,
            WeekdayKey::Tue => self.tue,
            WeekdayKey::Wed => self.wed,
            WeekdayKey::Thu => self.thu,
            WeekdayKey::Fri => self.fri,
            WeekdayKey::Sat => self.sat,
            WeekdayKey::Sun => self.sun,
        }
    }

    pub(crate) fn mon(&self) -> DayAutoRunSlot {
        self.mon
    }

    pub(crate) fn tue(&self) -> DayAutoRunSlot {
        self.tue
    }

    pub(crate) fn wed(&self) -> DayAutoRunSlot {
        self.wed
    }

    pub(crate) fn thu(&self) -> DayAutoRunSlot {
        self.thu
    }

    pub(crate) fn fri(&self) -> DayAutoRunSlot {
        self.fri
    }

    pub(crate) fn sat(&self) -> DayAutoRunSlot {
        self.sat
    }

    pub(crate) fn sun(&self) -> DayAutoRunSlot {
        self.sun
    }
}

fn find_overlapping_weekday_pairs(config: &WeeklyAutoRunConfig) -> Vec<(WeekdayKey, WeekdayKey)> {
    let weekday_ranges: Vec<(WeekdayKey, Vec<WeekMinuteRange>)> = WeekdayKey::ALL
        .into_iter()
        .filter(|weekday| config.slot(*weekday).enabled())
        .map(|weekday| {
            let slot = config.slot(weekday);
            (
                weekday,
                build_week_minute_ranges(
                    weekday.index_from_monday(),
                    slot.start_time().minutes_since_midnight(),
                    slot.end_time().minutes_since_midnight(),
                    slot.crosses_midnight(),
                ),
            )
        })
        .collect();

    let mut overlapping_pairs = Vec::new();
    for (primary_index, (primary_weekday, primary_ranges)) in weekday_ranges.iter().enumerate() {
        for (secondary_weekday, secondary_ranges) in &weekday_ranges[primary_index + 1..] {
            let overlaps = primary_ranges.iter().any(|primary_range| {
                secondary_ranges.iter().any(|secondary_range| {
                    week_minute_ranges_overlap(*primary_range, *secondary_range)
                })
            });
            if overlaps {
                overlapping_pairs.push((*primary_weekday, *secondary_weekday));
            }
        }
    }
    overlapping_pairs
}

impl Default for WeeklyAutoRunConfig {
    fn default() -> Self {
        let slot = DayAutoRunSlot::default();
        Self {
            mon: slot,
            tue: slot,
            wed: slot,
            thu: slot,
            fri: slot,
            sat: slot,
            sun: slot,
        }
    }
}

fn build_week_minute_ranges(
    weekday_index: u32,
    start_minutes: u32,
    end_minutes: u32,
    crosses_midnight: bool,
) -> Vec<WeekMinuteRange> {
    const MINUTES_PER_DAY: u32 = 24 * 60;

    let day_start_minute_of_week = weekday_index * MINUTES_PER_DAY;
    let start_minute_of_week = day_start_minute_of_week + start_minutes;
    let end_minute_of_week = day_start_minute_of_week + end_minutes;

    if !crosses_midnight {
        return vec![WeekMinuteRange {
            start_minute_of_week,
            end_minute_of_week,
        }];
    }

    let next_weekday_index = (weekday_index + 1) % 7;
    vec![
        WeekMinuteRange {
            start_minute_of_week,
            end_minute_of_week: day_start_minute_of_week + MINUTES_PER_DAY,
        },
        WeekMinuteRange {
            start_minute_of_week: next_weekday_index * MINUTES_PER_DAY,
            end_minute_of_week: next_weekday_index * MINUTES_PER_DAY + end_minutes,
        },
    ]
}

fn week_minute_ranges_overlap(left: WeekMinuteRange, right: WeekMinuteRange) -> bool {
    left.start_minute_of_week < right.end_minute_of_week
        && right.start_minute_of_week < left.end_minute_of_week
}

/// Origin of a work-session start request.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub(crate) enum WorkStartOrigin {
    #[default]
    ManualOutsideScheduleWindow,
    ManualInsideScheduleWindow,
    ScheduleStartBoundary,
}

/// Current stop behavior for schedule-controlled sessions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub(crate) enum ScheduleStopState {
    #[default]
    NotControlledBySchedule,
    WaitingForScheduleEnd,
    StopAtNextSessionBoundary,
}

/// Whether the user is at the desk during the current break.
///
/// `Away` keeps the break running but makes its end wait for a manual
/// decision even under the automatic post-break policy, so an unattended
/// machine does not run through work sessions nobody is doing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) enum BreakAttendance {
    #[default]
    AtDesk,
    Away,
}

impl BreakAttendance {
    pub(crate) fn from_away_flag(away: bool) -> Self {
        if away {
            Self::Away
        } else {
            Self::AtDesk
        }
    }

    pub(crate) fn is_away(self) -> bool {
        self == Self::Away
    }
}

/// Policy for deciding what happens when a break session finishes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub(crate) enum PostBreakTransitionPolicy {
    #[default]
    RequireManualContinue,
    StartNextWorkAutomatically,
}

/// Action selected while a post-break manual decision is pending.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum PostBreakDecisionAction {
    Continue,
    StopCycle,
}

/// Reason emitted for a session-boundary transition.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum SessionTransitionReason {
    WorkToBreak,
    BreakWaitingDecision,
    BreakAutoStarted,
    ScheduledStopExecuted,
}

/// Identifies one occurrence of a schedule boundary in logs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct BoundaryOccurrenceId(String);

impl BoundaryOccurrenceId {
    pub(crate) fn for_schedule_boundary(
        slot_key: &str,
        boundary_kind: &str,
        epoch_seconds: i64,
    ) -> Self {
        Self(format!("{slot_key}|{boundary_kind}|{epoch_seconds}"))
    }

    #[cfg(test)]
    pub(crate) fn epoch_seconds(&self) -> Option<i64> {
        self.0.rsplit('|').next()?.parse::<i64>().ok()
    }

    pub(crate) fn as_str(&self) -> &str {
        &self.0
    }
}

/// Payload sent to frontend on each timer tick
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub(crate) struct TimerTickPayload {
    pub status: TimerStatus,
    pub remaining_seconds: u64,
    /// Total length of the current session; while idle, the length the next
    /// work session will have.
    pub session_duration_secs: u64,
    pub session_type: SessionType,
    pub current_cycle: u32,
    pub completed_pomodoros: u32,
    pub break_interaction_count: u32,
    pub away_during_break: bool,
}

/// Payload sent to frontend when a session completes
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub(crate) struct SessionCompletedPayload {
    pub transition_reason: SessionTransitionReason,
    pub completed_session_type: SessionType,
    pub next_status: TimerStatus,
    pub next_session_type: SessionType,
    pub requires_manual_continue_decision: bool,
}

impl SessionCompletedPayload {
    pub(crate) fn work_to_break(next_session_type: SessionType) -> Self {
        debug_assert!(matches!(
            next_session_type,
            SessionType::ShortBreak | SessionType::LongBreak
        ));
        Self {
            transition_reason: SessionTransitionReason::WorkToBreak,
            completed_session_type: SessionType::Work,
            next_status: TimerStatus::OnBreak,
            next_session_type,
            requires_manual_continue_decision: false,
        }
    }

    pub(crate) fn break_waiting_decision(completed_session_type: SessionType) -> Self {
        debug_assert!(matches!(
            completed_session_type,
            SessionType::ShortBreak | SessionType::LongBreak
        ));
        Self {
            transition_reason: SessionTransitionReason::BreakWaitingDecision,
            completed_session_type,
            next_status: TimerStatus::Idle,
            next_session_type: SessionType::Work,
            requires_manual_continue_decision: true,
        }
    }

    pub(crate) fn break_auto_started(completed_session_type: SessionType) -> Self {
        debug_assert!(matches!(
            completed_session_type,
            SessionType::ShortBreak | SessionType::LongBreak
        ));
        Self {
            transition_reason: SessionTransitionReason::BreakAutoStarted,
            completed_session_type,
            next_status: TimerStatus::Working,
            next_session_type: SessionType::Work,
            requires_manual_continue_decision: false,
        }
    }

    pub(crate) fn scheduled_stop_executed(completed_session_type: SessionType) -> Self {
        Self {
            transition_reason: SessionTransitionReason::ScheduledStopExecuted,
            completed_session_type,
            next_status: TimerStatus::Idle,
            next_session_type: SessionType::Work,
            requires_manual_continue_decision: false,
        }
    }
}

/// Full timer state sent on initial sync
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub(crate) struct FullStatePayload {
    pub status: TimerStatus,
    pub remaining_seconds: u64,
    pub session_duration_secs: u64,
    pub session_type: SessionType,
    pub current_cycle: u32,
    pub completed_pomodoros: u32,
    pub break_interaction_count: u32,
    pub away_during_break: bool,
    pub post_break_decision_pending: bool,
    pub settings: UserSettings,
    pub settings_file_status: SettingsFileStatus,
    /// Present while the boot auto-start prompt is still waiting for a decision.
    pub boot_auto_start_offer: Option<BootAutoStartOfferPayload>,
    pub active_window_summary: Option<ActiveWindowSummaryPayload>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub(crate) struct BootAutoStartOfferPayload {
    pub anchor_local_date: chrono::NaiveDate,
    pub window: ActiveWindowSummaryPayload,
}

/// Summary of the currently active schedule window.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub(crate) struct ActiveWindowSummaryPayload {
    pub weekday_key: String,
    pub start_time: String,
    pub end_time: String,
    pub crosses_midnight: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub(crate) struct ActiveWindowChangedPayload {
    pub active_window_summary: Option<ActiveWindowSummaryPayload>,
}

/// Payload sent to frontend when the settings in effect change
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub(crate) struct SettingsChangedPayload {
    pub settings: UserSettings,
    pub settings_file_status: SettingsFileStatus,
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

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

    #[test]
    fn test_bounded_newtypes_accept_their_bounds_and_reject_values_outside() {
        assert!(WorkDurationSecs::try_new(1).is_ok());
        assert!(WorkDurationSecs::try_new(7_200).is_ok());
        assert!(WorkDurationSecs::try_new(0).is_err());
        assert!(WorkDurationSecs::try_new(7_201).is_err());
        assert!(ShortBreakDurationSecs::try_new(3_600).is_ok());
        assert!(ShortBreakDurationSecs::try_new(3_601).is_err());
        assert!(LongBreakDurationSecs::try_new(3_600).is_ok());
        assert!(LongBreakDurationSecs::try_new(3_601).is_err());
        assert!(LongBreakIntervalCount::try_new(2).is_ok());
        assert!(LongBreakIntervalCount::try_new(10).is_ok());
        assert!(LongBreakIntervalCount::try_new(1).is_err());
        assert!(LongBreakIntervalCount::try_new(11).is_err());
        assert!(NotificationSoundVolumePercent::try_new(0).is_ok());
        assert!(NotificationSoundVolumePercent::try_new(100).is_ok());
        assert!(NotificationSoundVolumePercent::try_new(101).is_err());
        assert!(serde_json::from_value::<SessionDurationSecs>(json!(0)).is_err());
        assert!(serde_json::from_value::<SessionDurationSecs>(json!(7_201)).is_err());
    }

    #[test]
    fn test_work_start_origin_default() {
        assert_eq!(
            WorkStartOrigin::default(),
            WorkStartOrigin::ManualOutsideScheduleWindow
        );
    }

    #[test]
    fn test_schedule_stop_state_default() {
        assert_eq!(
            ScheduleStopState::default(),
            ScheduleStopState::NotControlledBySchedule
        );
    }

    #[test]
    fn test_post_break_transition_policy_default() {
        assert_eq!(
            PostBreakTransitionPolicy::default(),
            PostBreakTransitionPolicy::RequireManualContinue
        );
    }

    #[test]
    fn test_boundary_occurrence_id_exposes_schedule_boundary_epoch() {
        let boundary_id =
            BoundaryOccurrenceId::for_schedule_boundary("mon", "start", 1_738_707_600);

        assert_eq!(boundary_id.epoch_seconds(), Some(1_738_707_600));
    }

    #[test]
    fn test_boundary_occurrence_id_matches_same_schedule_boundary() {
        let first = BoundaryOccurrenceId::for_schedule_boundary("mon", "start", 1_738_707_600);
        let same = BoundaryOccurrenceId::for_schedule_boundary("mon", "start", 1_738_707_600);
        let different = BoundaryOccurrenceId::for_schedule_boundary("mon", "end", 1_738_707_600);

        assert_eq!(first, same);
        assert_ne!(first, different);
    }

    #[test]
    fn session_completed_payloads_carry_the_transition_contract() {
        let work_to_break = SessionCompletedPayload::work_to_break(SessionType::LongBreak);
        assert_eq!(
            work_to_break.transition_reason,
            SessionTransitionReason::WorkToBreak
        );
        assert_eq!(work_to_break.completed_session_type, SessionType::Work);
        assert_eq!(work_to_break.next_status, TimerStatus::OnBreak);
        assert_eq!(work_to_break.next_session_type, SessionType::LongBreak);
        assert!(!work_to_break.requires_manual_continue_decision);

        let waiting = SessionCompletedPayload::break_waiting_decision(SessionType::ShortBreak);
        assert_eq!(
            waiting.transition_reason,
            SessionTransitionReason::BreakWaitingDecision
        );
        assert_eq!(waiting.completed_session_type, SessionType::ShortBreak);
        assert_eq!(waiting.next_status, TimerStatus::Idle);
        assert_eq!(waiting.next_session_type, SessionType::Work);
        assert!(waiting.requires_manual_continue_decision);
    }
}
