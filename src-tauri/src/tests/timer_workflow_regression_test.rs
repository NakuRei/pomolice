use std::future::Future;
use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::sync::atomic::{AtomicBool as StdAtomicBool, Ordering};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use chrono::{Local, LocalResult, TimeZone, Utc};

use crate::commands::timer::get_full_state_for_tests;
use crate::error::{PomoliceError, Result};
use crate::external_adapters::clock::testing::FixedClock;
use crate::external_adapters::clock::Clock;
use crate::external_adapters::persistence::PersistenceManager;
use crate::scheduler::SchedulerTickTestRuntime;
use crate::session_pipeline::{
    emergency_stop_with_runtime_effects, sync_automation_policies_with_runtime_effects,
    TimerCommandRuntimeEffectShell,
};
use crate::state::AppState;
use crate::types::{
    DayAutoRunSlot, FullStatePayload, HhMm, PostBreakTransitionPolicy, SessionTransitionReason,
    SessionType, TimerSettings, TimerState, TimerStatus, TimerTickPayload, WeeklyAutoRunConfig,
    WorkStartOrigin,
};
use crate::work_start::{
    resolve_boot_auto_start_prompt_with_effects as resolve_boot_auto_start_prompt_contract,
    start_manual_work_from_command_with_effects, StartWorkRuntimeEffects,
};

#[derive(Clone, Debug, PartialEq)]
struct TimerTickSnapshot {
    status: TimerStatus,
    remaining_seconds: u64,
    session_type: SessionType,
    current_cycle: u32,
    completed_pomodoros: u32,
    break_interaction_count: u32,
    is_fullscreen_break: bool,
}

impl From<&TimerTickPayload> for TimerTickSnapshot {
    fn from(payload: &TimerTickPayload) -> Self {
        Self {
            status: payload.status,
            remaining_seconds: payload.remaining_seconds,
            session_type: payload.session_type,
            current_cycle: payload.current_cycle,
            completed_pomodoros: payload.completed_pomodoros,
            break_interaction_count: payload.break_interaction_count,
            is_fullscreen_break: payload.is_fullscreen_break,
        }
    }
}

#[derive(Default)]
struct RecordingRuntimeEffectShell {
    emitted_tick_snapshots: Vec<TimerTickSnapshot>,
    tray_tick_snapshots: Vec<TimerTickSnapshot>,
    work_started_durations: Vec<u64>,
}

impl StartWorkRuntimeEffects for RecordingRuntimeEffectShell {
    fn emit_timer_tick(&mut self, tick_payload: &TimerTickPayload) {
        self.emitted_tick_snapshots
            .push(TimerTickSnapshot::from(tick_payload));
    }

    fn update_tray(&mut self, tick_payload: &TimerTickPayload) {
        self.tray_tick_snapshots
            .push(TimerTickSnapshot::from(tick_payload));
    }

    fn emit_work_started(&mut self, work_duration: u64) {
        self.work_started_durations.push(work_duration);
    }

    fn spawn_timer_task<'a>(
        &'a mut self,
        _state: &'a AppState,
    ) -> Pin<Box<dyn Future<Output = ()> + Send + 'a>> {
        Box::pin(async {})
    }
}

#[derive(Default)]
struct RecordingCommandRuntimeEffectShell {
    emitted_tick_snapshots: Vec<TimerTickSnapshot>,
    tray_tick_snapshots: Vec<TimerTickSnapshot>,
    fullscreen_break_exit_requests: usize,
}

impl TimerCommandRuntimeEffectShell for RecordingCommandRuntimeEffectShell {
    fn emit_tick(&mut self, tick_payload: &TimerTickPayload) {
        self.emitted_tick_snapshots
            .push(TimerTickSnapshot::from(tick_payload));
    }

    fn update_tray(&mut self, tick_payload: &TimerTickPayload) {
        self.tray_tick_snapshots
            .push(TimerTickSnapshot::from(tick_payload));
    }

    fn spawn_timer_task<'a>(
        &'a mut self,
        _state: &'a AppState,
    ) -> Pin<Box<dyn Future<Output = ()> + Send + 'a>> {
        Box::pin(async {})
    }

    fn request_fullscreen_break_exit(&mut self) {
        self.fullscreen_break_exit_requests += 1;
    }
}

fn create_temp_dir(test_name: &str) -> PathBuf {
    let timestamp_nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time should be after unix epoch")
        .as_nanos();

    let path = std::env::temp_dir().join(format!(
        "pomolice-timer-command-{}-{}-{}",
        test_name,
        std::process::id(),
        timestamp_nanos
    ));
    std::fs::create_dir_all(&path).expect("temp directory should be created");
    path
}

fn cleanup_temp_dir(path: &Path) {
    let _ = std::fs::remove_dir_all(path);
}

fn create_utc_datetime(
    year: i32,
    month: u32,
    day: u32,
    hour: u32,
    minute: u32,
    second: u32,
) -> chrono::DateTime<Utc> {
    Utc.with_ymd_and_hms(year, month, day, hour, minute, second)
        .single()
        .expect("valid UTC datetime")
}

fn fixed_clock(now_utc: chrono::DateTime<Utc>) -> Arc<FixedClock> {
    Arc::new(FixedClock::new(now_utc.with_timezone(&Local)))
}

fn local_datetime(
    year: i32,
    month: u32,
    day: u32,
    hour: u32,
    minute: u32,
    second: u32,
) -> chrono::DateTime<Local> {
    match Local.with_ymd_and_hms(year, month, day, hour, minute, second) {
        LocalResult::Single(datetime) => datetime,
        LocalResult::Ambiguous(first, _) => first,
        LocalResult::None => panic!("test local datetime should exist"),
    }
}

fn as_clock_trait(clock: &Arc<FixedClock>) -> Arc<dyn Clock + Send + Sync> {
    Arc::<FixedClock>::clone(clock) as Arc<dyn Clock + Send + Sync>
}

async fn build_test_state_without_persistence_path() -> AppState {
    let clock = fixed_clock(create_utc_datetime(2025, 2, 1, 9, 0, 0));
    let persistence = PersistenceManager::new(as_clock_trait(&clock), None);
    let (app_state, _) =
        AppState::build_startup_state_for_tests(as_clock_trait(&clock), persistence).await;
    app_state
}

async fn build_test_state_at(now_local: chrono::DateTime<Local>) -> AppState {
    let clock = Arc::new(FixedClock::new(now_local));
    let persistence = PersistenceManager::new(as_clock_trait(&clock), None);
    let (app_state, _) =
        AppState::build_startup_state_for_tests(as_clock_trait(&clock), persistence).await;
    app_state
}

fn assert_single_published_timer_state<'a>(
    emitted_tick_snapshots: &'a [TimerTickSnapshot],
    tray_tick_snapshots: &[TimerTickSnapshot],
    expected_status: TimerStatus,
    expected_session_type: SessionType,
) -> &'a TimerTickSnapshot {
    assert_eq!(emitted_tick_snapshots.len(), 1);
    assert_eq!(tray_tick_snapshots.len(), 1);
    assert_eq!(emitted_tick_snapshots, tray_tick_snapshots);
    let published = &emitted_tick_snapshots[0];
    assert_eq!(published.status, expected_status);
    assert_eq!(published.session_type, expected_session_type);
    published
}

fn assert_single_published_start_timer_state(
    runtime_effect_shell: &RecordingRuntimeEffectShell,
    expected_status: TimerStatus,
    expected_session_type: SessionType,
) -> &TimerTickSnapshot {
    assert_single_published_timer_state(
        &runtime_effect_shell.emitted_tick_snapshots,
        &runtime_effect_shell.tray_tick_snapshots,
        expected_status,
        expected_session_type,
    )
}

fn assert_no_start_runtime_effects(runtime_effect_shell: &RecordingRuntimeEffectShell) {
    assert!(runtime_effect_shell.emitted_tick_snapshots.is_empty());
    assert!(runtime_effect_shell.tray_tick_snapshots.is_empty());
    assert!(runtime_effect_shell.work_started_durations.is_empty());
}

fn assert_start_work_started_duration(
    runtime_effect_shell: &RecordingRuntimeEffectShell,
    expected_work_duration: u64,
) {
    assert_eq!(
        runtime_effect_shell.work_started_durations,
        [expected_work_duration]
    );
}

fn assert_persistence_save_error(result: &Result<()>) {
    assert!(matches!(result, Err(PomoliceError::PersistenceSave(_))));
}

fn enabled_slot(start_time: &str, end_time: &str) -> DayAutoRunSlot {
    DayAutoRunSlot::try_new(
        true,
        HhMm::try_from(start_time.to_string()).expect("start time should be valid"),
        HhMm::try_from(end_time.to_string()).expect("end time should be valid"),
    )
    .expect("slot should be valid")
}

fn monday_config(start_time: &str, end_time: &str) -> WeeklyAutoRunConfig {
    WeeklyAutoRunConfig::try_new(
        enabled_slot(start_time, end_time),
        DayAutoRunSlot::default(),
        DayAutoRunSlot::default(),
        DayAutoRunSlot::default(),
        DayAutoRunSlot::default(),
        DayAutoRunSlot::default(),
        DayAutoRunSlot::default(),
    )
    .expect("weekly config should be valid")
}

fn monday_and_tuesday_config(start_time: &str, end_time: &str) -> WeeklyAutoRunConfig {
    WeeklyAutoRunConfig::try_new(
        enabled_slot(start_time, end_time),
        enabled_slot(start_time, end_time),
        DayAutoRunSlot::default(),
        DayAutoRunSlot::default(),
        DayAutoRunSlot::default(),
        DayAutoRunSlot::default(),
        DayAutoRunSlot::default(),
    )
    .expect("weekly config should be valid")
}

fn make_enabled_daytime_monday_config() -> WeeklyAutoRunConfig {
    monday_config("09:00", "18:00")
}

fn monday_config_with_disabled_tuesday(start_time: &str, end_time: &str) -> WeeklyAutoRunConfig {
    WeeklyAutoRunConfig::try_new(
        enabled_slot("09:00", "18:00"),
        DayAutoRunSlot::try_new(
            false,
            HhMm::try_from(start_time.to_string()).expect("start time should be valid"),
            HhMm::try_from(end_time.to_string()).expect("end time should be valid"),
        )
        .expect("disabled Tuesday slot should be valid"),
        DayAutoRunSlot::default(),
        DayAutoRunSlot::default(),
        DayAutoRunSlot::default(),
        DayAutoRunSlot::default(),
        DayAutoRunSlot::default(),
    )
    .expect("weekly configuration should be valid")
}

#[derive(Debug, Clone, Copy)]
enum TargetScheduleBoundary {
    Start,
    End,
}

#[derive(Debug, Clone, Copy)]
enum BoundaryRelation {
    AlreadyPassed,
    StillAhead,
}

async fn sync_auto_run_policies_for_test(
    state: &AppState,
    weekly_auto_run_config: WeeklyAutoRunConfig,
    post_break_transition_policy: PostBreakTransitionPolicy,
) {
    sync_auto_run_policies_with_feature_for_test(
        state,
        true,
        weekly_auto_run_config,
        post_break_transition_policy,
    )
    .await;
}

async fn sync_auto_run_policies_with_feature_for_test(
    state: &AppState,
    auto_run_feature_enabled: bool,
    weekly_auto_run_config: WeeklyAutoRunConfig,
    post_break_transition_policy: PostBreakTransitionPolicy,
) {
    let mut runtime_effect_shell = RecordingCommandRuntimeEffectShell::default();

    sync_automation_policies_with_runtime_effects(
        state,
        auto_run_feature_enabled,
        weekly_auto_run_config,
        post_break_transition_policy,
        &mut runtime_effect_shell,
    )
    .await
    .expect("automation policy sync should succeed");
}

async fn start_auto_run_eligible_work_session(state: &AppState) {
    let mut timer = state.timer_manager.lock().await;
    timer
        .start_work(
            TimerSettings::default(),
            WorkStartOrigin::ManualInsideScheduleWindow,
        )
        .expect("eligible work session should start");
}

async fn complete_work_session_reason(state: &AppState) -> SessionTransitionReason {
    let mut timer = state.timer_manager.lock().await;
    timer.complete_work_session_transition().transition_reason
}

async fn complete_initial_full_state_sync_outside_schedule_window(state: &AppState) {
    let full_state = get_full_state_for_tests(state)
        .await
        .expect("full state should be available");

    assert!(full_state.boot_auto_start_prompt_window.is_none());
}

async fn assert_work_session_started_from_boundary(state: &AppState) {
    assert_work_session_started_from_boundary_for_scenario(
        state,
        "scheduled work session should start at the boundary",
    )
    .await;
}

async fn assert_work_session_started_from_boundary_for_scenario(state: &AppState, scenario: &str) {
    let timer = state.timer_manager.lock().await;
    let full_state = timer.get_full_state();

    assert_eq!(full_state.status, TimerStatus::Working, "{scenario}");
    assert_eq!(
        full_state.remaining_seconds,
        full_state.settings.work_duration_secs.get(),
        "{scenario}"
    );
}

async fn assert_no_work_session_started_from_boundary(state: &AppState) {
    assert_no_work_session_started_from_boundary_for_scenario(
        state,
        "scheduled work session should not start at the boundary",
    )
    .await;
}

async fn assert_no_work_session_started_from_boundary_for_scenario(
    state: &AppState,
    scenario: &str,
) {
    let timer = state.timer_manager.lock().await;
    let full_state = timer.get_full_state();

    assert_eq!(full_state.status, TimerStatus::Idle, "{scenario}");
}

async fn assert_scope_change_boundary_result(
    scenario: &str,
    changed_config: WeeklyAutoRunConfig,
    restore_original_config_before_evaluation: bool,
    target_boundary: TargetScheduleBoundary,
    boundary_relation: BoundaryRelation,
) {
    let baseline_config = make_enabled_daytime_monday_config();
    let (state, previous_local, evaluation_local, later_boundary_local) = match target_boundary {
        TargetScheduleBoundary::Start => {
            let state = build_test_state_at(local_datetime(2025, 1, 6, 8, 0, 0)).await;
            sync_auto_run_policies_for_test(
                &state,
                baseline_config.clone(),
                PostBreakTransitionPolicy::RequireManualContinue,
            )
            .await;
            complete_initial_full_state_sync_outside_schedule_window(&state).await;

            let (evaluation_local, later_boundary_local) = match boundary_relation {
                BoundaryRelation::AlreadyPassed => (
                    local_datetime(2025, 1, 6, 10, 0, 0),
                    local_datetime(2025, 1, 6, 10, 0, 30),
                ),
                BoundaryRelation::StillAhead => (
                    local_datetime(2025, 1, 6, 8, 59, 55),
                    local_datetime(2025, 1, 6, 9, 0, 10),
                ),
            };

            (
                state,
                local_datetime(2025, 1, 6, 8, 59, 50),
                evaluation_local,
                later_boundary_local,
            )
        }
        TargetScheduleBoundary::End => {
            let state = build_test_state_without_persistence_path().await;
            sync_auto_run_policies_for_test(
                &state,
                baseline_config.clone(),
                PostBreakTransitionPolicy::RequireManualContinue,
            )
            .await;
            start_auto_run_eligible_work_session(&state).await;

            let (evaluation_local, later_boundary_local) = match boundary_relation {
                BoundaryRelation::AlreadyPassed => (
                    local_datetime(2025, 1, 6, 18, 30, 0),
                    local_datetime(2025, 1, 6, 18, 30, 30),
                ),
                BoundaryRelation::StillAhead => (
                    local_datetime(2025, 1, 6, 17, 59, 55),
                    local_datetime(2025, 1, 6, 18, 0, 10),
                ),
            };

            (
                state,
                local_datetime(2025, 1, 6, 17, 59, 50),
                evaluation_local,
                later_boundary_local,
            )
        }
    };

    let mut scheduler_runtime = SchedulerTickTestRuntime::from_state(&state, previous_local).await;

    sync_auto_run_policies_for_test(
        &state,
        changed_config,
        PostBreakTransitionPolicy::RequireManualContinue,
    )
    .await;

    if restore_original_config_before_evaluation {
        sync_auto_run_policies_for_test(
            &state,
            baseline_config,
            PostBreakTransitionPolicy::RequireManualContinue,
        )
        .await;
    }

    scheduler_runtime.run_tick(&state, evaluation_local).await;
    scheduler_runtime
        .run_tick(&state, later_boundary_local)
        .await;

    match (target_boundary, boundary_relation) {
        (TargetScheduleBoundary::Start, BoundaryRelation::AlreadyPassed) => {
            assert_no_work_session_started_from_boundary_for_scenario(&state, scenario).await;
        }
        (TargetScheduleBoundary::Start, BoundaryRelation::StillAhead) => {
            assert_work_session_started_from_boundary_for_scenario(&state, scenario).await;
        }
        (TargetScheduleBoundary::End, BoundaryRelation::AlreadyPassed) => {
            assert_eq!(
                complete_work_session_reason(&state).await,
                SessionTransitionReason::WorkToBreak,
                "{scenario}"
            );
        }
        (TargetScheduleBoundary::End, BoundaryRelation::StillAhead) => {
            assert_eq!(
                complete_work_session_reason(&state).await,
                SessionTransitionReason::ScheduledStopExecuted,
                "{scenario}"
            );
        }
    }
}

fn assert_daytime_monday_window(full_state: &FullStatePayload) {
    let active_window = full_state
        .active_window_summary
        .as_ref()
        .expect("Monday daytime window should be active");

    assert_eq!(active_window.weekday_key, "mon");
    assert_eq!(active_window.start_time, "09:00");
    assert_eq!(active_window.end_time, "18:00");
    assert!(!active_window.crosses_midnight);
}

async fn build_prompting_state(
    now_local: chrono::DateTime<Local>,
    weekly_auto_run_config: WeeklyAutoRunConfig,
) -> AppState {
    let state = build_test_state_at(now_local).await;
    sync_auto_run_policies_for_test(
        &state,
        weekly_auto_run_config,
        PostBreakTransitionPolicy::RequireManualContinue,
    )
    .await;

    let full_state = get_full_state_for_tests(&state)
        .await
        .expect("full state should be available");
    assert!(full_state.boot_auto_start_prompt_window.is_some());
    assert!(full_state.active_window_summary.is_some());

    state
}

async fn build_prompting_state_with_controlled_save_failure(
    now_local: chrono::DateTime<Local>,
    weekly_auto_run_config: WeeklyAutoRunConfig,
    app_data_dir: PathBuf,
    fail_saves: Arc<StdAtomicBool>,
) -> AppState {
    let clock = Arc::new(FixedClock::new(now_local));
    let fail_saves_for_hook = Arc::clone(&fail_saves);
    let persistence = PersistenceManager::new(as_clock_trait(&clock), Some(app_data_dir))
        .with_injected_save_error_hook(Arc::new(move || {
            if fail_saves_for_hook.load(Ordering::Acquire) {
                Some(
                    crate::external_adapters::persistence::SaveError::Serialize {
                        source: serde_json::from_str::<serde_json::Value>("{")
                            .expect_err("invalid JSON should fail to parse"),
                    },
                )
            } else {
                None
            }
        }));
    let (state, _) =
        AppState::build_startup_state_for_tests(as_clock_trait(&clock), persistence).await;

    sync_auto_run_policies_for_test(
        &state,
        weekly_auto_run_config,
        PostBreakTransitionPolicy::RequireManualContinue,
    )
    .await;

    let full_state = get_full_state_for_tests(&state)
        .await
        .expect("full state should be available");
    assert!(full_state.boot_auto_start_prompt_window.is_some());
    assert!(full_state.active_window_summary.is_some());

    state
}

async fn start_manual_command_and_run_end_boundary(state: &AppState) -> SessionTransitionReason {
    let mut start_effects = RecordingRuntimeEffectShell::default();
    start_manual_work_from_command_with_effects(
        state,
        TimerSettings::default(),
        &mut start_effects,
    )
    .await
    .expect("manual command should start work");
    let mut scheduler_runtime =
        SchedulerTickTestRuntime::from_state(state, local_datetime(2025, 1, 6, 17, 59, 50)).await;

    scheduler_runtime
        .run_tick(state, local_datetime(2025, 1, 6, 18, 0, 10))
        .await;

    complete_work_session_reason(state).await
}

#[tokio::test]
async fn manual_command_inside_window_requests_scheduled_stop_at_end_boundary() {
    let state = build_test_state_at(local_datetime(2025, 1, 6, 10, 0, 0)).await;
    sync_auto_run_policies_for_test(
        &state,
        make_enabled_daytime_monday_config(),
        PostBreakTransitionPolicy::RequireManualContinue,
    )
    .await;

    let transition_reason = start_manual_command_and_run_end_boundary(&state).await;

    assert_eq!(
        transition_reason,
        SessionTransitionReason::ScheduledStopExecuted
    );
}

#[tokio::test]
async fn manual_command_outside_window_does_not_request_scheduled_stop() {
    let state = build_test_state_at(local_datetime(2025, 1, 6, 8, 0, 0)).await;
    sync_auto_run_policies_for_test(
        &state,
        make_enabled_daytime_monday_config(),
        PostBreakTransitionPolicy::RequireManualContinue,
    )
    .await;

    let transition_reason = start_manual_command_and_run_end_boundary(&state).await;

    assert_eq!(transition_reason, SessionTransitionReason::WorkToBreak);
}

#[tokio::test]
async fn manual_command_with_disabled_auto_run_does_not_request_scheduled_stop() {
    let state = build_test_state_at(local_datetime(2025, 1, 6, 10, 0, 0)).await;
    sync_auto_run_policies_with_feature_for_test(
        &state,
        false,
        make_enabled_daytime_monday_config(),
        PostBreakTransitionPolicy::RequireManualContinue,
    )
    .await;

    let transition_reason = start_manual_command_and_run_end_boundary(&state).await;

    assert_eq!(transition_reason, SessionTransitionReason::WorkToBreak);
}

#[tokio::test]
async fn boot_prompt_resolution_before_initial_full_state_does_not_open_start_boundary() {
    for start_now in [true, false] {
        let state = build_test_state_at(local_datetime(2025, 1, 6, 9, 0, 5)).await;
        sync_auto_run_policies_for_test(
            &state,
            make_enabled_daytime_monday_config(),
            PostBreakTransitionPolicy::RequireManualContinue,
        )
        .await;
        let mut runtime_effect_shell = RecordingRuntimeEffectShell::default();
        let mut scheduler_runtime =
            SchedulerTickTestRuntime::from_state(&state, local_datetime(2025, 1, 6, 8, 59, 50))
                .await;

        resolve_boot_auto_start_prompt_contract(&state, start_now, &mut runtime_effect_shell)
            .await
            .expect("unexpected boot prompt resolution should be ignored");
        scheduler_runtime
            .run_tick(&state, local_datetime(2025, 1, 6, 9, 0, 10))
            .await;

        assert_no_start_runtime_effects(&runtime_effect_shell);
        assert_no_work_session_started_from_boundary(&state).await;
        let full_state = get_full_state_for_tests(&state)
            .await
            .expect("initial full state should still decide startup prompt");
        assert!(full_state.boot_auto_start_prompt_window.is_some());
        assert_daytime_monday_window(&full_state);
    }
}

#[tokio::test]
async fn boot_prompt_resolution_without_active_prompt_does_not_start_work() {
    for start_now in [true, false] {
        let state = build_test_state_at(local_datetime(2025, 1, 6, 10, 0, 0)).await;
        let mut runtime_effect_shell = RecordingRuntimeEffectShell::default();

        resolve_boot_auto_start_prompt_contract(&state, start_now, &mut runtime_effect_shell)
            .await
            .expect("missing boot prompt should resolve safely");

        let full_state = state.timer_manager.lock().await.get_full_state();
        assert_eq!(full_state.status, TimerStatus::Idle);
        assert_eq!(full_state.session_type, SessionType::Work);
        assert_no_start_runtime_effects(&runtime_effect_shell);
    }
}

#[tokio::test]
async fn boot_prompt_start_persistence_error_allows_later_scheduled_start_boundary() {
    let temp_dir = create_temp_dir("boot-prompt-start-save-fails-allows-later-schedule");
    let fail_saves = Arc::new(StdAtomicBool::new(false));
    let state = build_prompting_state_with_controlled_save_failure(
        local_datetime(2025, 1, 6, 10, 0, 0),
        monday_and_tuesday_config("09:00", "18:00"),
        temp_dir.clone(),
        Arc::clone(&fail_saves),
    )
    .await;
    fail_saves.store(true, Ordering::Release);
    let mut start_runtime_effect_shell = RecordingRuntimeEffectShell::default();

    let start_result =
        resolve_boot_auto_start_prompt_contract(&state, true, &mut start_runtime_effect_shell)
            .await;
    assert_persistence_save_error(&start_result);

    let mut stop_runtime_effect_shell = RecordingCommandRuntimeEffectShell::default();
    let stop_result =
        emergency_stop_with_runtime_effects(&state, &mut stop_runtime_effect_shell).await;
    assert_persistence_save_error(&stop_result);
    assert_no_work_session_started_from_boundary(&state).await;

    let mut scheduler_runtime =
        SchedulerTickTestRuntime::from_state(&state, local_datetime(2025, 1, 7, 8, 59, 50)).await;
    scheduler_runtime
        .run_tick(&state, local_datetime(2025, 1, 7, 9, 0, 10))
        .await;

    assert_work_session_started_from_boundary(&state).await;

    cleanup_temp_dir(&temp_dir);
}

#[tokio::test]
async fn boot_prompt_start_returns_save_error_after_publishing_started_work() {
    let temp_dir = create_temp_dir("boot-prompt-start-save-fails-after-publish");
    let fail_saves = Arc::new(StdAtomicBool::new(false));
    let state = build_prompting_state_with_controlled_save_failure(
        local_datetime(2025, 1, 6, 10, 0, 0),
        make_enabled_daytime_monday_config(),
        temp_dir.clone(),
        Arc::clone(&fail_saves),
    )
    .await;
    fail_saves.store(true, Ordering::Release);
    let mut runtime_effect_shell = RecordingRuntimeEffectShell::default();

    let result =
        resolve_boot_auto_start_prompt_contract(&state, true, &mut runtime_effect_shell).await;

    assert_persistence_save_error(&result);
    let published = assert_single_published_start_timer_state(
        &runtime_effect_shell,
        TimerStatus::Working,
        SessionType::Work,
    );
    assert_eq!(published.remaining_seconds, 1500);
    assert_start_work_started_duration(&runtime_effect_shell, 1500);

    cleanup_temp_dir(&temp_dir);
}

#[tokio::test]
async fn feature_enable_does_not_replay_past_start_boundary() {
    let state = build_test_state_at(local_datetime(2025, 1, 6, 8, 0, 0)).await;
    sync_auto_run_policies_with_feature_for_test(
        &state,
        false,
        monday_and_tuesday_config("09:00", "18:00"),
        PostBreakTransitionPolicy::RequireManualContinue,
    )
    .await;
    complete_initial_full_state_sync_outside_schedule_window(&state).await;

    let mut scheduler_runtime =
        SchedulerTickTestRuntime::from_state(&state, local_datetime(2025, 1, 6, 8, 59, 50)).await;

    sync_auto_run_policies_for_test(
        &state,
        monday_and_tuesday_config("09:00", "18:00"),
        PostBreakTransitionPolicy::RequireManualContinue,
    )
    .await;
    scheduler_runtime
        .run_tick(&state, local_datetime(2025, 1, 6, 10, 0, 10))
        .await;
    scheduler_runtime
        .run_tick(&state, local_datetime(2025, 1, 6, 10, 0, 30))
        .await;

    assert_no_work_session_started_from_boundary(&state).await;
}

#[tokio::test]
async fn feature_enable_keeps_future_start_boundary_active() {
    let state = build_test_state_at(local_datetime(2025, 1, 6, 8, 0, 0)).await;
    sync_auto_run_policies_with_feature_for_test(
        &state,
        false,
        monday_and_tuesday_config("09:00", "18:00"),
        PostBreakTransitionPolicy::RequireManualContinue,
    )
    .await;
    complete_initial_full_state_sync_outside_schedule_window(&state).await;

    let mut scheduler_runtime =
        SchedulerTickTestRuntime::from_state(&state, local_datetime(2025, 1, 6, 8, 59, 50)).await;

    sync_auto_run_policies_for_test(
        &state,
        monday_and_tuesday_config("09:00", "18:00"),
        PostBreakTransitionPolicy::RequireManualContinue,
    )
    .await;
    scheduler_runtime
        .run_tick(&state, local_datetime(2025, 1, 6, 10, 0, 10))
        .await;
    scheduler_runtime
        .run_tick(&state, local_datetime(2025, 1, 7, 9, 0, 10))
        .await;

    assert_work_session_started_from_boundary(&state).await;
}

#[tokio::test]
async fn initial_sync_outside_schedule_window_allows_future_scheduled_work_start() {
    let state = build_test_state_at(local_datetime(2025, 1, 6, 8, 0, 0)).await;
    sync_auto_run_policies_for_test(
        &state,
        make_enabled_daytime_monday_config(),
        PostBreakTransitionPolicy::RequireManualContinue,
    )
    .await;
    let full_state = get_full_state_for_tests(&state)
        .await
        .expect("full state should be available");
    let mut scheduler_runtime =
        SchedulerTickTestRuntime::from_state(&state, local_datetime(2025, 1, 6, 8, 59, 50)).await;

    scheduler_runtime
        .run_tick(&state, local_datetime(2025, 1, 6, 9, 0, 10))
        .await;

    assert!(full_state.boot_auto_start_prompt_window.is_none());
    assert!(full_state.active_window_summary.is_none());
    assert_work_session_started_from_boundary(&state).await;
}

#[tokio::test]
async fn restored_auto_run_scope_allows_first_future_scheduled_work_start() {
    let temp_dir = create_temp_dir("restored-auto-run-scope");
    let clock = Arc::new(FixedClock::new(local_datetime(2025, 1, 6, 8, 0, 0)));
    let persistence = PersistenceManager::new(as_clock_trait(&clock), Some(temp_dir.clone()));
    persistence
        .save_state_sync(TimerState {
            auto_run_feature_enabled: true,
            weekly_auto_run_config: make_enabled_daytime_monday_config(),
            ..TimerState::default()
        })
        .expect("restored auto-run scope should save");
    let (state, _) =
        AppState::build_startup_state_for_tests(as_clock_trait(&clock), persistence).await;
    complete_initial_full_state_sync_outside_schedule_window(&state).await;
    let mut scheduler_runtime =
        SchedulerTickTestRuntime::from_state(&state, local_datetime(2025, 1, 6, 8, 59, 50)).await;

    scheduler_runtime
        .run_tick(&state, local_datetime(2025, 1, 6, 9, 0, 10))
        .await;

    assert_work_session_started_from_boundary(&state).await;
    cleanup_temp_dir(&temp_dir);
}

#[tokio::test]
async fn initial_sync_while_working_allows_future_scheduled_work_start_after_stop() {
    let state = build_test_state_at(local_datetime(2025, 1, 6, 10, 0, 0)).await;
    sync_auto_run_policies_for_test(
        &state,
        monday_and_tuesday_config("09:00", "18:00"),
        PostBreakTransitionPolicy::RequireManualContinue,
    )
    .await;
    start_auto_run_eligible_work_session(&state).await;
    let full_state = get_full_state_for_tests(&state)
        .await
        .expect("full state should be available");
    let mut stop_runtime_effect_shell = RecordingCommandRuntimeEffectShell::default();
    emergency_stop_with_runtime_effects(&state, &mut stop_runtime_effect_shell)
        .await
        .expect("emergency stop should succeed");
    let mut scheduler_runtime =
        SchedulerTickTestRuntime::from_state(&state, local_datetime(2025, 1, 7, 8, 59, 50)).await;

    scheduler_runtime
        .run_tick(&state, local_datetime(2025, 1, 7, 9, 0, 10))
        .await;

    assert_eq!(full_state.status, TimerStatus::Working);
    assert!(full_state.boot_auto_start_prompt_window.is_none());
    assert_work_session_started_from_boundary(&state).await;
}

#[tokio::test]
async fn pending_boot_prompt_prevents_scheduled_work_start_after_follow_up_full_sync() {
    let state = build_prompting_state(
        local_datetime(2025, 1, 6, 9, 0, 5),
        make_enabled_daytime_monday_config(),
    )
    .await;
    let second_full_state = get_full_state_for_tests(&state)
        .await
        .expect("second full state should be available");
    let mut scheduler_runtime =
        SchedulerTickTestRuntime::from_state(&state, local_datetime(2025, 1, 6, 8, 59, 50)).await;

    scheduler_runtime
        .run_tick(&state, local_datetime(2025, 1, 6, 9, 0, 10))
        .await;

    assert!(second_full_state.boot_auto_start_prompt_window.is_none());
    assert_no_work_session_started_from_boundary(&state).await;
}

#[tokio::test]
async fn policy_only_change_does_not_suppress_arriving_end_boundary() {
    let state = build_test_state_without_persistence_path().await;
    sync_auto_run_policies_for_test(
        &state,
        make_enabled_daytime_monday_config(),
        PostBreakTransitionPolicy::RequireManualContinue,
    )
    .await;
    start_auto_run_eligible_work_session(&state).await;

    let mut scheduler_runtime =
        SchedulerTickTestRuntime::from_state(&state, local_datetime(2025, 1, 6, 17, 59, 50)).await;

    sync_auto_run_policies_for_test(
        &state,
        make_enabled_daytime_monday_config(),
        PostBreakTransitionPolicy::StartNextWorkAutomatically,
    )
    .await;
    scheduler_runtime
        .run_tick(&state, local_datetime(2025, 1, 6, 18, 0, 10))
        .await;

    assert_eq!(
        complete_work_session_reason(&state).await,
        SessionTransitionReason::ScheduledStopExecuted
    );
}

#[tokio::test]
async fn policy_only_change_does_not_suppress_arriving_start_boundary() {
    let state = build_test_state_at(local_datetime(2025, 1, 6, 8, 0, 0)).await;
    sync_auto_run_policies_for_test(
        &state,
        make_enabled_daytime_monday_config(),
        PostBreakTransitionPolicy::RequireManualContinue,
    )
    .await;
    complete_initial_full_state_sync_outside_schedule_window(&state).await;

    let mut scheduler_runtime =
        SchedulerTickTestRuntime::from_state(&state, local_datetime(2025, 1, 6, 8, 59, 50)).await;

    sync_auto_run_policies_for_test(
        &state,
        make_enabled_daytime_monday_config(),
        PostBreakTransitionPolicy::StartNextWorkAutomatically,
    )
    .await;
    scheduler_runtime
        .run_tick(&state, local_datetime(2025, 1, 6, 9, 0, 10))
        .await;

    assert_work_session_started_from_boundary(&state).await;
}

#[tokio::test]
async fn resolve_boot_prompt_later_keeps_timer_idle() {
    let state = build_prompting_state(
        local_datetime(2025, 1, 6, 10, 0, 0),
        make_enabled_daytime_monday_config(),
    )
    .await;
    let mut runtime_effect_shell = RecordingRuntimeEffectShell::default();

    resolve_boot_auto_start_prompt_contract(&state, false, &mut runtime_effect_shell)
        .await
        .expect("boot prompt should resolve later");

    let full_state = state.timer_manager.lock().await.get_full_state();
    assert_eq!(full_state.status, TimerStatus::Idle);
    assert_eq!(full_state.session_type, SessionType::Work);
    assert_no_start_runtime_effects(&runtime_effect_shell);
}

#[tokio::test]
async fn resolve_boot_prompt_start_begins_work_when_prompt_is_active() {
    let state = build_prompting_state(
        local_datetime(2025, 1, 6, 10, 0, 0),
        make_enabled_daytime_monday_config(),
    )
    .await;
    let mut runtime_effect_shell = RecordingRuntimeEffectShell::default();

    resolve_boot_auto_start_prompt_contract(&state, true, &mut runtime_effect_shell)
        .await
        .expect("boot prompt should start work");

    let full_state = state.timer_manager.lock().await.get_full_state();
    assert_eq!(full_state.status, TimerStatus::Working);
    assert_eq!(full_state.session_type, SessionType::Work);
    assert_eq!(
        full_state.remaining_seconds,
        full_state.settings.work_duration_secs.get()
    );
}

#[tokio::test]
async fn resolved_later_allows_next_scheduled_start_boundary() {
    let state = build_prompting_state(
        local_datetime(2025, 1, 6, 10, 0, 0),
        monday_and_tuesday_config("09:00", "18:00"),
    )
    .await;
    let mut runtime_effect_shell = RecordingRuntimeEffectShell::default();
    let mut scheduler_runtime =
        SchedulerTickTestRuntime::from_state(&state, local_datetime(2025, 1, 6, 10, 0, 0)).await;

    resolve_boot_auto_start_prompt_contract(&state, false, &mut runtime_effect_shell)
        .await
        .expect("boot prompt should resolve later");
    scheduler_runtime
        .run_tick(&state, local_datetime(2025, 1, 7, 9, 0, 10))
        .await;

    let full_state = state.timer_manager.lock().await.get_full_state();
    assert_eq!(full_state.status, TimerStatus::Working);
    assert_eq!(full_state.session_type, SessionType::Work);
    assert_eq!(
        full_state.remaining_seconds,
        full_state.settings.work_duration_secs.get()
    );
}

#[tokio::test]
async fn resolved_start_allows_next_scheduled_start_boundary() {
    let state = build_prompting_state(
        local_datetime(2025, 1, 6, 10, 0, 0),
        monday_and_tuesday_config("09:00", "18:00"),
    )
    .await;
    let mut start_runtime_effect_shell = RecordingRuntimeEffectShell::default();

    resolve_boot_auto_start_prompt_contract(&state, true, &mut start_runtime_effect_shell)
        .await
        .expect("boot prompt should start work");

    assert_work_session_started_from_boundary(&state).await;

    let mut stop_runtime_effect_shell = RecordingCommandRuntimeEffectShell::default();
    emergency_stop_with_runtime_effects(&state, &mut stop_runtime_effect_shell)
        .await
        .expect("emergency stop should succeed");
    assert_no_work_session_started_from_boundary(&state).await;

    let mut scheduler_runtime =
        SchedulerTickTestRuntime::from_state(&state, local_datetime(2025, 1, 7, 8, 59, 50)).await;
    scheduler_runtime
        .run_tick(&state, local_datetime(2025, 1, 7, 9, 0, 10))
        .await;

    assert_work_session_started_from_boundary(&state).await;
}

#[tokio::test]
async fn scheduler_does_not_auto_start_while_boot_prompt_is_waiting() {
    let state = build_prompting_state(
        local_datetime(2025, 1, 6, 9, 0, 5),
        make_enabled_daytime_monday_config(),
    )
    .await;
    let mut scheduler_runtime =
        SchedulerTickTestRuntime::from_state(&state, local_datetime(2025, 1, 6, 8, 59, 50)).await;

    scheduler_runtime
        .run_tick(&state, local_datetime(2025, 1, 6, 9, 0, 10))
        .await;

    let full_state = state.timer_manager.lock().await.get_full_state();
    assert_eq!(full_state.status, TimerStatus::Idle);
    assert_eq!(full_state.session_type, SessionType::Work);
}

#[tokio::test]
async fn scheduler_waits_for_initial_full_state_before_start_boundary() {
    let state = build_test_state_at(local_datetime(2025, 1, 6, 9, 0, 5)).await;
    sync_auto_run_policies_for_test(
        &state,
        make_enabled_daytime_monday_config(),
        PostBreakTransitionPolicy::RequireManualContinue,
    )
    .await;
    let mut scheduler_runtime =
        SchedulerTickTestRuntime::from_state(&state, local_datetime(2025, 1, 6, 8, 59, 50)).await;

    scheduler_runtime
        .run_tick(&state, local_datetime(2025, 1, 6, 9, 0, 10))
        .await;

    assert_no_work_session_started_from_boundary(&state).await;
    let full_state = get_full_state_for_tests(&state)
        .await
        .expect("initial full state should still be available");
    assert!(full_state.boot_auto_start_prompt_window.is_some());
    assert_daytime_monday_window(&full_state);
}

#[tokio::test]
async fn stale_boot_prompt_resolution_does_not_replace_running_session() {
    for start_now in [true, false] {
        let state = build_prompting_state(
            local_datetime(2025, 1, 6, 10, 0, 0),
            make_enabled_daytime_monday_config(),
        )
        .await;
        {
            let mut timer = state.timer_manager.lock().await;
            timer
                .start_work(
                    TimerSettings::default(),
                    WorkStartOrigin::ManualOutsideScheduleWindow,
                )
                .expect("work session should start through another path");
        }
        let before = state.timer_manager.lock().await.get_full_state();
        let mut runtime_effect_shell = RecordingRuntimeEffectShell::default();

        resolve_boot_auto_start_prompt_contract(&state, start_now, &mut runtime_effect_shell)
            .await
            .expect("stale boot prompt should resolve safely");

        let after = state.timer_manager.lock().await.get_full_state();
        assert_eq!(after.status, TimerStatus::Working);
        assert_eq!(after.session_type, before.session_type);
        assert_eq!(after.current_cycle, before.current_cycle);
        assert_eq!(after.remaining_seconds, before.remaining_seconds);
        assert_no_start_runtime_effects(&runtime_effect_shell);
    }
}

#[tokio::test]
async fn topology_change_does_not_replay_past_end_boundary() {
    let state = build_test_state_without_persistence_path().await;
    sync_auto_run_policies_for_test(
        &state,
        make_enabled_daytime_monday_config(),
        PostBreakTransitionPolicy::RequireManualContinue,
    )
    .await;
    start_auto_run_eligible_work_session(&state).await;

    let mut scheduler_runtime =
        SchedulerTickTestRuntime::from_state(&state, local_datetime(2025, 1, 6, 17, 59, 50)).await;

    sync_auto_run_policies_for_test(
        &state,
        monday_and_tuesday_config("09:00", "18:00"),
        PostBreakTransitionPolicy::RequireManualContinue,
    )
    .await;
    scheduler_runtime
        .run_tick(&state, local_datetime(2025, 1, 6, 18, 30, 0))
        .await;

    assert_eq!(
        complete_work_session_reason(&state).await,
        SessionTransitionReason::WorkToBreak
    );
}

#[tokio::test]
async fn topology_change_does_not_replay_past_start_boundary() {
    let state = build_test_state_at(local_datetime(2025, 1, 6, 8, 0, 0)).await;
    sync_auto_run_policies_for_test(
        &state,
        make_enabled_daytime_monday_config(),
        PostBreakTransitionPolicy::RequireManualContinue,
    )
    .await;
    complete_initial_full_state_sync_outside_schedule_window(&state).await;

    let mut scheduler_runtime =
        SchedulerTickTestRuntime::from_state(&state, local_datetime(2025, 1, 6, 8, 59, 50)).await;

    sync_auto_run_policies_for_test(
        &state,
        monday_and_tuesday_config("09:00", "18:00"),
        PostBreakTransitionPolicy::RequireManualContinue,
    )
    .await;
    scheduler_runtime
        .run_tick(&state, local_datetime(2025, 1, 6, 10, 0, 0))
        .await;
    scheduler_runtime
        .run_tick(&state, local_datetime(2025, 1, 6, 10, 0, 30))
        .await;

    assert_no_work_session_started_from_boundary(&state).await;
}

#[tokio::test]
async fn topology_change_keeps_future_end_boundary_active() {
    let state = build_test_state_without_persistence_path().await;
    sync_auto_run_policies_for_test(
        &state,
        make_enabled_daytime_monday_config(),
        PostBreakTransitionPolicy::RequireManualContinue,
    )
    .await;
    start_auto_run_eligible_work_session(&state).await;

    let mut scheduler_runtime =
        SchedulerTickTestRuntime::from_state(&state, local_datetime(2025, 1, 6, 17, 59, 50)).await;

    sync_auto_run_policies_for_test(
        &state,
        monday_and_tuesday_config("09:00", "18:00"),
        PostBreakTransitionPolicy::RequireManualContinue,
    )
    .await;
    scheduler_runtime
        .run_tick(&state, local_datetime(2025, 1, 6, 18, 30, 0))
        .await;
    scheduler_runtime
        .run_tick(&state, local_datetime(2025, 1, 7, 18, 0, 10))
        .await;

    assert_eq!(
        complete_work_session_reason(&state).await,
        SessionTransitionReason::ScheduledStopExecuted
    );
}

#[tokio::test]
async fn topology_change_keeps_future_start_boundary_active() {
    let state = build_test_state_at(local_datetime(2025, 1, 6, 8, 0, 0)).await;
    sync_auto_run_policies_for_test(
        &state,
        make_enabled_daytime_monday_config(),
        PostBreakTransitionPolicy::RequireManualContinue,
    )
    .await;
    complete_initial_full_state_sync_outside_schedule_window(&state).await;

    let mut scheduler_runtime =
        SchedulerTickTestRuntime::from_state(&state, local_datetime(2025, 1, 6, 8, 59, 50)).await;

    sync_auto_run_policies_for_test(
        &state,
        monday_and_tuesday_config("09:00", "18:00"),
        PostBreakTransitionPolicy::RequireManualContinue,
    )
    .await;
    scheduler_runtime
        .run_tick(&state, local_datetime(2025, 1, 6, 10, 0, 0))
        .await;
    scheduler_runtime
        .run_tick(&state, local_datetime(2025, 1, 7, 9, 0, 10))
        .await;

    assert_work_session_started_from_boundary(&state).await;
}

#[tokio::test]
async fn return_to_original_schedule_keeps_past_and_future_boundaries_correct() {
    for (scenario, target_boundary, boundary_relation) in [
        (
            "return-to-original start boundary already passed",
            TargetScheduleBoundary::Start,
            BoundaryRelation::AlreadyPassed,
        ),
        (
            "return-to-original start boundary still ahead",
            TargetScheduleBoundary::Start,
            BoundaryRelation::StillAhead,
        ),
        (
            "return-to-original end boundary already passed",
            TargetScheduleBoundary::End,
            BoundaryRelation::AlreadyPassed,
        ),
        (
            "return-to-original end boundary still ahead",
            TargetScheduleBoundary::End,
            BoundaryRelation::StillAhead,
        ),
    ] {
        assert_scope_change_boundary_result(
            scenario,
            monday_and_tuesday_config("09:00", "18:00"),
            true,
            target_boundary,
            boundary_relation,
        )
        .await;
    }
}

#[tokio::test]
async fn disabled_day_time_changes_keep_past_and_future_boundaries_correct() {
    for (scenario, changed_config, target_boundary, boundary_relation) in [
        (
            "disabled Tuesday start time with past start boundary",
            monday_config_with_disabled_tuesday("10:00", "18:00"),
            TargetScheduleBoundary::Start,
            BoundaryRelation::AlreadyPassed,
        ),
        (
            "disabled Tuesday start time with future start boundary",
            monday_config_with_disabled_tuesday("10:00", "18:00"),
            TargetScheduleBoundary::Start,
            BoundaryRelation::StillAhead,
        ),
        (
            "disabled Tuesday start time with past end boundary",
            monday_config_with_disabled_tuesday("10:00", "18:00"),
            TargetScheduleBoundary::End,
            BoundaryRelation::AlreadyPassed,
        ),
        (
            "disabled Tuesday start time with future end boundary",
            monday_config_with_disabled_tuesday("10:00", "18:00"),
            TargetScheduleBoundary::End,
            BoundaryRelation::StillAhead,
        ),
        (
            "disabled Tuesday end time with past start boundary",
            monday_config_with_disabled_tuesday("09:00", "17:00"),
            TargetScheduleBoundary::Start,
            BoundaryRelation::AlreadyPassed,
        ),
        (
            "disabled Tuesday end time with future start boundary",
            monday_config_with_disabled_tuesday("09:00", "17:00"),
            TargetScheduleBoundary::Start,
            BoundaryRelation::StillAhead,
        ),
        (
            "disabled Tuesday end time with past end boundary",
            monday_config_with_disabled_tuesday("09:00", "17:00"),
            TargetScheduleBoundary::End,
            BoundaryRelation::AlreadyPassed,
        ),
        (
            "disabled Tuesday end time with future end boundary",
            monday_config_with_disabled_tuesday("09:00", "17:00"),
            TargetScheduleBoundary::End,
            BoundaryRelation::StillAhead,
        ),
    ] {
        assert_scope_change_boundary_result(
            scenario,
            changed_config,
            false,
            target_boundary,
            boundary_relation,
        )
        .await;
    }
}
