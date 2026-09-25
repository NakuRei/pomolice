// Event-sequence tests. Every scenario feeds events with explicit `now`
// values into a `TimerCore` and checks what the outside world can observe:
// the payloads it publishes, the session completions it announces, what it
// persists, and which effects it asks for.
//
// Reading guide:
// - `workday()` is a Monday schedule from 09:00 to 18:00; `monday(h, m, s)`
//   builds a local time on that Monday.
// - `Durations { .. }` names the timer settings a scenario cares about;
//   everything not mentioned keeps the application defaults, and
//   `default_work_secs()` / `default_short_break_secs()` name those defaults.
// - Break-lock outcomes (`BreakLockEntered` etc.) are fed back by hand,
//   playing the role the shell has in production.

mod active_window;
mod boot_prompt;
mod completion;
mod countdown;
mod emergency_stop_and_escape;
mod guards;
mod restore;
mod schedule;

use super::*;
use crate::error::PomoliceError;
use crate::types::{
    BreakAttendance, DayAutoRunSlot, HhMm, PostBreakDecisionAction, PostBreakTransitionPolicy,
    ScheduleStopState, SessionTransitionReason, WeeklyAutoRunConfig,
    PERSISTED_STATE_SCHEMA_VERSION,
};
use chrono::{Duration as ChronoDuration, TimeZone};

// ---- time ----

/// 2025-02-03 is a Monday.
fn monday(hour: u32, minute: u32, second: u32) -> DateTime<Local> {
    Local
        .with_ymd_and_hms(2025, 2, 3, hour, minute, second)
        .single()
        .expect("valid local datetime")
}

fn seconds_before(time: DateTime<Local>, seconds: i64) -> DateTime<Local> {
    time - ChronoDuration::seconds(seconds)
}

fn seconds_after(time: DateTime<Local>, seconds: i64) -> DateTime<Local> {
    time + ChronoDuration::seconds(seconds)
}

fn millis_after(time: DateTime<Local>, millis: i64) -> DateTime<Local> {
    time + ChronoDuration::milliseconds(millis)
}

fn millis_before(time: DateTime<Local>, millis: i64) -> DateTime<Local> {
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

fn workday_start() -> DateTime<Local> {
    monday(9, 0, 0)
}

fn workday_end() -> DateTime<Local> {
    monday(18, 0, 0)
}

/// Enables the schedule with `config`. The post-break policy is sent along
/// because the event carries all automation policies; it stays at the
/// default (manual continue). Tests about the policy set it explicitly.
fn schedule_enabled(config: WeeklyAutoRunConfig) -> Event {
    Event::AutomationPoliciesChanged {
        auto_run_feature_enabled: true,
        weekly_auto_run_config: config,
        post_break_transition_policy: PostBreakTransitionPolicy::RequireManualContinue,
    }
}

/// Disables the schedule; the post-break policy stays at the default.
fn schedule_disabled() -> Event {
    Event::AutomationPoliciesChanged {
        auto_run_feature_enabled: false,
        weekly_auto_run_config: workday(),
        post_break_transition_policy: PostBreakTransitionPolicy::RequireManualContinue,
    }
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
        let defaults = TimerSettings::default();
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

/// A break loses exactly one second per tick the core observes. This is the
/// specification, not a tunable: the runtime ticks once a second.
const SECONDS_PER_TICK: u64 = 1;

fn timer_settings(durations: Durations) -> TimerSettings {
    TimerSettings::try_from_raw(
        durations.work,
        durations.short_break,
        durations.long_break,
        durations.long_break_interval,
        durations.policy,
    )
    .expect("valid settings")
}

fn durations_changed(durations: Durations) -> Event {
    let settings = timer_settings(durations);
    Event::TimerDurationSettingsChanged {
        work_duration_secs: settings.work_duration_secs,
        short_break_duration_secs: settings.short_break_duration_secs,
        long_break_duration_secs: settings.long_break_duration_secs,
        long_break_interval: settings.long_break_interval,
    }
}

// ---- driving the core ----

fn apply(core: &mut TimerCore, event: Event, now: DateTime<Local>) -> Vec<Effect> {
    core.apply(event, now).expect("event should be accepted")
}

fn boot_prompt_is_offered(core: &TimerCore, now: DateTime<Local>) -> bool {
    core.full_state_payload(now)
        .boot_auto_start_prompt_window
        .is_some()
}

/// Plays the frontend's first fetch of the state and, if the boot prompt is
/// offered, answers it with "later".
fn dismiss_boot_prompt(core: &mut TimerCore, now: DateTime<Local>) {
    apply(core, Event::FirstFullStateObserved, now);
    if boot_prompt_is_offered(core, now) {
        apply(core, Event::BootPromptResolved { start_now: false }, now);
    }
    assert!(!boot_prompt_is_offered(core, now));
}

/// An idle timer whose schedule is enabled and whose boot prompt is out of
/// the way, so scheduled starts are free to fire.
fn idle_with_schedule(now: DateTime<Local>, config: WeeklyAutoRunConfig) -> TimerCore {
    let mut core = TimerCore::restore(None, now);
    apply(&mut core, schedule_enabled(config), now);
    dismiss_boot_prompt(&mut core, now);
    core
}

fn idle_without_schedule(now: DateTime<Local>) -> TimerCore {
    TimerCore::restore(None, now)
}

fn start_work(core: &mut TimerCore, now: DateTime<Local>, settings: TimerSettings) {
    apply(core, Event::StartWorkRequested { settings }, now);
    assert_eq!(core.status(), TimerStatus::Working);
}

/// Starts a one-second work session and ticks it into its break at
/// `now + 1s`. Only the break-related fields of `break_durations` matter;
/// its work duration is replaced by that one second.
fn reach_break_after_one_second_of_work(
    core: &mut TimerCore,
    now: DateTime<Local>,
    break_durations: Durations,
) {
    start_work(
        core,
        now,
        timer_settings(Durations {
            work: 1,
            ..break_durations
        }),
    );
    apply(core, Event::Tick, seconds_after(now, 1));
    assert_eq!(core.status(), TimerStatus::OnBreak);
}

fn saved_state(
    status: TimerStatus,
    remaining_seconds: u64,
    saved_at: DateTime<Local>,
) -> PersistedState {
    PersistedState {
        schema_version: PERSISTED_STATE_SCHEMA_VERSION,
        status,
        remaining_seconds,
        session_type: SessionType::Work,
        current_cycle: 1,
        completed_pomodoros: 0,
        post_break_decision_pending: false,
        settings: TimerSettings::default(),
        saved_at: saved_at.with_timezone(&Utc),
        auto_run_feature_enabled: false,
        weekly_auto_run_config: WeeklyAutoRunConfig::default(),
        schedule_stop_state: ScheduleStopState::NotControlledBySchedule,
    }
}

// ---- reading the results ----

fn remaining(core: &TimerCore, now: DateTime<Local>) -> u64 {
    core.tick_payload(now).remaining_seconds
}

fn published_states(effects: &[Effect]) -> Vec<&TimerTickPayload> {
    effects
        .iter()
        .filter_map(|effect| match effect {
            Effect::PublishTimerState(payload) => Some(payload),
            _ => None,
        })
        .collect()
}

/// The state the frontend is left with after these effects.
fn last_published(effects: &[Effect]) -> &TimerTickPayload {
    published_states(effects)
        .last()
        .copied()
        .expect("a state should have been published")
}

fn starts_work(effects: &[Effect]) -> bool {
    published_states(effects)
        .iter()
        .any(|payload| payload.status == TimerStatus::Working)
}

fn completion_reason(effects: &[Effect]) -> Option<SessionTransitionReason> {
    effects.iter().find_map(|effect| match effect {
        Effect::EmitSessionCompleted(payload) => Some(payload.transition_reason),
        _ => None,
    })
}
