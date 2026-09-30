// Weekly schedule boundary evaluation with pure functions.

use chrono::{
    DateTime, Datelike, Duration, Local, LocalResult, NaiveDate, NaiveDateTime, NaiveTime,
    TimeZone, Weekday,
};

use crate::ipc::responses::ActiveWindowSummaryPayload;
use crate::types::{BoundaryOccurrenceId, DayAutoRunSlot, WeeklyAutoRunConfig};

const MAX_BOUNDARY_LOOKBACK_DAYS: i64 = 7;
const MAX_BOUNDARY_OCCURRENCES_PER_TICK: usize = 128;
const MAX_NONEXISTENT_TIME_SEARCH_MINUTES: i64 = 360;
/// Active schedule window data for the current local time.
///
/// Usage example:
/// ```rust,ignore
/// let active_window = ActiveWindow {
///     slot_key: "mon".to_string(),
///     anchor_local_date: chrono::NaiveDate::from_ymd_opt(2025, 1, 6).unwrap(),
///     start_time: "09:00".to_string(),
///     end_time: "18:00".to_string(),
///     crosses_midnight: false,
/// };
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ActiveWindow {
    pub slot_key: String,
    pub anchor_local_date: NaiveDate,
    pub start_time: String,
    pub end_time: String,
    pub crosses_midnight: bool,
}

impl ActiveWindow {
    pub(crate) fn to_summary_payload(&self) -> ActiveWindowSummaryPayload {
        ActiveWindowSummaryPayload {
            weekday_key: match self.anchor_local_date.weekday() {
                Weekday::Mon => crate::types::WeekdayKey::Mon,
                Weekday::Tue => crate::types::WeekdayKey::Tue,
                Weekday::Wed => crate::types::WeekdayKey::Wed,
                Weekday::Thu => crate::types::WeekdayKey::Thu,
                Weekday::Fri => crate::types::WeekdayKey::Fri,
                Weekday::Sat => crate::types::WeekdayKey::Sat,
                Weekday::Sun => crate::types::WeekdayKey::Sun,
            },
            start_time: self.start_time.clone(),
            end_time: self.end_time.clone(),
            crosses_midnight: self.crosses_midnight,
        }
    }
}

#[derive(Clone, Copy)]
struct SlotDescriptor {
    slot_key: &'static str,
    weekday: Weekday,
    slot: DayAutoRunSlot,
}

#[derive(Clone, Copy)]
enum BoundaryType {
    Start,
    End,
}

impl BoundaryType {
    fn as_str(self) -> &'static str {
        match self {
            Self::Start => "start",
            Self::End => "end",
        }
    }
}

/// Returns the active schedule window if `now_local` is inside any enabled slot.
///
/// Semantics:
/// - Start boundary is inclusive.
/// - End boundary is exclusive.
/// - Overnight slots belong to their start weekday.
pub(crate) fn is_now_inside_schedule_window(
    now_local: DateTime<Local>,
    config: &WeeklyAutoRunConfig,
) -> Option<ActiveWindow> {
    let now_local_date = now_local.date_naive();
    let now_local_naive = now_local.naive_local();
    let mut active_windows: Vec<(NaiveDateTime, ActiveWindow)> = Vec::new();

    for descriptor in build_slot_descriptors(config) {
        if !descriptor.slot.enabled() {
            continue;
        }

        let (start_minutes, end_minutes, crosses_midnight) = descriptor.slot.parsed_time_range();

        let anchor_local_date =
            most_recent_weekday_on_or_before(now_local_date, descriptor.weekday);
        let Some(start_local_naive) = create_local_naive_datetime(anchor_local_date, start_minutes)
        else {
            continue;
        };
        let end_boundary_date = if crosses_midnight {
            anchor_local_date + Duration::days(1)
        } else {
            anchor_local_date
        };
        let Some(end_local_naive) = create_local_naive_datetime(end_boundary_date, end_minutes)
        else {
            continue;
        };

        if start_local_naive <= now_local_naive && now_local_naive < end_local_naive {
            active_windows.push((
                start_local_naive,
                ActiveWindow {
                    slot_key: descriptor.slot_key.to_string(),
                    anchor_local_date,
                    start_time: descriptor.slot.start_time().to_string(),
                    end_time: descriptor.slot.end_time().to_string(),
                    crosses_midnight,
                },
            ));
        }
    }

    active_windows.sort_by(|left, right| {
        right
            .0
            .cmp(&left.0)
            .then_with(|| left.1.slot_key.cmp(&right.1.slot_key))
    });

    active_windows.into_iter().next().map(|(_, window)| window)
}

/// Collects crossed start boundaries in chronological order.
///
/// The search range is capped to the latest 7 days.
pub(crate) fn collect_crossed_start_boundaries(
    previous_local: DateTime<Local>,
    now_local: DateTime<Local>,
    config: &WeeklyAutoRunConfig,
) -> Vec<BoundaryOccurrenceId> {
    collect_crossed_boundaries(
        &previous_local,
        &now_local,
        &Local,
        config,
        BoundaryType::Start,
    )
}

/// Collects crossed end boundaries in chronological order.
///
/// The search range is capped to the latest 7 days.
pub(crate) fn collect_crossed_end_boundaries(
    previous_local: DateTime<Local>,
    now_local: DateTime<Local>,
    config: &WeeklyAutoRunConfig,
) -> Vec<BoundaryOccurrenceId> {
    collect_crossed_boundaries(
        &previous_local,
        &now_local,
        &Local,
        config,
        BoundaryType::End,
    )
}

fn collect_crossed_boundaries<Tz: TimeZone>(
    previous_local: &DateTime<Tz>,
    now_local: &DateTime<Tz>,
    timezone: &Tz,
    config: &WeeklyAutoRunConfig,
    boundary_type: BoundaryType,
) -> Vec<BoundaryOccurrenceId> {
    if now_local <= previous_local {
        return Vec::new();
    }

    let earliest_allowed_previous =
        now_local.to_owned() - Duration::days(MAX_BOUNDARY_LOOKBACK_DAYS);
    let effective_previous_local = if previous_local < &earliest_allowed_previous {
        earliest_allowed_previous
    } else {
        previous_local.to_owned()
    };

    let previous_epoch_seconds = effective_previous_local.timestamp();
    let now_epoch_seconds = now_local.timestamp();
    let anchor_range_start = effective_previous_local.date_naive() - Duration::days(1);
    let anchor_range_end = now_local.date_naive() + Duration::days(1);

    let mut crossed_boundaries: Vec<(i64, BoundaryOccurrenceId)> = Vec::new();

    for descriptor in build_slot_descriptors(config) {
        if !descriptor.slot.enabled() {
            continue;
        }

        let (start_minutes, end_minutes, crosses_midnight) = descriptor.slot.parsed_time_range();

        let mut anchor_local_date =
            first_weekday_on_or_after(anchor_range_start, descriptor.weekday);

        while anchor_local_date <= anchor_range_end {
            let boundary_local_naive = match boundary_type {
                BoundaryType::Start => {
                    create_local_naive_datetime(anchor_local_date, start_minutes)
                }
                BoundaryType::End => {
                    let end_boundary_date = if crosses_midnight {
                        anchor_local_date + Duration::days(1)
                    } else {
                        anchor_local_date
                    };
                    create_local_naive_datetime(end_boundary_date, end_minutes)
                }
            };

            let Some(boundary_local_naive) = boundary_local_naive else {
                anchor_local_date += Duration::days(7);
                continue;
            };

            for boundary_epoch_seconds in
                collect_boundary_epoch_candidates(timezone, boundary_local_naive)
            {
                if previous_epoch_seconds < boundary_epoch_seconds
                    && boundary_epoch_seconds <= now_epoch_seconds
                {
                    crossed_boundaries.push((
                        boundary_epoch_seconds,
                        build_boundary_occurrence_id(
                            descriptor.slot_key,
                            boundary_type,
                            boundary_epoch_seconds,
                        ),
                    ));
                }
            }

            anchor_local_date += Duration::days(7);
        }
    }

    crossed_boundaries.sort_by(|left, right| {
        left.0
            .cmp(&right.0)
            .then_with(|| left.1.as_str().cmp(right.1.as_str()))
    });
    crossed_boundaries.dedup_by(|left, right| left.1 == right.1);

    crossed_boundaries
        .into_iter()
        .take(MAX_BOUNDARY_OCCURRENCES_PER_TICK)
        .map(|(_, boundary_id)| boundary_id)
        .collect()
}

fn build_boundary_occurrence_id(
    slot_key: &str,
    boundary_type: BoundaryType,
    boundary_epoch_seconds: i64,
) -> BoundaryOccurrenceId {
    BoundaryOccurrenceId::for_schedule_boundary(
        slot_key,
        boundary_type.as_str(),
        boundary_epoch_seconds,
    )
}

fn build_slot_descriptors(config: &WeeklyAutoRunConfig) -> [SlotDescriptor; 7] {
    [
        SlotDescriptor {
            slot_key: "mon",
            weekday: Weekday::Mon,
            slot: config.mon(),
        },
        SlotDescriptor {
            slot_key: "tue",
            weekday: Weekday::Tue,
            slot: config.tue(),
        },
        SlotDescriptor {
            slot_key: "wed",
            weekday: Weekday::Wed,
            slot: config.wed(),
        },
        SlotDescriptor {
            slot_key: "thu",
            weekday: Weekday::Thu,
            slot: config.thu(),
        },
        SlotDescriptor {
            slot_key: "fri",
            weekday: Weekday::Fri,
            slot: config.fri(),
        },
        SlotDescriptor {
            slot_key: "sat",
            weekday: Weekday::Sat,
            slot: config.sat(),
        },
        SlotDescriptor {
            slot_key: "sun",
            weekday: Weekday::Sun,
            slot: config.sun(),
        },
    ]
}

fn create_local_naive_datetime(date: NaiveDate, total_minutes: u32) -> Option<NaiveDateTime> {
    let hour = total_minutes / 60;
    let minute = total_minutes % 60;
    let time = NaiveTime::from_hms_opt(hour, minute, 0)?;
    Some(date.and_time(time))
}

fn most_recent_weekday_on_or_before(
    reference_date: NaiveDate,
    target_weekday: Weekday,
) -> NaiveDate {
    let current = i64::from(reference_date.weekday().num_days_from_monday());
    let target = i64::from(target_weekday.num_days_from_monday());
    let offset_days = (current - target + 7) % 7;
    reference_date - Duration::days(offset_days)
}

fn first_weekday_on_or_after(reference_date: NaiveDate, target_weekday: Weekday) -> NaiveDate {
    let current = i64::from(reference_date.weekday().num_days_from_monday());
    let target = i64::from(target_weekday.num_days_from_monday());
    let offset_days = (target - current + 7) % 7;
    reference_date + Duration::days(offset_days)
}

fn collect_boundary_epoch_candidates<Tz: TimeZone>(
    timezone: &Tz,
    boundary_local_naive: NaiveDateTime,
) -> Vec<i64> {
    match timezone.from_local_datetime(&boundary_local_naive) {
        LocalResult::Single(boundary_local) => vec![boundary_local.timestamp()],
        LocalResult::Ambiguous(first, second) => {
            let mut epoch_candidates = vec![first.timestamp(), second.timestamp()];
            epoch_candidates.sort_unstable();
            epoch_candidates.dedup();
            epoch_candidates
        }
        LocalResult::None => resolve_nonexistent_boundary_epoch(timezone, boundary_local_naive)
            .into_iter()
            .collect(),
    }
}

fn resolve_nonexistent_boundary_epoch<Tz: TimeZone>(
    timezone: &Tz,
    boundary_local_naive: NaiveDateTime,
) -> Option<i64> {
    find_earliest_epoch_on_or_after(timezone, boundary_local_naive)
        .or_else(|| find_latest_epoch_on_or_before(timezone, boundary_local_naive))
}

fn find_earliest_epoch_on_or_after<Tz: TimeZone>(
    timezone: &Tz,
    reference_local_naive: NaiveDateTime,
) -> Option<i64> {
    for minute_offset in 0..=MAX_NONEXISTENT_TIME_SEARCH_MINUTES {
        let candidate_local_naive = reference_local_naive + Duration::minutes(minute_offset);
        match timezone.from_local_datetime(&candidate_local_naive) {
            LocalResult::Single(candidate_local) => return Some(candidate_local.timestamp()),
            LocalResult::Ambiguous(first, second) => {
                return Some(first.timestamp().min(second.timestamp()));
            }
            LocalResult::None => {}
        }
    }
    None
}

fn find_latest_epoch_on_or_before<Tz: TimeZone>(
    timezone: &Tz,
    reference_local_naive: NaiveDateTime,
) -> Option<i64> {
    for minute_offset in 0..=MAX_NONEXISTENT_TIME_SEARCH_MINUTES {
        let candidate_local_naive = reference_local_naive - Duration::minutes(minute_offset);
        match timezone.from_local_datetime(&candidate_local_naive) {
            LocalResult::Single(candidate_local) => return Some(candidate_local.timestamp()),
            LocalResult::Ambiguous(first, second) => {
                return Some(first.timestamp().max(second.timestamp()));
            }
            LocalResult::None => {}
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::HhMm;
    use chrono::{NaiveDate, Timelike};
    use chrono_tz::Tz;

    fn local_datetime(
        year: i32,
        month: u32,
        day: u32,
        hour: u32,
        minute: u32,
        second: u32,
    ) -> DateTime<Local> {
        match Local.with_ymd_and_hms(year, month, day, hour, minute, second) {
            LocalResult::Single(datetime) => datetime,
            LocalResult::Ambiguous(first, _) => first,
            LocalResult::None => panic!("test local datetime should exist"),
        }
    }

    fn update_weekday_slot(
        config: &mut WeeklyAutoRunConfig,
        weekday_key: &str,
        enabled: bool,
        start_time: &str,
        end_time: &str,
    ) {
        let slot = DayAutoRunSlot::try_new(
            enabled,
            HhMm::try_from(start_time.to_string()).expect("start time should be valid"),
            HhMm::try_from(end_time.to_string()).expect("end time should be valid"),
        )
        .expect("slot should be valid");

        *config = match weekday_key {
            "mon" => WeeklyAutoRunConfig::try_new(
                slot,
                config.tue(),
                config.wed(),
                config.thu(),
                config.fri(),
                config.sat(),
                config.sun(),
            ),
            "tue" => WeeklyAutoRunConfig::try_new(
                config.mon(),
                slot,
                config.wed(),
                config.thu(),
                config.fri(),
                config.sat(),
                config.sun(),
            ),
            "wed" => WeeklyAutoRunConfig::try_new(
                config.mon(),
                config.tue(),
                slot,
                config.thu(),
                config.fri(),
                config.sat(),
                config.sun(),
            ),
            "thu" => WeeklyAutoRunConfig::try_new(
                config.mon(),
                config.tue(),
                config.wed(),
                slot,
                config.fri(),
                config.sat(),
                config.sun(),
            ),
            "fri" => WeeklyAutoRunConfig::try_new(
                config.mon(),
                config.tue(),
                config.wed(),
                config.thu(),
                slot,
                config.sat(),
                config.sun(),
            ),
            "sat" => WeeklyAutoRunConfig::try_new(
                config.mon(),
                config.tue(),
                config.wed(),
                config.thu(),
                config.fri(),
                slot,
                config.sun(),
            ),
            "sun" => WeeklyAutoRunConfig::try_new(
                config.mon(),
                config.tue(),
                config.wed(),
                config.thu(),
                config.fri(),
                config.sat(),
                slot,
            ),
            other => panic!("unsupported weekday key: {other}"),
        }
        .expect("updated weekly config should be valid");
    }

    fn schedule_boundary_id<Tz: TimeZone>(
        slot_key: &str,
        boundary_kind: &str,
        boundary_time: &DateTime<Tz>,
    ) -> BoundaryOccurrenceId {
        BoundaryOccurrenceId::for_schedule_boundary(
            slot_key,
            boundary_kind,
            boundary_time.timestamp(),
        )
    }

    fn new_york() -> Tz {
        chrono_tz::America::New_York
    }

    fn new_york_datetime(
        year: i32,
        month: u32,
        day: u32,
        hour: u32,
        minute: u32,
        second: u32,
    ) -> DateTime<Tz> {
        match new_york().with_ymd_and_hms(year, month, day, hour, minute, second) {
            LocalResult::Single(datetime) => datetime,
            LocalResult::Ambiguous(_, _) => panic!("test fixture datetime should be unambiguous"),
            LocalResult::None => panic!("test fixture datetime should exist"),
        }
    }

    fn new_york_naive_datetime(
        year: i32,
        month: u32,
        day: u32,
        hour: u32,
        minute: u32,
        second: u32,
    ) -> NaiveDateTime {
        NaiveDate::from_ymd_opt(year, month, day)
            .expect("test fixture date should be valid")
            .and_hms_opt(hour, minute, second)
            .expect("test fixture time should be valid")
    }

    fn new_york_nonexistent_naive_datetime(
        year: i32,
        month: u32,
        day: u32,
        hour: u32,
        minute: u32,
        second: u32,
    ) -> NaiveDateTime {
        let naive = new_york_naive_datetime(year, month, day, hour, minute, second);
        assert!(
            matches!(new_york().from_local_datetime(&naive), LocalResult::None),
            "test fixture should be nonexistent in New York local time"
        );
        naive
    }

    fn new_york_ambiguous_naive_datetime(
        year: i32,
        month: u32,
        day: u32,
        hour: u32,
        minute: u32,
        second: u32,
    ) -> (NaiveDateTime, DateTime<Tz>, DateTime<Tz>) {
        let naive = new_york_naive_datetime(year, month, day, hour, minute, second);
        let LocalResult::Ambiguous(first, second) = new_york().from_local_datetime(&naive) else {
            panic!("test fixture should be ambiguous in New York local time");
        };

        if first < second {
            (naive, first, second)
        } else {
            (naive, second, first)
        }
    }

    fn hh_mm_string(naive: NaiveDateTime) -> String {
        format!("{:02}:{:02}", naive.time().hour(), naive.time().minute())
    }

    #[test]
    fn test_is_now_inside_schedule_window_for_standard_daytime_slot() {
        let mut config = WeeklyAutoRunConfig::default();
        update_weekday_slot(&mut config, "mon", true, "09:00", "18:00");

        let now_local = local_datetime(2025, 1, 6, 10, 0, 0); // Monday
        let active_window =
            is_now_inside_schedule_window(now_local, &config).expect("window should be active");

        assert_eq!(active_window.slot_key, "mon");
        assert_eq!(
            active_window.anchor_local_date,
            NaiveDate::from_ymd_opt(2025, 1, 6).expect("valid date")
        );
        assert_eq!(active_window.start_time, "09:00");
        assert_eq!(active_window.end_time, "18:00");
        assert!(!active_window.crosses_midnight);
    }

    #[test]
    fn test_is_now_inside_schedule_window_for_overnight_slot() {
        let mut config = WeeklyAutoRunConfig::default();
        update_weekday_slot(&mut config, "mon", true, "22:00", "02:00");

        let now_local = local_datetime(2025, 1, 7, 1, 30, 0); // Tuesday
        let active_window =
            is_now_inside_schedule_window(now_local, &config).expect("window should be active");

        assert_eq!(active_window.slot_key, "mon");
        assert_eq!(
            active_window.anchor_local_date,
            NaiveDate::from_ymd_opt(2025, 1, 6).expect("valid date")
        );
        assert!(active_window.crosses_midnight);
    }

    #[test]
    fn test_is_now_inside_schedule_window_start_inclusive_end_exclusive() {
        let mut config = WeeklyAutoRunConfig::default();
        update_weekday_slot(&mut config, "mon", true, "09:00", "18:00");

        let at_start = local_datetime(2025, 1, 6, 9, 0, 0);
        assert!(is_now_inside_schedule_window(at_start, &config).is_some());

        let at_end = local_datetime(2025, 1, 6, 18, 0, 0);
        assert!(is_now_inside_schedule_window(at_end, &config).is_none());
    }

    #[test]
    fn test_collect_crossed_start_boundaries_returns_boundary_at_crossed_start_time() {
        let mut config = WeeklyAutoRunConfig::default();
        update_weekday_slot(&mut config, "mon", true, "09:00", "18:00");

        let previous_local = local_datetime(2025, 1, 6, 8, 59, 50);
        let now_local = local_datetime(2025, 1, 6, 9, 0, 10);
        let boundary_ids = collect_crossed_start_boundaries(previous_local, now_local, &config);

        assert_eq!(
            boundary_ids,
            vec![schedule_boundary_id(
                "mon",
                "start",
                &local_datetime(2025, 1, 6, 9, 0, 0)
            )]
        );
    }

    #[test]
    fn test_collect_crossed_end_boundaries_for_overnight_slot_returns_end_time_boundary() {
        let mut config = WeeklyAutoRunConfig::default();
        update_weekday_slot(&mut config, "mon", true, "22:00", "02:00");

        let previous_local = local_datetime(2025, 1, 7, 1, 59, 50); // Tuesday
        let now_local = local_datetime(2025, 1, 7, 2, 0, 10); // Tuesday
        let boundary_ids = collect_crossed_end_boundaries(previous_local, now_local, &config);

        assert_eq!(
            boundary_ids,
            vec![schedule_boundary_id(
                "mon",
                "end",
                &local_datetime(2025, 1, 7, 2, 0, 0)
            )]
        );
    }

    #[test]
    fn test_collect_crossed_end_boundaries_returns_boundary_at_standard_end_time() {
        let mut config = WeeklyAutoRunConfig::default();
        update_weekday_slot(&mut config, "mon", true, "09:00", "18:00");

        let previous_local = local_datetime(2025, 1, 6, 17, 59, 50);
        let now_local = local_datetime(2025, 1, 6, 18, 0, 10);
        let boundary_ids = collect_crossed_end_boundaries(previous_local, now_local, &config);

        assert_eq!(
            boundary_ids,
            vec![schedule_boundary_id(
                "mon",
                "end",
                &local_datetime(2025, 1, 6, 18, 0, 0)
            )]
        );
    }

    #[test]
    fn test_collect_crossed_end_boundaries_ignores_disabled_slot() {
        let mut config = WeeklyAutoRunConfig::default();
        update_weekday_slot(&mut config, "mon", false, "09:00", "18:00");

        let previous_local = local_datetime(2025, 1, 6, 17, 59, 50);
        let now_local = local_datetime(2025, 1, 6, 18, 0, 10);
        let boundary_ids = collect_crossed_end_boundaries(previous_local, now_local, &config);

        assert!(boundary_ids.is_empty());
    }

    #[test]
    fn test_collect_crossed_end_boundaries_returns_empty_when_time_is_not_forward() {
        let mut config = WeeklyAutoRunConfig::default();
        update_weekday_slot(&mut config, "mon", true, "09:00", "18:00");

        for (previous_local, now_local) in [
            (
                local_datetime(2025, 1, 6, 18, 0, 0),
                local_datetime(2025, 1, 6, 18, 0, 0),
            ),
            (
                local_datetime(2025, 1, 6, 18, 0, 10),
                local_datetime(2025, 1, 6, 18, 0, 0),
            ),
        ] {
            let boundary_ids = collect_crossed_end_boundaries(previous_local, now_local, &config);

            assert!(boundary_ids.is_empty());
        }
    }

    #[test]
    fn test_collect_crossed_start_boundaries_returns_time_sorted_boundaries() {
        let mut config = WeeklyAutoRunConfig::default();
        update_weekday_slot(&mut config, "mon", true, "09:00", "18:00");
        update_weekday_slot(&mut config, "tue", true, "09:00", "18:00");

        let previous_local = local_datetime(2025, 1, 6, 8, 59, 50); // Monday
        let now_local = local_datetime(2025, 1, 7, 9, 0, 10); // Tuesday
        let boundary_ids = collect_crossed_start_boundaries(previous_local, now_local, &config);

        assert_eq!(
            boundary_ids,
            vec![
                schedule_boundary_id("mon", "start", &local_datetime(2025, 1, 6, 9, 0, 0)),
                schedule_boundary_id("tue", "start", &local_datetime(2025, 1, 7, 9, 0, 0)),
            ]
        );
    }

    #[test]
    fn test_collect_crossed_start_boundaries_caps_lookback_to_recent_seven_days() {
        let mut config = WeeklyAutoRunConfig::default();
        update_weekday_slot(&mut config, "mon", true, "00:00", "00:01");
        update_weekday_slot(&mut config, "tue", true, "00:00", "00:01");
        update_weekday_slot(&mut config, "wed", true, "00:00", "00:01");
        update_weekday_slot(&mut config, "thu", true, "00:00", "00:01");
        update_weekday_slot(&mut config, "fri", true, "00:00", "00:01");
        update_weekday_slot(&mut config, "sat", true, "00:00", "00:01");
        update_weekday_slot(&mut config, "sun", true, "00:00", "00:01");

        let now_local = local_datetime(2025, 2, 1, 0, 0, 30);
        let previous_local = now_local - Duration::days(40);
        let boundary_ids = collect_crossed_start_boundaries(previous_local, now_local, &config);

        assert!(boundary_ids.len() <= 8);
        assert!(boundary_ids.len() <= MAX_BOUNDARY_OCCURRENCES_PER_TICK);

        let oldest_allowed_epoch = (now_local - Duration::days(8)).timestamp();
        for boundary_id in boundary_ids {
            assert!(
                boundary_id
                    .epoch_seconds()
                    .expect("boundary should expose epoch seconds")
                    >= oldest_allowed_epoch,
                "boundary should remain inside the bounded lookback window"
            );
        }
    }

    #[test]
    fn test_collect_crossed_start_boundaries_detects_nonexistent_dst_boundary() {
        let nonexistent_boundary = new_york_nonexistent_naive_datetime(2025, 3, 9, 2, 30, 0);
        let mut config = WeeklyAutoRunConfig::default();
        update_weekday_slot(
            &mut config,
            "sun",
            true,
            &hh_mm_string(nonexistent_boundary),
            "23:59",
        );

        let previous_local = new_york_datetime(2025, 3, 9, 1, 59, 59);
        let now_local = new_york_datetime(2025, 3, 9, 3, 0, 1);
        let expected_boundary = new_york_datetime(2025, 3, 9, 3, 0, 0);

        let boundary_ids = collect_crossed_boundaries(
            &previous_local,
            &now_local,
            &new_york(),
            &config,
            BoundaryType::Start,
        );

        assert_eq!(
            boundary_ids,
            vec![schedule_boundary_id("sun", "start", &expected_boundary)]
        );
    }

    #[test]
    fn test_collect_crossed_end_boundaries_detects_nonexistent_dst_boundary() {
        let nonexistent_boundary = new_york_nonexistent_naive_datetime(2025, 3, 9, 2, 30, 0);
        let mut config = WeeklyAutoRunConfig::default();
        update_weekday_slot(
            &mut config,
            "sun",
            true,
            "00:00",
            &hh_mm_string(nonexistent_boundary),
        );

        let previous_local = new_york_datetime(2025, 3, 9, 1, 59, 59);
        let now_local = new_york_datetime(2025, 3, 9, 3, 0, 1);
        let expected_boundary = new_york_datetime(2025, 3, 9, 3, 0, 0);

        let boundary_ids = collect_crossed_boundaries(
            &previous_local,
            &now_local,
            &new_york(),
            &config,
            BoundaryType::End,
        );

        assert_eq!(
            boundary_ids,
            vec![schedule_boundary_id("sun", "end", &expected_boundary)]
        );
    }

    #[test]
    fn test_collect_crossed_start_boundaries_detects_ambiguous_dst_boundaries() {
        let (ambiguous_boundary, first_occurrence, second_occurrence) =
            new_york_ambiguous_naive_datetime(2025, 11, 2, 1, 30, 0);
        let mut config = WeeklyAutoRunConfig::default();
        update_weekday_slot(
            &mut config,
            "sun",
            true,
            &hh_mm_string(ambiguous_boundary),
            "23:59",
        );

        let previous_local = first_occurrence - Duration::seconds(1);
        let now_local = second_occurrence + Duration::seconds(1);

        let boundary_ids = collect_crossed_boundaries(
            &previous_local,
            &now_local,
            &new_york(),
            &config,
            BoundaryType::Start,
        );

        assert_eq!(
            boundary_ids,
            vec![
                schedule_boundary_id("sun", "start", &first_occurrence),
                schedule_boundary_id("sun", "start", &second_occurrence),
            ]
        );
    }

    #[test]
    fn test_collect_crossed_end_boundaries_detects_ambiguous_dst_boundaries() {
        let (ambiguous_boundary, first_occurrence, second_occurrence) =
            new_york_ambiguous_naive_datetime(2025, 11, 2, 1, 30, 0);
        let mut config = WeeklyAutoRunConfig::default();
        update_weekday_slot(
            &mut config,
            "sun",
            true,
            "00:00",
            &hh_mm_string(ambiguous_boundary),
        );

        let previous_local = first_occurrence - Duration::seconds(1);
        let now_local = second_occurrence + Duration::seconds(1);

        let boundary_ids = collect_crossed_boundaries(
            &previous_local,
            &now_local,
            &new_york(),
            &config,
            BoundaryType::End,
        );

        assert_eq!(
            boundary_ids,
            vec![
                schedule_boundary_id("sun", "end", &first_occurrence),
                schedule_boundary_id("sun", "end", &second_occurrence),
            ]
        );
    }

    #[test]
    fn test_collect_crossed_start_boundaries_returns_all_boundaries_for_large_time_jump() {
        let mut config = WeeklyAutoRunConfig::default();
        update_weekday_slot(&mut config, "mon", true, "09:00", "18:00");
        update_weekday_slot(&mut config, "tue", true, "09:00", "18:00");
        update_weekday_slot(&mut config, "wed", true, "09:00", "18:00");
        update_weekday_slot(&mut config, "thu", true, "09:00", "18:00");
        update_weekday_slot(&mut config, "fri", true, "09:00", "18:00");

        let previous_local = local_datetime(2025, 1, 6, 8, 59, 50); // Monday
        let now_local = local_datetime(2025, 1, 10, 9, 0, 10); // Friday
        let boundary_ids = collect_crossed_start_boundaries(previous_local, now_local, &config);

        assert_eq!(
            boundary_ids,
            vec![
                schedule_boundary_id("mon", "start", &local_datetime(2025, 1, 6, 9, 0, 0)),
                schedule_boundary_id("tue", "start", &local_datetime(2025, 1, 7, 9, 0, 0)),
                schedule_boundary_id("wed", "start", &local_datetime(2025, 1, 8, 9, 0, 0)),
                schedule_boundary_id("thu", "start", &local_datetime(2025, 1, 9, 9, 0, 0)),
                schedule_boundary_id("fri", "start", &local_datetime(2025, 1, 10, 9, 0, 0)),
            ]
        );
    }
}
