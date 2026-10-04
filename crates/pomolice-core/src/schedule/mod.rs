pub(crate) mod boundary;
mod hh_mm;

pub use hh_mm::{HhMm, HhMmParseError};
use serde::Serialize;
use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq, Error)]
#[error("start_time and end_time of an enabled slot must differ")]
pub struct EmptyTimeRangeError;

#[derive(Debug, Clone, PartialEq, Eq, Error)]
#[error("enabled schedule windows overlap: {0:?}")]
pub struct OverlappingSlotsError(pub Vec<(WeekdayKey, WeekdayKey)>);

fn hh_mm_or_panic(hour: u8, minute: u8) -> HhMm {
    match HhMm::try_new(hour, minute) {
        Ok(value) => value,
        Err(error) => panic!("default HH:MM should be valid: {error}"),
    }
}

/// Monday-first weekday keys shared by the schedule and its validation issues.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "snake_case")]
pub enum WeekdayKey {
    Mon,
    Tue,
    Wed,
    Thu,
    Fri,
    Sat,
    Sun,
}

impl WeekdayKey {
    pub const ALL: [Self; 7] = [
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
pub struct DayAutoRunSlot {
    enabled: bool,
    start_time: HhMm,
    end_time: HhMm,
}

impl DayAutoRunSlot {
    /// A disabled slot keeps its times only so they can be edited later, so
    /// it is never checked for an empty range.
    /// # Errors
    /// Rejects an enabled slot whose start and end are equal.
    pub fn try_new(
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

    #[must_use]
    pub fn enabled(self) -> bool {
        self.enabled
    }

    #[must_use]
    pub fn start_time(self) -> HhMm {
        self.start_time
    }

    #[must_use]
    pub fn end_time(self) -> HhMm {
        self.end_time
    }

    #[must_use]
    pub fn crosses_midnight(self) -> bool {
        self.start_time.minutes_since_midnight() > self.end_time.minutes_since_midnight()
    }

    #[must_use]
    pub fn parsed_time_range(self) -> (u32, u32, bool) {
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
pub struct WeeklyAutoRunConfig {
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
    /// # Errors
    /// Reports every pair of overlapping enabled slots, including week wraparound.
    pub fn try_new(
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

    #[must_use]
    pub fn slot(&self, weekday: WeekdayKey) -> DayAutoRunSlot {
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

    #[must_use]
    pub fn mon(&self) -> DayAutoRunSlot {
        self.mon
    }

    #[must_use]
    pub fn tue(&self) -> DayAutoRunSlot {
        self.tue
    }

    #[must_use]
    pub fn wed(&self) -> DayAutoRunSlot {
        self.wed
    }

    #[must_use]
    pub fn thu(&self) -> DayAutoRunSlot {
        self.thu
    }

    #[must_use]
    pub fn fri(&self) -> DayAutoRunSlot {
        self.fri
    }

    #[must_use]
    pub fn sat(&self) -> DayAutoRunSlot {
        self.sat
    }

    #[must_use]
    pub fn sun(&self) -> DayAutoRunSlot {
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

#[cfg(test)]
mod tests;
