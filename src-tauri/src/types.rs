// Common type definitions for the timer application

use serde::{Deserialize, Serialize};
use thiserror::Error;

#[cfg(test)]
/// Legacy persisted state schema version retained for fallback tests.
pub(crate) const PERSISTED_STATE_SCHEMA_VERSION_V3: u32 = 3;
/// Persisted state schema version currently supported by this binary.
pub(crate) const PERSISTED_STATE_SCHEMA_VERSION_V4: u32 = 4;

/// Timer status representing the current phase
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum TimerStatus {
    Idle,
    Working,
    OnBreak,
}

/// Session type representing work or break sessions
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
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
pub(crate) enum ScheduleValidationError {
    #[error("start_time and end_time must differ")]
    EmptyRange,
    #[error("overlapping schedule windows between {primary_weekday} and {secondary_weekday}")]
    Overlap {
        primary_weekday: &'static str,
        secondary_weekday: &'static str,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
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

impl From<HhMm> for String {
    fn from(value: HhMm) -> Self {
        value.to_string()
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

bounded_u64_newtype!(WorkDurationSecs, "work_duration_secs", 1, 86_400);
bounded_u64_newtype!(
    ShortBreakDurationSecs,
    "short_break_duration_secs",
    1,
    86_400
);
bounded_u64_newtype!(LongBreakDurationSecs, "long_break_duration_secs", 1, 86_400);
bounded_u32_newtype!(LongBreakIntervalCount, "long_break_interval", 1, 100);

fn hh_mm_or_panic(hour: u8, minute: u8) -> HhMm {
    match HhMm::try_new(hour, minute) {
        Ok(value) => value,
        Err(error) => panic!("default HH:MM should be valid: {error}"),
    }
}

fn work_duration_secs_or_panic(value: u64) -> WorkDurationSecs {
    match WorkDurationSecs::try_new(value) {
        Ok(duration) => duration,
        Err(error) => panic!("default work duration should be valid: {error}"),
    }
}

fn short_break_duration_secs_or_panic(value: u64) -> ShortBreakDurationSecs {
    match ShortBreakDurationSecs::try_new(value) {
        Ok(duration) => duration,
        Err(error) => panic!("default short break duration should be valid: {error}"),
    }
}

fn long_break_duration_secs_or_panic(value: u64) -> LongBreakDurationSecs {
    match LongBreakDurationSecs::try_new(value) {
        Ok(duration) => duration,
        Err(error) => panic!("default long break duration should be valid: {error}"),
    }
}

fn long_break_interval_count_or_panic(value: u32) -> LongBreakIntervalCount {
    match LongBreakIntervalCount::try_new(value) {
        Ok(interval) => interval,
        Err(error) => panic!("default long break interval should be valid: {error}"),
    }
}

/// Timer settings synchronized from frontend
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct TimerSettings {
    pub work_duration_secs: WorkDurationSecs,
    pub short_break_duration_secs: ShortBreakDurationSecs,
    pub long_break_duration_secs: LongBreakDurationSecs,
    pub long_break_interval: LongBreakIntervalCount,
    #[serde(default)]
    pub post_break_transition_policy: PostBreakTransitionPolicy,
}

impl TimerSettings {
    pub(crate) fn new(
        work_duration_secs: WorkDurationSecs,
        short_break_duration_secs: ShortBreakDurationSecs,
        long_break_duration_secs: LongBreakDurationSecs,
        long_break_interval: LongBreakIntervalCount,
        post_break_transition_policy: PostBreakTransitionPolicy,
    ) -> Self {
        Self {
            work_duration_secs,
            short_break_duration_secs,
            long_break_duration_secs,
            long_break_interval,
            post_break_transition_policy,
        }
    }

    #[cfg(test)]
    pub(crate) fn try_from_raw(
        work_duration_secs: u64,
        short_break_duration_secs: u64,
        long_break_duration_secs: u64,
        long_break_interval: u32,
        post_break_transition_policy: PostBreakTransitionPolicy,
    ) -> std::result::Result<Self, IntegerRangeValidationError> {
        Ok(Self::new(
            WorkDurationSecs::try_new(work_duration_secs)?,
            ShortBreakDurationSecs::try_new(short_break_duration_secs)?,
            LongBreakDurationSecs::try_new(long_break_duration_secs)?,
            LongBreakIntervalCount::try_new(long_break_interval)?,
            post_break_transition_policy,
        ))
    }
}

impl Default for TimerSettings {
    fn default() -> Self {
        Self {
            work_duration_secs: work_duration_secs_or_panic(25 * 60),
            short_break_duration_secs: short_break_duration_secs_or_panic(5 * 60),
            long_break_duration_secs: long_break_duration_secs_or_panic(15 * 60),
            long_break_interval: long_break_interval_count_or_panic(4),
            post_break_transition_policy: PostBreakTransitionPolicy::RequireManualContinue,
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
struct RawDayAutoRunSlot {
    enabled: bool,
    #[serde(alias = "startTime")]
    start_time: HhMm,
    #[serde(alias = "endTime")]
    end_time: HhMm,
}

/// A single weekday schedule slot for automatic timer execution.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "RawDayAutoRunSlot", into = "RawDayAutoRunSlot")]
pub(crate) struct DayAutoRunSlot {
    enabled: bool,
    start_time: HhMm,
    end_time: HhMm,
}

impl DayAutoRunSlot {
    pub(crate) fn try_new(
        enabled: bool,
        start_time: HhMm,
        end_time: HhMm,
    ) -> std::result::Result<Self, ScheduleValidationError> {
        if start_time == end_time {
            return Err(ScheduleValidationError::EmptyRange);
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

impl TryFrom<RawDayAutoRunSlot> for DayAutoRunSlot {
    type Error = ScheduleValidationError;

    fn try_from(raw: RawDayAutoRunSlot) -> std::result::Result<Self, Self::Error> {
        Self::try_new(raw.enabled, raw.start_time, raw.end_time)
    }
}

impl From<DayAutoRunSlot> for RawDayAutoRunSlot {
    fn from(value: DayAutoRunSlot) -> Self {
        Self {
            enabled: value.enabled,
            start_time: value.start_time,
            end_time: value.end_time,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct RawWeeklyAutoRunConfig {
    mon: DayAutoRunSlot,
    tue: DayAutoRunSlot,
    wed: DayAutoRunSlot,
    thu: DayAutoRunSlot,
    fri: DayAutoRunSlot,
    sat: DayAutoRunSlot,
    sun: DayAutoRunSlot,
}

#[derive(Debug, Clone, Copy)]
struct WeekMinuteRange {
    start_minute_of_week: u32,
    end_minute_of_week: u32,
}

/// Weekly schedule configuration normalized to Monday-Sunday keys.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "RawWeeklyAutoRunConfig", into = "RawWeeklyAutoRunConfig")]
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
    pub(crate) fn try_new(
        mon: DayAutoRunSlot,
        tue: DayAutoRunSlot,
        wed: DayAutoRunSlot,
        thu: DayAutoRunSlot,
        fri: DayAutoRunSlot,
        sat: DayAutoRunSlot,
        sun: DayAutoRunSlot,
    ) -> std::result::Result<Self, ScheduleValidationError> {
        let config = Self {
            mon,
            tue,
            wed,
            thu,
            fri,
            sat,
            sun,
        };

        if let Some((primary_weekday, secondary_weekday)) =
            config.find_first_overlapping_weekday_pair()
        {
            return Err(ScheduleValidationError::Overlap {
                primary_weekday,
                secondary_weekday,
            });
        }

        Ok(config)
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

    pub(crate) fn find_first_overlapping_weekday_pair(
        &self,
    ) -> Option<(&'static str, &'static str)> {
        let weekday_ranges: Vec<(&'static str, Vec<WeekMinuteRange>)> = [
            ("mon", 0_u32, self.mon),
            ("tue", 1, self.tue),
            ("wed", 2, self.wed),
            ("thu", 3, self.thu),
            ("fri", 4, self.fri),
            ("sat", 5, self.sat),
            ("sun", 6, self.sun),
        ]
        .into_iter()
        .filter(|(_, _, slot)| slot.enabled())
        .map(|(weekday_key, weekday_index, slot)| {
            (
                weekday_key,
                build_week_minute_ranges(
                    weekday_index,
                    slot.start_time().minutes_since_midnight(),
                    slot.end_time().minutes_since_midnight(),
                    slot.crosses_midnight(),
                ),
            )
        })
        .collect();

        for primary_index in 0..weekday_ranges.len() {
            for secondary_index in (primary_index + 1)..weekday_ranges.len() {
                let (primary_weekday, primary_ranges) = &weekday_ranges[primary_index];
                let (secondary_weekday, secondary_ranges) = &weekday_ranges[secondary_index];

                for primary_range in primary_ranges {
                    for secondary_range in secondary_ranges {
                        if week_minute_ranges_overlap(*primary_range, *secondary_range) {
                            return Some((primary_weekday, secondary_weekday));
                        }
                    }
                }
            }
        }

        None
    }
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

impl TryFrom<RawWeeklyAutoRunConfig> for WeeklyAutoRunConfig {
    type Error = ScheduleValidationError;

    fn try_from(raw: RawWeeklyAutoRunConfig) -> std::result::Result<Self, Self::Error> {
        Self::try_new(
            raw.mon, raw.tue, raw.wed, raw.thu, raw.fri, raw.sat, raw.sun,
        )
    }
}

impl From<WeeklyAutoRunConfig> for RawWeeklyAutoRunConfig {
    fn from(value: WeeklyAutoRunConfig) -> Self {
        Self {
            mon: value.mon,
            tue: value.tue,
            wed: value.wed,
            thu: value.thu,
            fri: value.fri,
            sat: value.sat,
            sun: value.sun,
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

/// Source that started the current work timer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub(crate) enum TimerStartSource {
    #[default]
    ManualOutsideWindow,
    ManualInsideWindow,
    ScheduledAutoStart,
}

/// Current state for session-boundary scheduled stop execution.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub(crate) enum ScheduledStopState {
    #[default]
    None,
    Pending,
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

/// Unified contract for session-boundary transition results.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub(crate) struct SessionTransitionOutcome {
    pub transition_reason: SessionTransitionReason,
    pub completed_session_type: SessionType,
    pub next_status: TimerStatus,
    pub next_session_type: SessionType,
    pub requires_manual_continue_decision: bool,
}

impl SessionTransitionOutcome {
    pub(crate) fn from_completed_payload(payload: &SessionCompletedPayload) -> Self {
        Self {
            transition_reason: payload.transition_reason,
            completed_session_type: payload.completed_session_type,
            next_status: payload.next_status,
            next_session_type: payload.next_session_type,
            requires_manual_continue_decision: payload.requires_manual_continue_decision,
        }
    }

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

/// Unique boundary identifier used to deduplicate processed schedule boundaries.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Default)]
pub(crate) struct BoundaryOccurrenceId(String);

impl BoundaryOccurrenceId {
    pub(crate) fn for_schedule_boundary(
        slot_key: &str,
        boundary_kind: &str,
        epoch_seconds: i64,
    ) -> Self {
        Self(format!("{slot_key}|{boundary_kind}|{epoch_seconds}"))
    }

    pub(crate) fn epoch_seconds(&self) -> Option<i64> {
        self.0.rsplit('|').next()?.parse::<i64>().ok()
    }

    pub(crate) fn as_str(&self) -> &str {
        &self.0
    }

    #[cfg(test)]
    pub(crate) fn from_raw_for_tests(value: impl Into<String>) -> Self {
        Self(value.into())
    }
}

/// Runtime state for the one-time startup confirmation decision.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub(crate) enum BootPromptRuntimeState {
    AwaitingDecision,
    ResolvedStart,
    ResolvedLater,
    #[default]
    NotApplicable,
}

/// Payload sent to frontend on each timer tick
#[derive(Debug, Clone, Serialize)]
pub(crate) struct TimerTickPayload {
    pub status: TimerStatus,
    pub remaining_seconds: u64,
    pub session_type: SessionType,
    pub current_cycle: u32,
    pub completed_pomodoros: u32,
    pub break_interaction_count: u32,
    pub is_fullscreen_break: bool,
}

/// Payload sent to frontend when a session completes
#[derive(Debug, Clone, Serialize)]
pub(crate) struct SessionCompletedPayload {
    pub transition_reason: SessionTransitionReason,
    pub completed_session_type: SessionType,
    pub next_status: TimerStatus,
    pub next_session_type: SessionType,
    pub requires_manual_continue_decision: bool,
    /// Compatibility alias for already-migrated frontend fields.
    /// This mirrors `requires_manual_continue_decision`.
    pub post_break_decision_pending: bool,
}

impl SessionCompletedPayload {
    pub(crate) fn from_transition_outcome(outcome: &SessionTransitionOutcome) -> Self {
        Self {
            transition_reason: outcome.transition_reason,
            completed_session_type: outcome.completed_session_type,
            next_status: outcome.next_status,
            next_session_type: outcome.next_session_type,
            requires_manual_continue_decision: outcome.requires_manual_continue_decision,
            post_break_decision_pending: outcome.requires_manual_continue_decision,
        }
    }
}

/// Full timer state sent on initial sync
#[derive(Debug, Clone, Serialize)]
pub(crate) struct FullStatePayload {
    pub status: TimerStatus,
    pub remaining_seconds: u64,
    pub session_type: SessionType,
    pub current_cycle: u32,
    pub completed_pomodoros: u32,
    pub break_interaction_count: u32,
    pub is_fullscreen_break: bool,
    pub post_break_decision_pending: bool,
    pub settings: TimerSettings,
    pub should_prompt_start_on_boot: bool,
    pub active_window_summary: Option<ActiveWindowSummaryPayload>,
}

/// Summary of the currently active schedule window.
#[derive(Debug, Clone, Serialize)]
pub(crate) struct ActiveWindowSummaryPayload {
    pub weekday_key: String,
    pub start_time: String,
    pub end_time: String,
    pub crosses_midnight: bool,
}

/// Application state for timer management
#[derive(Debug, Clone)]
pub(crate) struct TimerState {
    pub status: TimerStatus,
    pub remaining_seconds: u64,
    pub session_type: SessionType,
    pub current_cycle: u32,
    pub completed_pomodoros: u32,
    pub break_interaction_count: u32,
    pub last_penalty_time: Option<std::time::Instant>,
    pub settings: TimerSettings,
    pub auto_run_feature_enabled: bool,
    pub weekly_auto_run_config: WeeklyAutoRunConfig,
    pub timer_start_source: Option<TimerStartSource>,
    pub scheduled_stop_state: ScheduledStopState,
    pub post_break_decision_pending: bool,
    pub is_fullscreen_break: bool,
    pub last_transition: Option<std::time::Instant>,
}

impl Default for TimerState {
    fn default() -> Self {
        let settings = TimerSettings::default();
        Self {
            status: TimerStatus::Idle,
            remaining_seconds: settings.work_duration_secs.get(),
            session_type: SessionType::Work,
            current_cycle: 0,
            completed_pomodoros: 0,
            break_interaction_count: 0,
            last_penalty_time: None,
            settings,
            auto_run_feature_enabled: false,
            weekly_auto_run_config: WeeklyAutoRunConfig::default(),
            timer_start_source: None,
            scheduled_stop_state: ScheduledStopState::None,
            post_break_decision_pending: false,
            is_fullscreen_break: false,
            last_transition: None,
        }
    }
}

impl TimerState {
    pub(crate) fn to_tick_payload(&self) -> TimerTickPayload {
        TimerTickPayload {
            status: self.status,
            remaining_seconds: self.remaining_seconds,
            session_type: self.session_type,
            current_cycle: self.current_cycle,
            completed_pomodoros: self.completed_pomodoros,
            break_interaction_count: self.break_interaction_count,
            is_fullscreen_break: self.is_fullscreen_break,
        }
    }

    pub(crate) fn to_full_payload(&self) -> FullStatePayload {
        FullStatePayload {
            status: self.status,
            remaining_seconds: self.remaining_seconds,
            session_type: self.session_type,
            current_cycle: self.current_cycle,
            completed_pomodoros: self.completed_pomodoros,
            break_interaction_count: self.break_interaction_count,
            is_fullscreen_break: self.is_fullscreen_break,
            post_break_decision_pending: self.post_break_decision_pending,
            settings: self.settings.clone(),
            should_prompt_start_on_boot: false,
            active_window_summary: None,
        }
    }

    /// Get break duration based on session type
    pub(crate) fn get_break_duration(&self) -> u64 {
        match self.session_type {
            SessionType::ShortBreak => self.settings.short_break_duration_secs.get(),
            SessionType::LongBreak => self.settings.long_break_duration_secs.get(),
            SessionType::Work => 0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::Deserialize;
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
        let value: HhMm = serde_json::from_str(r#""08:05""#).expect("HH:MM should deserialize");
        assert_eq!(value.hour(), 8);
        assert_eq!(value.minute(), 5);
        assert_eq!(value.minutes_since_midnight(), 485);
        assert_eq!(value.to_string(), "08:05");
    }

    #[test]
    fn test_hh_mm_rejects_malformed_value() {
        let result = serde_json::from_str::<HhMm>(r#""8:5""#);
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
        assert!(matches!(result, Err(ScheduleValidationError::EmptyRange)));
    }

    #[test]
    fn test_day_auto_run_slot_deserialize_accepts_camel_case_aliases() {
        let slot: DayAutoRunSlot =
            serde_json::from_str(r#"{"enabled":true,"startTime":"08:00","endTime":"17:00"}"#)
                .expect("day slot should deserialize from camelCase aliases");
        assert!(slot.enabled());
        assert_eq!(slot.start_time().to_string(), "08:00");
        assert_eq!(slot.end_time().to_string(), "17:00");
    }

    #[test]
    fn test_day_auto_run_slot_deserialize_rejects_missing_fields() {
        let result = serde_json::from_str::<DayAutoRunSlot>(r#"{"enabled":true}"#);
        assert!(result.is_err());
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
    fn test_weekly_auto_run_config_deserialize_rejects_empty_object() {
        let result = serde_json::from_str::<WeeklyAutoRunConfig>("{}");
        assert!(result.is_err());
    }

    #[test]
    fn test_weekly_auto_run_config_deserialize_rejects_missing_weekday() {
        let result = serde_json::from_value::<WeeklyAutoRunConfig>(json!({
            "mon": {"enabled": false, "startTime": "09:00", "endTime": "18:00"},
            "tue": {"enabled": false, "startTime": "09:00", "endTime": "18:00"},
            "wed": {"enabled": false, "startTime": "09:00", "endTime": "18:00"},
            "thu": {"enabled": false, "startTime": "09:00", "endTime": "18:00"},
            "fri": {"enabled": false, "startTime": "09:00", "endTime": "18:00"},
            "sat": {"enabled": false, "startTime": "09:00", "endTime": "18:00"}
        }));
        assert!(result.is_err());
    }

    #[test]
    fn test_weekly_auto_run_config_try_new_rejects_overnight_overlap() {
        let result = WeeklyAutoRunConfig::try_new(
            enabled_slot("22:00", "02:00"),
            enabled_slot("01:00", "05:00"),
            DayAutoRunSlot::default(),
            DayAutoRunSlot::default(),
            DayAutoRunSlot::default(),
            DayAutoRunSlot::default(),
            DayAutoRunSlot::default(),
        );

        assert!(matches!(
            result,
            Err(ScheduleValidationError::Overlap {
                primary_weekday: "mon",
                secondary_weekday: "tue",
            })
        ));
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

        let Err(ScheduleValidationError::Overlap {
            primary_weekday,
            secondary_weekday,
        }) = result
        else {
            panic!("Sunday overnight and Monday early morning should overlap");
        };
        assert!(matches!(
            (primary_weekday, secondary_weekday),
            ("sun", "mon") | ("mon", "sun")
        ));
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
    fn test_duration_newtypes_reject_out_of_range_values() {
        assert!(serde_json::from_value::<WorkDurationSecs>(json!(0)).is_err());
        assert!(serde_json::from_value::<ShortBreakDurationSecs>(json!(90_000)).is_err());
        assert!(serde_json::from_value::<LongBreakDurationSecs>(json!(90_000)).is_err());
        assert!(serde_json::from_value::<LongBreakIntervalCount>(json!(0)).is_err());
        assert!(serde_json::from_value::<LongBreakIntervalCount>(json!(101)).is_err());
    }

    #[test]
    fn test_timer_state_default() {
        let state = TimerState::default();
        assert_eq!(state.status, TimerStatus::Idle);
        assert_eq!(state.session_type, SessionType::Work);
        assert_eq!(state.remaining_seconds, 25 * 60);
        assert!(!state.auto_run_feature_enabled);
        assert_eq!(state.weekly_auto_run_config, WeeklyAutoRunConfig::default());
        assert!(state.timer_start_source.is_none());
        assert_eq!(state.scheduled_stop_state, ScheduledStopState::None);
    }

    #[test]
    fn test_timer_settings_default() {
        let settings = TimerSettings::default();
        assert_eq!(settings.work_duration_secs.get(), 25 * 60);
        assert_eq!(settings.short_break_duration_secs.get(), 5 * 60);
        assert_eq!(settings.long_break_duration_secs.get(), 15 * 60);
        assert_eq!(settings.long_break_interval.get(), 4);
        assert_eq!(
            settings.post_break_transition_policy,
            PostBreakTransitionPolicy::RequireManualContinue
        );
    }

    #[test]
    fn test_timer_start_source_default() {
        assert_eq!(
            TimerStartSource::default(),
            TimerStartSource::ManualOutsideWindow
        );
    }

    #[test]
    fn test_scheduled_stop_state_default() {
        assert_eq!(ScheduledStopState::default(), ScheduledStopState::None);
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
    fn test_boot_prompt_runtime_state_default() {
        assert_eq!(
            BootPromptRuntimeState::default(),
            BootPromptRuntimeState::NotApplicable
        );
    }

    #[test]
    fn test_boot_prompt_runtime_state_serializes_as_ipc_contract_values() {
        let cases = [
            (
                BootPromptRuntimeState::AwaitingDecision,
                json!("awaiting_decision"),
            ),
            (
                BootPromptRuntimeState::ResolvedStart,
                json!("resolved_start"),
            ),
            (
                BootPromptRuntimeState::ResolvedLater,
                json!("resolved_later"),
            ),
            (
                BootPromptRuntimeState::NotApplicable,
                json!("not_applicable"),
            ),
        ];

        for (runtime_state, expected) in cases {
            let serialized =
                serde_json::to_value(runtime_state).expect("runtime state should serialize");
            assert_eq!(serialized, expected);
        }
    }

    #[test]
    fn test_enum_fields_deserialize_with_serde_default() {
        #[derive(Debug, Deserialize)]
        struct EnumDefaultsWrapper {
            #[serde(default)]
            timer_start_source: TimerStartSource,
            #[serde(default)]
            scheduled_stop_state: ScheduledStopState,
            #[serde(default)]
            boot_prompt_runtime_state: BootPromptRuntimeState,
            #[serde(default)]
            post_break_transition_policy: PostBreakTransitionPolicy,
        }

        let wrapper: EnumDefaultsWrapper =
            serde_json::from_str("{}").expect("enum defaults wrapper should deserialize");

        assert_eq!(
            wrapper.timer_start_source,
            TimerStartSource::ManualOutsideWindow
        );
        assert_eq!(wrapper.scheduled_stop_state, ScheduledStopState::None);
        assert_eq!(
            wrapper.boot_prompt_runtime_state,
            BootPromptRuntimeState::NotApplicable
        );
        assert_eq!(
            wrapper.post_break_transition_policy,
            PostBreakTransitionPolicy::RequireManualContinue
        );
    }

    #[test]
    fn test_session_transition_outcome_work_to_break_contract() {
        let outcome = SessionTransitionOutcome::work_to_break(SessionType::LongBreak);

        assert_eq!(
            outcome.transition_reason,
            SessionTransitionReason::WorkToBreak
        );
        assert_eq!(outcome.completed_session_type, SessionType::Work);
        assert_eq!(outcome.next_status, TimerStatus::OnBreak);
        assert_eq!(outcome.next_session_type, SessionType::LongBreak);
        assert!(!outcome.requires_manual_continue_decision);
    }

    #[test]
    fn test_session_completed_payload_from_outcome_maps_manual_decision_flag() {
        let outcome = SessionTransitionOutcome::break_waiting_decision(SessionType::ShortBreak);
        let payload = SessionCompletedPayload::from_transition_outcome(&outcome);

        assert_eq!(
            payload.transition_reason,
            SessionTransitionReason::BreakWaitingDecision
        );
        assert_eq!(payload.completed_session_type, SessionType::ShortBreak);
        assert_eq!(payload.next_status, TimerStatus::Idle);
        assert_eq!(payload.next_session_type, SessionType::Work);
        assert!(payload.requires_manual_continue_decision);
        assert!(payload.post_break_decision_pending);
    }
}
