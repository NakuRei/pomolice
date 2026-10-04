// Event-sequence tests. Every scenario feeds events with explicit `now`
// values into a `TimerCore` and checks what the outside world can observe:
// the payloads it publishes, the session completions it announces, what it
// persists, and which effects it asks for.
//
// Reading guide:
// - `workday()` is a Monday schedule from 09:00 to 18:00; `monday(h, m, s)`
//   builds a UTC time on that Monday.
// - `Durations { .. }` names the timer settings a scenario cares about;
//   everything not mentioned keeps the application defaults, and
//   `default_work_secs()` / `default_short_break_secs()` name those defaults.
// - Settings reach the core only as a whole (`Event::SettingsChanged`), so
//   helpers such as `enable_schedule` and `start_work` change the settings the
//   core already holds and keep everything else.

mod active_window;
mod boot_prompt;
mod completion;
mod countdown;
mod emergency_stop_and_escape;
mod guards;
mod restore;
mod schedule;
mod settings_changes;

use super::*;
type TimerCore = super::TimerCore<chrono_tz::Tz>;
use crate::schedule::{DayAutoRunSlot, HhMm, WeeklyAutoRunConfig};
use crate::settings::UserSettings;
use crate::settings::{
    LongBreakDurationSecs, LongBreakIntervalCount, PostBreakTransitionPolicy,
    ShortBreakDurationSecs, WorkDurationSecs,
};
use crate::timer_core::observation::SessionTransitionReason;
use crate::timer_core::TimerError;
use crate::timer_core::{
    BreakAttendance, PostBreakDecisionAction, ScheduleStopState, SessionDurationSecs,
};
use chrono::{Duration as ChronoDuration, TimeZone};

// ---- time ----

/// 2025-02-03 is a Monday.
fn monday(hour: u32, minute: u32, second: u32) -> DateTime<Utc> {
    Utc.with_ymd_and_hms(2025, 2, 3, hour, minute, second)
        .single()
        .expect("valid local datetime")
}

fn seconds_before(time: DateTime<Utc>, seconds: i64) -> DateTime<Utc> {
    time - ChronoDuration::seconds(seconds)
}

fn seconds_after(time: DateTime<Utc>, seconds: i64) -> DateTime<Utc> {
    time + ChronoDuration::seconds(seconds)
}

fn millis_after(time: DateTime<Utc>, millis: i64) -> DateTime<Utc> {
    time + ChronoDuration::milliseconds(millis)
}

fn millis_before(time: DateTime<Utc>, millis: i64) -> DateTime<Utc> {
    time - ChronoDuration::milliseconds(millis)
}

// ---- schedule ----

fn hh_mm(hour: u8, minute: u8) -> HhMm {
    HhMm::try_new(hour, minute).expect("valid time")
}

/// A schedule with a single enabled slot on Monday.
fn monday_slot(start: (u8, u8), end: (u8, u8)) -> WeeklyAutoRunConfig {
    WeeklyAutoRunConfig::try_new(
        DayAutoRunSlot::try_new(true, hh_mm(start.0, start.1), hh_mm(end.0, end.1))
            .expect("valid slot"),
        DayAutoRunSlot::default(),
        DayAutoRunSlot::default(),
        DayAutoRunSlot::default(),
        DayAutoRunSlot::default(),
        DayAutoRunSlot::default(),
        DayAutoRunSlot::default(),
    )
    .expect("valid weekly config")
}

fn workday() -> WeeklyAutoRunConfig {
    monday_slot((9, 0), (18, 0))
}

fn workday_start() -> DateTime<Utc> {
    monday(9, 0, 0)
}

fn workday_end() -> DateTime<Utc> {
    monday(18, 0, 0)
}

/// Saves settings with the schedule enabled on `config`; every other setting
/// keeps its current value.
fn enable_schedule(
    core: &mut TimerCore,
    config: WeeklyAutoRunConfig,
    now: DateTime<Utc>,
) -> Vec<Effect> {
    let settings = UserSettings {
        auto_run_feature_enabled: true,
        weekly_auto_run_config: config,
        ..core.settings.clone()
    };
    apply(core, Event::SettingsChanged { settings }, now)
}

/// Saves settings with the schedule disabled; every other setting keeps its
/// current value.
fn disable_schedule(core: &mut TimerCore, now: DateTime<Utc>) -> Vec<Effect> {
    let settings = UserSettings {
        auto_run_feature_enabled: false,
        ..core.settings.clone()
    };
    apply(core, Event::SettingsChanged { settings }, now)
}

// ---- settings ----

#[derive(Clone, Copy)]
struct Durations {
    work: u64,
    short_break: u64,
    long_break: u64,
    long_break_interval: u32,
    policy: PostBreakTransitionPolicy,
}

impl Default for Durations {
    fn default() -> Self {
        let defaults = UserSettings::default();
        Self {
            work: defaults.work_duration_secs.get(),
            short_break: defaults.short_break_duration_secs.get(),
            long_break: defaults.long_break_duration_secs.get(),
            long_break_interval: defaults.long_break_interval.get(),
            policy: defaults.post_break_transition_policy,
        }
    }
}

fn default_work_secs() -> u64 {
    Durations::default().work
}

fn default_short_break_secs() -> u64 {
    Durations::default().short_break
}

/// These scenarios credit one second per tick, matching the application ticker.
const SECONDS_PER_TICK: u64 = 1;

/// `settings` with the durations and policy replaced by `durations`.
fn with_durations(settings: UserSettings, durations: Durations) -> UserSettings {
    UserSettings {
        work_duration_secs: WorkDurationSecs::try_new(durations.work).expect("valid work"),
        short_break_duration_secs: ShortBreakDurationSecs::try_new(durations.short_break)
            .expect("valid short break"),
        long_break_duration_secs: LongBreakDurationSecs::try_new(durations.long_break)
            .expect("valid long break"),
        long_break_interval: LongBreakIntervalCount::try_new(durations.long_break_interval)
            .expect("valid interval"),
        post_break_transition_policy: durations.policy,
        ..settings
    }
}

/// Saves `durations`; every other setting keeps its current value.
fn change_durations(core: &mut TimerCore, durations: Durations, now: DateTime<Utc>) -> Vec<Effect> {
    let settings = with_durations(core.settings.clone(), durations);
    apply(core, Event::SettingsChanged { settings }, now)
}

// ---- driving the core ----

fn apply(core: &mut TimerCore, event: Event, now: DateTime<Utc>) -> Vec<Effect> {
    core.apply(event, now).expect("event should be accepted")
}

fn boot_prompt_is_offered(core: &TimerCore, now: DateTime<Utc>) -> bool {
    core.snapshot(now).boot_auto_start_offer.is_some()
}

fn idle_with_schedule(now: DateTime<Utc>, config: WeeklyAutoRunConfig) -> TimerCore {
    let mut core = idle_without_schedule(now);
    enable_schedule(&mut core, config, now);
    core
}

fn idle_without_schedule(now: DateTime<Utc>) -> TimerCore {
    restore_with_default_settings(None, now)
}

fn restore_with_default_settings(
    persisted: Option<TimerCheckpoint>,
    now: DateTime<Utc>,
) -> TimerCore {
    TimerCore::restore(persisted, UserSettings::default(), now, chrono_tz::UTC).0
}

/// Saves `durations` and starts a work session with them.
fn start_work(core: &mut TimerCore, now: DateTime<Utc>, durations: Durations) {
    change_durations(core, durations, now);
    apply(core, Event::StartWorkRequested, now);
    assert_eq!(core.status(), TimerStatus::Working);
}

/// Starts a one-second work session and ticks it into its break at
/// `now + 1s`. Only the break-related fields of `break_durations` matter;
/// its work duration is replaced by that one second.
fn reach_break_after_one_second_of_work(
    core: &mut TimerCore,
    now: DateTime<Utc>,
    break_durations: Durations,
) {
    start_work(
        core,
        now,
        Durations {
            work: 1,
            ..break_durations
        },
    );
    apply(
        core,
        Event::TimeObserved {
            credited_break_seconds: 1,
        },
        seconds_after(now, 1),
    );
    assert_eq!(core.status(), TimerStatus::OnBreak);
}

fn saved_state(
    status: TimerStatus,
    remaining_seconds: u64,
    saved_at: DateTime<Utc>,
) -> TimerCheckpoint {
    TimerCheckpoint {
        status,
        remaining_seconds,
        session_duration_secs: SessionDurationSecs::try_new(default_work_secs())
            .expect("valid session duration"),
        session_type: SessionType::Work,
        current_cycle: 1,
        completed_pomodoros: 0,
        post_break_decision_pending: false,
        saved_at: saved_at.with_timezone(&Utc),
        schedule_stop_state: ScheduleStopState::NotControlledBySchedule,
    }
}

// ---- reading the results ----

fn remaining(core: &TimerCore, now: DateTime<Utc>) -> u64 {
    core.snapshot(now).remaining_seconds
}

fn completion_reason(effects: &[Effect]) -> Option<SessionTransitionReason> {
    effects.iter().find_map(|effect| match effect {
        Effect::EmitSessionCompleted(payload) => Some(payload.transition_reason),
        _ => None,
    })
}
