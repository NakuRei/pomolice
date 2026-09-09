use crate::error::{PersistenceFailure, PomoliceError, Result};
use crate::external_adapters::clock::testing::FixedClock;
use crate::external_adapters::clock::Clock;
use crate::external_adapters::persistence::{PersistenceManager, SaveError};
use crate::external_adapters::window_lock::MainWindowRestorePlan;
use crate::external_adapters::window_lock::{WindowState, WindowStateManager};
use crate::state::AppState;
use crate::timer_state_transition::TimerStateManager;
use crate::types::{
    PostBreakDecisionAction, PostBreakTransitionPolicy, ScheduleStopState, SessionTransitionReason,
    SessionType, TimerSettings, TimerState, TimerStatus, TimerTickPayload, WorkStartOrigin,
};
use crate::work_start::{
    start_boot_prompt_work, start_manual_work, start_scheduler_work, StartWorkAttempt,
    StartWorkRuntimeEffects,
};
use chrono::{Local, TimeZone, Utc};
use std::collections::BTreeSet;
use std::path::PathBuf;
use std::sync::Arc;
use tauri::{PhysicalPosition, PhysicalSize};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AcceptedStartOrigin {
    Manual,
    BootPrompt,
    Scheduler,
}

#[derive(Debug, Clone, PartialEq)]
struct PublishedTimerState {
    status: TimerStatus,
    remaining_seconds: u64,
    session_type: SessionType,
}

impl PublishedTimerState {
    fn from_payload(payload: &TimerTickPayload) -> Self {
        Self {
            status: payload.status,
            remaining_seconds: payload.remaining_seconds,
            session_type: payload.session_type,
        }
    }
}

#[derive(Debug, Default, PartialEq)]
struct RecordedStartWorkEffects {
    timer_ticks: Vec<PublishedTimerState>,
    tray_updates: Vec<PublishedTimerState>,
    work_started_durations: Vec<u64>,
    timer_task_spawn_count: usize,
}

impl StartWorkRuntimeEffects for RecordedStartWorkEffects {
    fn emit_timer_tick(&mut self, tick_payload: &TimerTickPayload) {
        self.timer_ticks
            .push(PublishedTimerState::from_payload(tick_payload));
    }

    fn update_tray(&mut self, tick_payload: &TimerTickPayload) {
        self.tray_updates
            .push(PublishedTimerState::from_payload(tick_payload));
    }

    fn emit_work_started(&mut self, work_duration_secs: u64) {
        self.work_started_durations.push(work_duration_secs);
    }

    fn spawn_timer_task<'a>(
        &'a mut self,
        _state: &'a AppState,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = ()> + Send + 'a>> {
        self.timer_task_spawn_count += 1;
        Box::pin(async {})
    }
}

#[derive(Debug, PartialEq)]
struct RecordedBreakLockEffects {
    saved_restore_plan: MainWindowRestorePlan,
    default_restore_plan: MainWindowRestorePlan,
    offscreen_geometry_was_ignored: bool,
}

const RESTORABLE_MAIN_WINDOW_POSITION: PhysicalPosition<i32> = PhysicalPosition { x: 120, y: 80 };
const OFFSCREEN_MAIN_WINDOW_POSITION: PhysicalPosition<i32> = PhysicalPosition { x: -32000, y: 80 };
const CAPTURED_MAIN_WINDOW_SIZE: PhysicalSize<u32> = PhysicalSize {
    width: 1024,
    height: 768,
};
const DEFAULT_CENTERED_RESTORE_WIDTH: f64 = 800.0;
const DEFAULT_CENTERED_RESTORE_HEIGHT: f64 = 600.0;

fn fixed_clock() -> Arc<FixedClock> {
    let now_utc = Utc
        .with_ymd_and_hms(2025, 2, 1, 9, 0, 0)
        .single()
        .expect("test datetime should be valid");
    Arc::new(FixedClock::new(now_utc.with_timezone(&Local)))
}

fn as_clock_trait(clock: &Arc<FixedClock>) -> Arc<dyn Clock + Send + Sync> {
    Arc::<FixedClock>::clone(clock) as Arc<dyn Clock + Send + Sync>
}

async fn state_without_persistence_path() -> AppState {
    let clock = fixed_clock();
    let persistence = PersistenceManager::new(as_clock_trait(&clock), None);
    let (state, _) =
        AppState::build_startup_state_for_tests(as_clock_trait(&clock), persistence).await;
    state
}

fn capture_public_command_error_envelope(error: PomoliceError) -> serde_json::Value {
    serde_json::to_value(error).expect("public command error should serialize")
}

fn capture_invalid_command_input_envelope() -> serde_json::Value {
    crate::commands::error_response::public_command_argument_rejection_error().0
}

async fn record_accepted_start_work_effects(
    origin: AcceptedStartOrigin,
) -> Result<(StartWorkAttempt, RecordedStartWorkEffects)> {
    let state = state_without_persistence_path().await;
    let mut effects = RecordedStartWorkEffects::default();
    let attempt = match origin {
        AcceptedStartOrigin::Manual => {
            start_manual_work(
                &state,
                TimerSettings::default(),
                accepted_work_start_origin(origin),
                &mut effects,
            )
            .await?
        }
        AcceptedStartOrigin::BootPrompt => {
            assert!(state.record_initial_full_state_auto_start_prompt_decision(true));
            start_boot_prompt_work(&state, TimerSettings::default(), &mut effects).await?
        }
        AcceptedStartOrigin::Scheduler => {
            start_scheduler_work(
                &state,
                TimerStatus::Idle,
                TimerSettings::default(),
                &mut effects,
            )
            .await?
        }
    };

    Ok((attempt, effects))
}

fn timer_transition_manager_from_state(timer_state: TimerState) -> TimerStateManager {
    TimerStateManager::new(Some(timer_state))
}

async fn state_with_runtime_persistence_failure() -> AppState {
    let clock = fixed_clock();
    let persistence = PersistenceManager::new(as_clock_trait(&clock), Some(PathBuf::from(".")))
        .with_injected_save_error_hook(Arc::new(|| {
            Some(SaveError::Serialize {
                source: serde_json::from_str::<serde_json::Value>("{")
                    .expect_err("invalid JSON should fail to parse"),
            })
        }));
    let (state, _) =
        AppState::build_startup_state_for_tests(as_clock_trait(&clock), persistence).await;
    state
}

async fn execute_scheduler_auto_start(
    state: &AppState,
    timer_status_at_tick_start: TimerStatus,
) -> Result<(StartWorkAttempt, RecordedStartWorkEffects)> {
    let mut effects = RecordedStartWorkEffects::default();
    let attempt = start_scheduler_work(
        state,
        timer_status_at_tick_start,
        TimerSettings::default(),
        &mut effects,
    )
    .await?;

    Ok((attempt, effects))
}

fn record_break_lock_effects() -> RecordedBreakLockEffects {
    let restorable_state = WindowState::from_restorable_main_window_geometry(
        RESTORABLE_MAIN_WINDOW_POSITION,
        CAPTURED_MAIN_WINDOW_SIZE,
    );
    let offscreen_state = WindowState::from_restorable_main_window_geometry(
        OFFSCREEN_MAIN_WINDOW_POSITION,
        CAPTURED_MAIN_WINDOW_SIZE,
    );

    let mut saved_manager = WindowStateManager::new();
    let restorable_state = restorable_state.expect("test geometry should be restorable");
    saved_manager.set_saved_state(Some(restorable_state));
    let default_manager = WindowStateManager::new();

    RecordedBreakLockEffects {
        saved_restore_plan: saved_manager.main_window_restore_plan(),
        default_restore_plan: default_manager.main_window_restore_plan(),
        offscreen_geometry_was_ignored: offscreen_state.is_none(),
    }
}

fn accepted_work_start_origin(origin: AcceptedStartOrigin) -> WorkStartOrigin {
    match origin {
        AcceptedStartOrigin::Manual => WorkStartOrigin::ManualOutsideScheduleWindow,
        AcceptedStartOrigin::BootPrompt => WorkStartOrigin::ManualInsideScheduleWindow,
        AcceptedStartOrigin::Scheduler => WorkStartOrigin::ScheduleStartBoundary,
    }
}

fn persistence_failure(detail: &'static str) -> PersistenceFailure {
    PersistenceFailure::from_error(std::io::Error::other(detail))
}

fn default_work_duration_secs() -> u64 {
    TimerSettings::default().work_duration_secs.get()
}

fn command_error_cases() -> Vec<(PomoliceError, &'static str, &'static str, Vec<&'static str>)> {
    vec![
        (
            PomoliceError::Unauthorized("settings-window".to_string()),
            "unauthorized_window",
            "Unauthorized window",
            vec!["settings-window"],
        ),
        (
            PomoliceError::InvalidStateTransition {
                action: "start work from enforced break".to_string(),
                current_state: TimerStatus::OnBreak,
            },
            "invalid_state_transition",
            "Invalid state transition",
            vec!["start work from enforced break", "OnBreak"],
        ),
        (
            PomoliceError::RateLimited {
                remaining_ms: 12345,
            },
            "rate_limited",
            "Rate limited",
            vec!["12345"],
        ),
        (
            PomoliceError::WindowControl(
                "Failed to enter fullscreen for main: platform denied request".to_string(),
            ),
            "window_control_error",
            "Window control error",
            vec!["Failed to enter fullscreen", "platform denied request"],
        ),
        (
            PomoliceError::PersistenceSave(persistence_failure(
                "failed to write C:\\Users\\nak\\secret\\timer_state.json: disk full",
            )),
            "persistence_error",
            "Persistence error",
            vec!["C:\\Users\\nak", "timer_state.json", "disk full"],
        ),
    ]
}

fn assert_public_error_envelope(
    envelope: &serde_json::Value,
    expected_code: &str,
    expected_message: &str,
    hidden_fragments: &[&str],
) {
    let response = envelope
        .as_object()
        .expect("public command error should be an object");
    let actual_fields = response.keys().map(String::as_str).collect::<BTreeSet<_>>();

    assert_eq!(actual_fields, BTreeSet::from(["code", "message"]));
    assert_eq!(
        response.get("code").and_then(serde_json::Value::as_str),
        Some(expected_code)
    );
    assert_eq!(
        response.get("message").and_then(serde_json::Value::as_str),
        Some(expected_message)
    );

    let serialized = envelope.to_string();
    for hidden_fragment in hidden_fragments {
        assert!(
            !serialized.contains(hidden_fragment),
            "public command error leaked diagnostic fragment {hidden_fragment:?}"
        );
    }
}

fn assert_accepted_start_effects(
    effects: &RecordedStartWorkEffects,
    expected_work_duration_secs: u64,
) {
    assert_eq!(
        effects.timer_ticks,
        [PublishedTimerState {
            status: TimerStatus::Working,
            remaining_seconds: expected_work_duration_secs,
            session_type: SessionType::Work,
        }]
    );
    assert_eq!(effects.tray_updates, effects.timer_ticks);
    assert_eq!(
        effects.work_started_durations,
        [expected_work_duration_secs]
    );
    assert_eq!(effects.timer_task_spawn_count, 1);
}

fn assert_no_start_effects(effects: &RecordedStartWorkEffects) {
    assert!(effects.timer_ticks.is_empty());
    assert!(effects.tray_updates.is_empty());
    assert!(effects.work_started_durations.is_empty());
    assert_eq!(effects.timer_task_spawn_count, 0);
}

fn working_state() -> TimerState {
    TimerState {
        status: TimerStatus::Working,
        session_type: SessionType::Work,
        remaining_seconds: 0,
        current_cycle: 1,
        ..TimerState::default()
    }
}

fn short_break_state(policy: PostBreakTransitionPolicy) -> TimerState {
    TimerState {
        status: TimerStatus::OnBreak,
        session_type: SessionType::ShortBreak,
        remaining_seconds: 0,
        current_cycle: 1,
        settings: TimerSettings {
            post_break_transition_policy: policy,
            ..TimerSettings::default()
        },
        ..TimerState::default()
    }
}

#[test]
fn command_failures_return_stable_public_error_envelopes() {
    for (error, expected_code, expected_message, hidden_fragments) in command_error_cases() {
        let envelope = capture_public_command_error_envelope(error);

        assert_public_error_envelope(
            &envelope,
            expected_code,
            expected_message,
            &hidden_fragments,
        );
    }

    let envelope = capture_invalid_command_input_envelope();
    assert_public_error_envelope(
        &envelope,
        "invalid_command_input",
        "Invalid command input",
        &["invalid type", "work_duration_secs", "start_work"],
    );
}

#[tokio::test]
async fn accepted_work_start_origins_publish_the_same_state_and_notifications() {
    for origin in [
        AcceptedStartOrigin::Manual,
        AcceptedStartOrigin::BootPrompt,
        AcceptedStartOrigin::Scheduler,
    ] {
        let (attempt, effects) = record_accepted_start_work_effects(origin)
            .await
            .expect("accepted start should succeed");

        assert_eq!(attempt, StartWorkAttempt::Accepted);
        assert_accepted_start_effects(&effects, default_work_duration_secs());
    }
}

#[tokio::test]
async fn rejected_work_starts_publish_no_start_effects() {
    let state = state_with_runtime_persistence_failure().await;
    {
        let mut timer = state.timer_manager.lock().await;
        let _ = timer.apply_persisted_state(short_break_state(
            PostBreakTransitionPolicy::RequireManualContinue,
        ));
    }

    let mut manual_effects = RecordedStartWorkEffects::default();
    let manual_result = crate::work_start::start_manual_work(
        &state,
        TimerSettings::default(),
        accepted_work_start_origin(AcceptedStartOrigin::Manual),
        &mut manual_effects,
    )
    .await;

    assert!(matches!(
        manual_result,
        Err(PomoliceError::InvalidStateTransition { .. })
    ));
    assert_no_start_effects(&manual_effects);

    let (scheduler_attempt, scheduler_effects) =
        execute_scheduler_auto_start(&state, TimerStatus::Working)
            .await
            .expect("scheduler rejection should be nonfatal");

    assert_eq!(scheduler_attempt, StartWorkAttempt::NoOp);
    assert_no_start_effects(&scheduler_effects);
}

#[test]
fn timer_start_sets_work_session_snapshot() {
    let mut timer = timer_transition_manager_from_state(TimerState::default());
    let expected_work_duration_secs = default_work_duration_secs();

    let (payload, work_duration_secs) = timer
        .start_work(
            TimerSettings::default(),
            accepted_work_start_origin(AcceptedStartOrigin::Manual),
        )
        .expect("idle timer should start work");

    assert_eq!(work_duration_secs, expected_work_duration_secs);
    assert_eq!(payload.status, TimerStatus::Working);
    assert_eq!(payload.session_type, SessionType::Work);
    assert_eq!(payload.remaining_seconds, expected_work_duration_secs);
    assert_eq!(timer.status(), TimerStatus::Working);
}

#[test]
fn work_completion_enters_break_unless_scheduled_stop_is_pending() {
    let mut normal_timer = timer_transition_manager_from_state(working_state());

    let normal_completion = normal_timer.complete_work_session_transition();

    assert_eq!(
        normal_completion.transition_reason,
        SessionTransitionReason::WorkToBreak
    );
    assert_eq!(normal_timer.status(), TimerStatus::OnBreak);
    assert_eq!(
        normal_timer.get_state().session_type,
        SessionType::ShortBreak
    );

    let mut stopped_timer = timer_transition_manager_from_state(TimerState {
        schedule_stop_state: ScheduleStopState::StopAtNextSessionBoundary,
        ..working_state()
    });

    let stopped_completion = stopped_timer.complete_work_session_transition();

    assert_eq!(
        stopped_completion.transition_reason,
        SessionTransitionReason::ScheduledStopExecuted
    );
    assert_eq!(stopped_timer.status(), TimerStatus::Idle);
    assert_eq!(
        stopped_timer.get_state().schedule_stop_state,
        ScheduleStopState::NotControlledBySchedule
    );
}

#[test]
fn break_completion_preserves_manual_wait_and_auto_continue_policies() {
    let mut manual_timer = timer_transition_manager_from_state(short_break_state(
        PostBreakTransitionPolicy::RequireManualContinue,
    ));

    let manual_completion = manual_timer.complete_break_session_transition();

    assert_eq!(
        manual_completion.transition_reason,
        SessionTransitionReason::BreakWaitingDecision
    );
    assert_eq!(manual_timer.status(), TimerStatus::Idle);
    assert!(manual_timer.get_state().post_break_decision_pending);

    let mut automatic_timer = timer_transition_manager_from_state(short_break_state(
        PostBreakTransitionPolicy::StartNextWorkAutomatically,
    ));

    let automatic_completion = automatic_timer.complete_break_session_transition();

    assert_eq!(
        automatic_completion.transition_reason,
        SessionTransitionReason::BreakAutoStarted
    );
    assert_eq!(automatic_timer.status(), TimerStatus::Working);
    assert!(!automatic_timer.get_state().post_break_decision_pending);
}

#[test]
fn post_break_decision_continue_and_stop_resolve_only_pending_idle_state() {
    let pending_state = TimerState {
        status: TimerStatus::Idle,
        session_type: SessionType::Work,
        post_break_decision_pending: true,
        current_cycle: 1,
        ..TimerState::default()
    };
    let mut continue_timer = timer_transition_manager_from_state(pending_state.clone());

    let continue_payload = continue_timer
        .resolve_post_break_decision(PostBreakDecisionAction::Continue)
        .expect("pending continue should be accepted");

    assert_eq!(continue_payload.status, TimerStatus::Working);
    assert_eq!(continue_timer.get_state().current_cycle, 2);
    assert!(!continue_timer.get_state().post_break_decision_pending);
    assert!(matches!(
        continue_timer.resolve_post_break_decision(PostBreakDecisionAction::Continue),
        Err(PomoliceError::InvalidStateTransition { .. })
    ));

    let mut stop_timer = timer_transition_manager_from_state(pending_state);

    let stop_payload = stop_timer
        .resolve_post_break_decision(PostBreakDecisionAction::StopCycle)
        .expect("pending stop should be accepted");

    assert_eq!(stop_payload.status, TimerStatus::Idle);
    assert!(!stop_timer.get_state().post_break_decision_pending);
}

#[test]
fn emergency_stop_resets_running_or_breaking_state_to_idle() {
    let mut timer = timer_transition_manager_from_state(TimerState {
        is_fullscreen_break: true,
        break_interaction_count: 3,
        ..short_break_state(PostBreakTransitionPolicy::RequireManualContinue)
    });

    let (was_fullscreen_break, payload) = timer.emergency_stop();

    assert!(was_fullscreen_break);
    assert_eq!(payload.status, TimerStatus::Idle);
    assert_eq!(timer.status(), TimerStatus::Idle);
    assert_eq!(timer.get_state().session_type, SessionType::Work);
    assert!(!timer.get_state().is_fullscreen_break);
    assert_eq!(timer.get_state().break_interaction_count, 0);
}

#[test]
fn duplicate_emergency_stop_while_idle_preserves_idle_result() {
    let mut timer = timer_transition_manager_from_state(TimerState::default());

    let (first_was_fullscreen_break, first_payload) = timer.emergency_stop();
    let (second_was_fullscreen_break, second_payload) = timer.emergency_stop();

    assert!(!first_was_fullscreen_break);
    assert!(!second_was_fullscreen_break);
    for payload in [first_payload, second_payload] {
        assert_eq!(payload.status, TimerStatus::Idle);
        assert_eq!(payload.session_type, SessionType::Work);
        assert_eq!(
            payload.remaining_seconds,
            TimerSettings::default().work_duration_secs.get()
        );
        assert!(!payload.is_fullscreen_break);
    }
    assert_eq!(timer.status(), TimerStatus::Idle);
    assert_eq!(timer.get_state().session_type, SessionType::Work);
    assert!(!timer.get_state().is_fullscreen_break);
    assert!(!timer.get_state().post_break_decision_pending);
}

#[test]
fn invalid_start_attempts_preserve_current_timer_state() {
    for blocked_state in [
        working_state(),
        short_break_state(PostBreakTransitionPolicy::RequireManualContinue),
    ] {
        let mut timer = timer_transition_manager_from_state(blocked_state.clone());

        let result = timer.start_work(
            TimerSettings::default(),
            accepted_work_start_origin(AcceptedStartOrigin::Manual),
        );

        assert!(matches!(
            result,
            Err(PomoliceError::InvalidStateTransition { .. })
        ));
        assert_eq!(timer.get_state().status, blocked_state.status);
        assert_eq!(timer.get_state().session_type, blocked_state.session_type);
    }
}

#[tokio::test]
async fn manual_persistence_failure_returns_after_published_start_effects() {
    let state = state_with_runtime_persistence_failure().await;
    let mut effects = RecordedStartWorkEffects::default();

    let result = crate::work_start::start_manual_work(
        &state,
        TimerSettings::default(),
        accepted_work_start_origin(AcceptedStartOrigin::Manual),
        &mut effects,
    )
    .await;

    assert!(matches!(result, Err(PomoliceError::PersistenceSave(_))));
    assert_accepted_start_effects(&effects, default_work_duration_secs());
    assert_eq!(
        state.timer_manager.lock().await.status(),
        TimerStatus::Working
    );
}

#[tokio::test]
async fn scheduler_persistence_failure_is_nonfatal_after_published_start_effects() {
    let state = state_with_runtime_persistence_failure().await;

    let (attempt, effects) = execute_scheduler_auto_start(&state, TimerStatus::Idle)
        .await
        .expect("scheduler persistence failure should be logged and accepted");

    assert_eq!(attempt, StartWorkAttempt::Accepted);
    assert_accepted_start_effects(&effects, default_work_duration_secs());
    assert_eq!(
        state.timer_manager.lock().await.status(),
        TimerStatus::Working
    );
}

#[test]
fn break_lock_restore_effects_preserve_captured_window_state_and_default_fallback() {
    let effects = record_break_lock_effects();

    assert!(effects.offscreen_geometry_was_ignored);
    assert!(matches!(
        effects.saved_restore_plan,
        MainWindowRestorePlan::Saved(_)
    ));
    assert_eq!(
        effects.default_restore_plan,
        MainWindowRestorePlan::DefaultCentered {
            width: DEFAULT_CENTERED_RESTORE_WIDTH,
            height: DEFAULT_CENTERED_RESTORE_HEIGHT,
        }
    );
}
