// Timer commands using dependency injection

use tauri::{Manager, State, WebviewWindow};

use crate::commands::input_rejection::ObservedCommandArg;
use crate::error::{PomoliceError, Result};
use crate::logging_event::{log_info_event, log_warn_event, BackendLogEvent};
use crate::schedule_boundary::{is_now_inside_schedule_window, ActiveWindow};
use crate::state::AppState;
use crate::types::{
    ActiveWindowSummaryPayload, FullStatePayload, LongBreakDurationSecs, LongBreakIntervalCount,
    PostBreakDecisionAction, PostBreakTransitionPolicy, ShortBreakDurationSecs, TimerSettings,
    TimerStatus, WeeklyAutoRunConfig, WorkDurationSecs,
};

/// Start a work session with settings
#[tauri::command]
pub(crate) async fn start_work(
    window: WebviewWindow,
    state: State<'_, AppState>,
    work_duration_secs: ObservedCommandArg<WorkDurationSecs>,
    short_break_duration_secs: ObservedCommandArg<ShortBreakDurationSecs>,
    long_break_duration_secs: ObservedCommandArg<LongBreakDurationSecs>,
    long_break_interval: ObservedCommandArg<LongBreakIntervalCount>,
    post_break_transition_policy: ObservedCommandArg<PostBreakTransitionPolicy>,
) -> Result<()> {
    log_info_event(
        BackendLogEvent::TimerCommandInvoked,
        &[
            ("command_name", "start_work".to_string()),
            ("window_label", window.label().to_string()),
        ],
    );
    authorize_main_window(&window, "start_work")?;
    let work_duration_secs = work_duration_secs.into_inner();
    let short_break_duration_secs = short_break_duration_secs.into_inner();
    let long_break_duration_secs = long_break_duration_secs.into_inner();
    let long_break_interval = long_break_interval.into_inner();
    let post_break_transition_policy = post_break_transition_policy.into_inner();

    let settings = TimerSettings::new(
        work_duration_secs,
        short_break_duration_secs,
        long_break_duration_secs,
        long_break_interval,
        post_break_transition_policy,
    );
    let app = window.app_handle();
    crate::work_start::start_manual_work_from_command(&state, app, settings).await
}

/// Resolve manual post-break decision from continue dialog.
///
/// This command is valid only while `post_break_decision_pending = true`.
#[tauri::command]
pub(crate) async fn resolve_post_break_decision(
    window: WebviewWindow,
    state: State<'_, AppState>,
    action: ObservedCommandArg<PostBreakDecisionAction>,
) -> Result<()> {
    authorize_main_window(&window, "resolve_post_break_decision")?;
    let action = action.into_inner();

    let app = window.app_handle();
    crate::session_pipeline::resolve_post_break_decision_from_command(app, &state, action).await
}

/// Emergency stop - reset all state
#[tauri::command]
pub(crate) async fn emergency_stop(
    window: WebviewWindow,
    state: State<'_, AppState>,
) -> Result<()> {
    log_info_event(
        BackendLogEvent::TimerCommandInvoked,
        &[
            ("command_name", "emergency_stop".to_string()),
            ("window_label", window.label().to_string()),
        ],
    );
    authorize_main_window(&window, "emergency_stop")?;

    let app = window.app_handle();
    crate::session_pipeline::emergency_stop_from_command(app, &state).await
}

/// Sync timer duration settings from frontend (idle only)
#[tauri::command]
pub(crate) async fn sync_timer_duration_settings(
    window: WebviewWindow,
    state: State<'_, AppState>,
    work_duration_secs: ObservedCommandArg<WorkDurationSecs>,
    short_break_duration_secs: ObservedCommandArg<ShortBreakDurationSecs>,
    long_break_duration_secs: ObservedCommandArg<LongBreakDurationSecs>,
    long_break_interval: ObservedCommandArg<LongBreakIntervalCount>,
) -> Result<()> {
    authorize_main_window(&window, "sync_timer_duration_settings")?;
    let work_duration_secs = work_duration_secs.into_inner();
    let short_break_duration_secs = short_break_duration_secs.into_inner();
    let long_break_duration_secs = long_break_duration_secs.into_inner();
    let long_break_interval = long_break_interval.into_inner();

    let app = window.app_handle();
    crate::session_pipeline::sync_timer_duration_settings_from_command(
        app,
        &state,
        work_duration_secs,
        short_break_duration_secs,
        long_break_duration_secs,
        long_break_interval,
    )
    .await
}

/// Sync automation policies from frontend
#[tauri::command]
pub(crate) async fn sync_automation_policies(
    window: WebviewWindow,
    state: State<'_, AppState>,
    auto_run_feature_enabled: ObservedCommandArg<bool>,
    weekly_auto_run_config: ObservedCommandArg<WeeklyAutoRunConfig>,
    post_break_transition_policy: ObservedCommandArg<PostBreakTransitionPolicy>,
) -> Result<()> {
    authorize_main_window(&window, "sync_automation_policies")?;
    let auto_run_feature_enabled = auto_run_feature_enabled.into_inner();
    let weekly_auto_run_config = weekly_auto_run_config.into_inner();
    let post_break_transition_policy = post_break_transition_policy.into_inner();

    let app = window.app_handle();
    crate::session_pipeline::sync_automation_policies_from_command(
        app,
        &state,
        auto_run_feature_enabled,
        weekly_auto_run_config,
        post_break_transition_policy,
    )
    .await
}

fn authorize_main_window_label(window_label: &str) -> Result<()> {
    if window_label == "main" {
        return Ok(());
    }

    Err(PomoliceError::Unauthorized(window_label.to_string()))
}

fn authorize_main_window(window: &WebviewWindow, command_name: &'static str) -> Result<()> {
    let window_label = window.label();
    if let Err(error) = authorize_main_window_label(window_label) {
        log_warn_event(
            BackendLogEvent::TimerCommandUnauthorized,
            &[
                ("command_name", command_name.to_string()),
                ("window_label", window_label.to_string()),
            ],
        );
        return Err(error);
    }

    Ok(())
}

fn to_active_window_summary_payload(active_window: &ActiveWindow) -> ActiveWindowSummaryPayload {
    ActiveWindowSummaryPayload {
        weekday_key: active_window.slot_key.clone(),
        start_time: active_window.start_time.clone(),
        end_time: active_window.end_time.clone(),
        crosses_midnight: active_window.crosses_midnight,
    }
}

#[cfg(test)]
async fn start_work_for_tests(
    state: &AppState,
    settings: TimerSettings,
    manual_start_origin: crate::types::WorkStartOrigin,
    runtime_effect_shell: &mut impl crate::work_start::StartWorkRuntimeEffects,
) -> Result<()> {
    crate::work_start::start_manual_work(
        state,
        settings,
        manual_start_origin,
        runtime_effect_shell,
    )
    .await?;
    Ok(())
}

#[cfg(test)]
async fn start_work_after_authorizing_window_label_for_tests(
    window_label: &str,
    state: &AppState,
    settings: TimerSettings,
    manual_start_origin: crate::types::WorkStartOrigin,
    runtime_effect_shell: &mut impl crate::work_start::StartWorkRuntimeEffects,
) -> Result<()> {
    authorize_main_window_label(window_label)?;
    start_work_for_tests(state, settings, manual_start_origin, runtime_effect_shell).await
}
async fn get_full_state_contract(state: &AppState) -> Result<FullStatePayload> {
    let now_local = state.clock.now_local();

    let (mut full_state_payload, timer_status, auto_run_feature_enabled, weekly_auto_run_config) = {
        let timer_manager = state.timer_manager.lock().await;
        let timer_state = timer_manager.get_state();
        (
            timer_manager.get_full_state(),
            timer_manager.status(),
            timer_state.auto_run_feature_enabled,
            timer_state.weekly_auto_run_config.clone(),
        )
    };

    let active_window = if auto_run_feature_enabled {
        is_now_inside_schedule_window(now_local, &weekly_auto_run_config)
    } else {
        None
    };

    full_state_payload.active_window_summary =
        active_window.as_ref().map(to_active_window_summary_payload);

    let should_prompt_start_on_boot = state.record_initial_full_state_auto_start_prompt_decision(
        timer_status == TimerStatus::Idle && active_window.is_some(),
    );

    full_state_payload.should_prompt_start_on_boot = should_prompt_start_on_boot;
    Ok(full_state_payload)
}

/// Get full timer state for initial sync
#[tauri::command]
pub(crate) async fn get_full_state(
    window: WebviewWindow,
    state: State<'_, AppState>,
) -> Result<FullStatePayload> {
    authorize_main_window(&window, "get_full_state")?;

    get_full_state_contract(&state).await
}

/// Resolve startup auto-start confirmation prompt.
#[tauri::command]
pub(crate) async fn resolve_boot_auto_start_prompt(
    window: WebviewWindow,
    state: State<'_, AppState>,
    start_now: ObservedCommandArg<bool>,
) -> Result<()> {
    authorize_main_window(&window, "resolve_boot_auto_start_prompt")?;
    let start_now = start_now.into_inner();

    let app = window.app_handle();
    crate::work_start::resolve_boot_auto_start_prompt_from_command(&state, app, start_now).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::external_adapters::clock::testing::FixedClock;
    use crate::external_adapters::clock::Clock;
    use crate::external_adapters::persistence::{PersistenceLoadOutcome, PersistenceManager};
    use crate::scheduler::SchedulerTickTestRuntime;
    use crate::session_pipeline::{
        emergency_stop_with_runtime_effects, resolve_post_break_decision_with_runtime_effects,
        sync_automation_policies_with_runtime_effects,
        sync_timer_duration_settings_with_runtime_effects,
        validate_post_break_decision_command_state, TimerCommandRuntimeEffectShell,
    };
    use crate::work_start::{
        determine_manual_start_origin,
        resolve_boot_auto_start_prompt_with_effects as resolve_boot_auto_start_prompt_contract,
        StartWorkRuntimeEffects,
    };
    use chrono::{Local, LocalResult, Utc};
    use std::future::Future;
    use std::path::{Path, PathBuf};
    use std::pin::Pin;
    use std::sync::atomic::{AtomicBool as StdAtomicBool, Ordering};
    use std::sync::{Arc, Mutex as StdMutex};
    use std::time::Duration as StdDuration;
    use std::time::{SystemTime, UNIX_EPOCH};

    use crate::types::{
        DayAutoRunSlot, HhMm, SessionTransitionReason, SessionType, TimerState, TimerTickPayload,
        WorkStartOrigin,
    };
    use chrono::TimeZone;
    use serde_json::json;

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

    struct ActiveCountdownRuntimeEffectShell {
        state: Arc<AppState>,
    }

    impl ActiveCountdownRuntimeEffectShell {
        fn new(state: Arc<AppState>) -> Self {
            Self { state }
        }
    }

    impl StartWorkRuntimeEffects for ActiveCountdownRuntimeEffectShell {
        fn emit_timer_tick(&mut self, _tick_payload: &TimerTickPayload) {}

        fn update_tray(&mut self, _tick_payload: &TimerTickPayload) {}

        fn emit_work_started(&mut self, _work_duration: u64) {}

        fn spawn_timer_task<'a>(
            &'a mut self,
            _state: &'a AppState,
        ) -> Pin<Box<dyn Future<Output = ()> + Send + 'a>> {
            let state = Arc::clone(&self.state);
            Box::pin(async move {
                install_timer_countdown_task(&state).await;
            })
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

    async fn build_test_state_with_persistence_dir(app_data_dir: PathBuf) -> AppState {
        let clock = fixed_clock(create_utc_datetime(2025, 2, 1, 9, 0, 0));
        let persistence = PersistenceManager::new(as_clock_trait(&clock), Some(app_data_dir));
        let (app_state, _) =
            AppState::build_startup_state_for_tests(as_clock_trait(&clock), persistence).await;
        app_state
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

    async fn build_test_state_with_injected_persistence_error() -> AppState {
        let clock = fixed_clock(create_utc_datetime(2025, 2, 1, 9, 0, 0));
        let persistence = PersistenceManager::new(as_clock_trait(&clock), Some(PathBuf::from(".")))
            .with_injected_save_error_hook(Arc::new(|| {
                Some(
                    crate::external_adapters::persistence::SaveError::Serialize {
                        source: serde_json::from_str::<serde_json::Value>("{")
                            .expect_err("invalid JSON should fail to parse"),
                    },
                )
            }));
        let (app_state, _) =
            AppState::build_startup_state_for_tests(as_clock_trait(&clock), persistence).await;
        app_state
    }

    async fn build_test_state_with_panicking_persistence_hook() -> AppState {
        let clock = fixed_clock(create_utc_datetime(2025, 2, 1, 9, 0, 0));
        let persistence = PersistenceManager::new(as_clock_trait(&clock), Some(PathBuf::from(".")))
            .with_injected_save_error_hook(Arc::new(|| {
                panic!("simulated persistence worker panic at C:\\Users\\nak\\secret")
            }));
        let (app_state, _) =
            AppState::build_startup_state_for_tests(as_clock_trait(&clock), persistence).await;
        app_state
    }

    fn timer_settings(
        work_duration_secs: u64,
        short_break_duration_secs: u64,
        long_break_duration_secs: u64,
        long_break_interval: u32,
        post_break_transition_policy: PostBreakTransitionPolicy,
    ) -> TimerSettings {
        TimerSettings::new(
            WorkDurationSecs::try_new(work_duration_secs).expect("work duration should be valid"),
            ShortBreakDurationSecs::try_new(short_break_duration_secs)
                .expect("short break duration should be valid"),
            LongBreakDurationSecs::try_new(long_break_duration_secs)
                .expect("long break duration should be valid"),
            LongBreakIntervalCount::try_new(long_break_interval)
                .expect("long break interval should be valid"),
            post_break_transition_policy,
        )
    }

    fn command_work_duration_secs() -> WorkDurationSecs {
        WorkDurationSecs::try_new(1800).expect("work duration should be valid")
    }

    fn command_short_break_duration_secs() -> ShortBreakDurationSecs {
        ShortBreakDurationSecs::try_new(360).expect("short break duration should be valid")
    }

    fn command_long_break_duration_secs() -> LongBreakDurationSecs {
        LongBreakDurationSecs::try_new(1200).expect("long break duration should be valid")
    }

    fn command_long_break_interval() -> LongBreakIntervalCount {
        LongBreakIntervalCount::try_new(5).expect("long break interval should be valid")
    }

    async fn prepare_post_break_decision_pending_state(state: &AppState) {
        let mut timer = state.timer_manager.lock().await;
        let settings = timer_settings(
            1500,
            300,
            900,
            4,
            PostBreakTransitionPolicy::RequireManualContinue,
        );
        timer
            .start_work(settings, WorkStartOrigin::ManualOutsideScheduleWindow)
            .expect("work session should start");
        timer.transition_to_break();
        timer.complete_break_session_transition();
        assert_eq!(timer.status(), TimerStatus::Idle);
        assert!(timer.get_state().post_break_decision_pending);
    }

    async fn apply_restored_post_break_decision_pending_state(state: &AppState) {
        let mut timer = state.timer_manager.lock().await;
        let settings = timer_settings(
            1500,
            300,
            900,
            4,
            PostBreakTransitionPolicy::RequireManualContinue,
        );
        let _ = timer.apply_persisted_state(TimerState {
            status: TimerStatus::Idle,
            remaining_seconds: settings.work_duration_secs.get(),
            session_type: SessionType::Work,
            current_cycle: 4,
            completed_pomodoros: 3,
            post_break_decision_pending: true,
            settings,
            ..TimerState::default()
        });
        assert_eq!(timer.status(), TimerStatus::Idle);
        assert!(timer.get_state().post_break_decision_pending);
    }

    async fn prepare_fullscreen_break_state(state: &AppState) {
        let mut timer = state.timer_manager.lock().await;
        let settings = timer_settings(
            1500,
            300,
            900,
            4,
            PostBreakTransitionPolicy::RequireManualContinue,
        );
        timer
            .start_work(settings, WorkStartOrigin::ManualOutsideScheduleWindow)
            .expect("work session should start");
        timer.transition_to_break();
        timer.set_fullscreen_break(true);
        assert_eq!(timer.status(), TimerStatus::OnBreak);
        assert!(timer.get_state().is_fullscreen_break);
    }

    async fn install_timer_countdown_task(state: &Arc<AppState>) {
        let state_for_task = Arc::downgrade(state);

        state.timer_manager.lock().await.spawn_timer_task(move || {
            let state_for_tick = std::sync::Weak::clone(&state_for_task);
            async move {
                if let Some(state) = state_for_tick.upgrade() {
                    let mut timer = state.timer_manager.lock().await;
                    if timer.status() != TimerStatus::Idle {
                        let _ = timer.tick();
                    }
                }
            }
        });
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

    fn assert_single_published_command_timer_state(
        runtime_effect_shell: &RecordingCommandRuntimeEffectShell,
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

    fn persisted_timer_state(state: &AppState) -> TimerState {
        match state.persistence.load_state_sync() {
            PersistenceLoadOutcome::Restored { timer_state, .. } => timer_state,
            PersistenceLoadOutcome::NoState => panic!("expected restored timer state"),
            PersistenceLoadOutcome::UnsupportedSchemaFallback => {
                panic!("expected restored timer state")
            }
        }
    }

    fn assert_persistence_save_error(result: &Result<()>) {
        assert!(matches!(result, Err(PomoliceError::PersistenceSave(_))));
    }

    fn assert_command_result_public_error<T>(
        result: Result<T>,
        expected_code: &str,
        diagnostic_fragments: &[&str],
    ) {
        let Err(error) = result else {
            panic!("command result should fail");
        };

        let serialized = serde_json::to_value(&error).expect("command error should serialize");
        let response = serialized
            .as_object()
            .expect("command error should serialize as a JSON object");

        assert_eq!(
            response
                .get("code")
                .expect("command error should include code")
                .as_str(),
            Some(expected_code)
        );
        let message = response
            .get("message")
            .expect("command error should include message")
            .as_str()
            .expect("command error message should be text");
        assert!(!message.is_empty());
        assert_public_error_excludes_retry_affordance(response);

        let serialized = serialized.to_string();
        for fragment in diagnostic_fragments {
            assert!(
                !serialized.contains(fragment),
                "public command error leaked diagnostic fragment: {fragment}"
            );
        }
    }

    fn assert_public_error_excludes_retry_affordance(
        response: &serde_json::Map<String, serde_json::Value>,
    ) {
        for key in response.keys() {
            let normalized = key.to_ascii_lowercase();
            assert!(
                !normalized.contains("retry") && !normalized.contains("transient"),
                "backend public command error encoded retry eligibility in field: {key}"
            );
        }
    }

    async fn wait_for_transition_event_count(
        events: &StdMutex<Vec<&'static str>>,
        expected_count: usize,
        context: &str,
    ) {
        for _ in 0..100 {
            let current_count = events
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .len();
            if current_count >= expected_count {
                return;
            }
            tokio::time::sleep(StdDuration::from_millis(10)).await;
        }

        panic!("timed out waiting for {context}");
    }

    async fn run_emergency_stop_exit_uses_break_mode_transition_coordinator_for_tests(
        state: Arc<AppState>,
        events: Arc<StdMutex<Vec<&'static str>>>,
        started_label: &'static str,
        completed_label: &'static str,
        release: Arc<StdAtomicBool>,
    ) {
        state
            .run_break_mode_transition(async move {
                {
                    let mut events = events
                        .lock()
                        .unwrap_or_else(std::sync::PoisonError::into_inner);
                    events.push(started_label);
                }

                while !release.load(Ordering::Acquire) {
                    tokio::time::sleep(StdDuration::from_millis(10)).await;
                }

                let mut events = events
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner);
                events.push(completed_label);
            })
            .await;
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
        let full_state = get_full_state_contract(state)
            .await
            .expect("full state should be available");

        assert!(!full_state.should_prompt_start_on_boot);
    }

    async fn assert_work_session_started_from_boundary(state: &AppState) {
        let timer = state.timer_manager.lock().await;
        let full_state = timer.get_full_state();

        assert_eq!(full_state.status, TimerStatus::Working);
        assert_eq!(
            full_state.remaining_seconds,
            full_state.settings.work_duration_secs.get()
        );
    }

    async fn assert_no_work_session_started_from_boundary(state: &AppState) {
        let timer = state.timer_manager.lock().await;
        let full_state = timer.get_full_state();

        assert_eq!(full_state.status, TimerStatus::Idle);
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

        let full_state = get_full_state_contract(&state)
            .await
            .expect("full state should be available");
        assert!(full_state.should_prompt_start_on_boot);
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

        let full_state = get_full_state_contract(&state)
            .await
            .expect("full state should be available");
        assert!(full_state.should_prompt_start_on_boot);
        assert!(full_state.active_window_summary.is_some());

        state
    }

    #[test]
    fn authorize_main_window_label_accepts_main() {
        assert!(authorize_main_window_label("main").is_ok());
    }

    #[test]
    fn authorize_main_window_label_rejects_break_overlay_window() {
        let result = authorize_main_window_label("break-overlay-1");

        assert!(matches!(
            result,
            Err(PomoliceError::Unauthorized(window_label)) if window_label == "break-overlay-1"
        ));
    }

    #[test]
    fn authorize_main_window_label_rejects_non_main_window() {
        let result = authorize_main_window_label("settings");

        assert!(matches!(
            result,
            Err(PomoliceError::Unauthorized(window_label)) if window_label == "settings"
        ));
    }

    #[tokio::test]
    async fn test_unauthorized_window_returns_public_command_error() {
        let state = build_test_state_without_persistence_path().await;
        let mut runtime_effect_shell = RecordingRuntimeEffectShell::default();
        let result = start_work_after_authorizing_window_label_for_tests(
            "C:\\Users\\nak\\secret\\settings",
            &state,
            TimerSettings::default(),
            WorkStartOrigin::ManualOutsideScheduleWindow,
            &mut runtime_effect_shell,
        )
        .await;

        assert_command_result_public_error(
            result,
            "unauthorized_window",
            &["C:\\Users\\nak", "secret", "settings"],
        );
        assert_no_start_runtime_effects(&runtime_effect_shell);
    }
    #[test]
    fn test_determine_manual_start_origin_returns_inside_when_enabled_and_in_window() {
        let now_local = Local
            .with_ymd_and_hms(2025, 1, 6, 10, 0, 0)
            .single()
            .expect("local datetime should be valid");
        let weekly_auto_run_config = make_enabled_daytime_monday_config();

        let origin = determine_manual_start_origin(now_local, true, &weekly_auto_run_config);

        assert_eq!(origin, WorkStartOrigin::ManualInsideScheduleWindow);
    }

    #[test]
    fn test_determine_manual_start_origin_returns_outside_when_enabled_and_out_of_window() {
        let now_local = Local
            .with_ymd_and_hms(2025, 1, 6, 8, 0, 0)
            .single()
            .expect("local datetime should be valid");
        let weekly_auto_run_config = make_enabled_daytime_monday_config();

        let origin = determine_manual_start_origin(now_local, true, &weekly_auto_run_config);

        assert_eq!(origin, WorkStartOrigin::ManualOutsideScheduleWindow);
    }

    #[test]
    fn test_determine_manual_start_origin_returns_outside_when_feature_disabled() {
        let now_local = Local
            .with_ymd_and_hms(2025, 1, 6, 10, 0, 0)
            .single()
            .expect("local datetime should be valid");
        let weekly_auto_run_config = make_enabled_daytime_monday_config();

        let origin = determine_manual_start_origin(now_local, false, &weekly_auto_run_config);

        assert_eq!(origin, WorkStartOrigin::ManualOutsideScheduleWindow);
    }

    #[tokio::test]
    async fn get_full_state_prompts_on_first_idle_sync_inside_enabled_window() {
        let state = build_test_state_at(local_datetime(2025, 1, 6, 10, 0, 0)).await;
        sync_auto_run_policies_for_test(
            &state,
            make_enabled_daytime_monday_config(),
            PostBreakTransitionPolicy::RequireManualContinue,
        )
        .await;

        let full_state = get_full_state_contract(&state)
            .await
            .expect("full state should be available");

        assert_eq!(full_state.status, TimerStatus::Idle);
        assert!(full_state.should_prompt_start_on_boot);
        assert_daytime_monday_window(&full_state);
    }

    #[tokio::test]
    async fn get_full_state_hides_active_window_when_auto_run_is_disabled() {
        let state = build_test_state_at(local_datetime(2025, 1, 6, 10, 0, 0)).await;
        sync_auto_run_policies_with_feature_for_test(
            &state,
            false,
            make_enabled_daytime_monday_config(),
            PostBreakTransitionPolicy::RequireManualContinue,
        )
        .await;

        let full_state = get_full_state_contract(&state)
            .await
            .expect("full state should be available");

        assert_eq!(full_state.status, TimerStatus::Idle);
        assert!(!full_state.should_prompt_start_on_boot);
        assert!(full_state.active_window_summary.is_none());
    }

    #[tokio::test]
    async fn get_full_state_prompts_only_on_first_full_sync() {
        let state = build_test_state_at(local_datetime(2025, 1, 6, 10, 0, 0)).await;
        sync_auto_run_policies_for_test(
            &state,
            make_enabled_daytime_monday_config(),
            PostBreakTransitionPolicy::RequireManualContinue,
        )
        .await;

        let first_full_state = get_full_state_contract(&state)
            .await
            .expect("first full state should be available");
        let second_full_state = get_full_state_contract(&state)
            .await
            .expect("second full state should be available");

        assert!(first_full_state.should_prompt_start_on_boot);
        assert_daytime_monday_window(&first_full_state);
        assert!(!second_full_state.should_prompt_start_on_boot);
        assert_daytime_monday_window(&second_full_state);
    }

    #[tokio::test]
    async fn get_full_state_does_not_prompt_when_timer_is_already_running() {
        let state = build_test_state_at(local_datetime(2025, 1, 6, 10, 0, 0)).await;
        sync_auto_run_policies_for_test(
            &state,
            make_enabled_daytime_monday_config(),
            PostBreakTransitionPolicy::RequireManualContinue,
        )
        .await;
        {
            let mut timer = state.timer_manager.lock().await;
            timer
                .start_work(
                    TimerSettings::default(),
                    WorkStartOrigin::ManualInsideScheduleWindow,
                )
                .expect("work session should start");
        }

        let full_state = get_full_state_contract(&state)
            .await
            .expect("full state should be available");

        assert_eq!(full_state.status, TimerStatus::Working);
        assert!(!full_state.should_prompt_start_on_boot);
        assert_daytime_monday_window(&full_state);
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
            SchedulerTickTestRuntime::from_state(&state, local_datetime(2025, 1, 6, 8, 59, 50));

        scheduler_runtime
            .run_tick(&state, local_datetime(2025, 1, 6, 9, 0, 10))
            .await;

        assert_no_work_session_started_from_boundary(&state).await;
        let full_state = get_full_state_contract(&state)
            .await
            .expect("initial full state should still be available");
        assert!(full_state.should_prompt_start_on_boot);
        assert_daytime_monday_window(&full_state);
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
                SchedulerTickTestRuntime::from_state(&state, local_datetime(2025, 1, 6, 8, 59, 50));

            resolve_boot_auto_start_prompt_contract(&state, start_now, &mut runtime_effect_shell)
                .await
                .expect("unexpected boot prompt resolution should be ignored");
            scheduler_runtime
                .run_tick(&state, local_datetime(2025, 1, 6, 9, 0, 10))
                .await;

            assert_no_start_runtime_effects(&runtime_effect_shell);
            assert_no_work_session_started_from_boundary(&state).await;
            let full_state = get_full_state_contract(&state)
                .await
                .expect("initial full state should still decide startup prompt");
            assert!(full_state.should_prompt_start_on_boot);
            assert_daytime_monday_window(&full_state);
        }
    }

    #[tokio::test]
    async fn pending_boot_prompt_prevents_scheduled_work_start_after_follow_up_full_sync() {
        let state = build_prompting_state(
            local_datetime(2025, 1, 6, 9, 0, 5),
            make_enabled_daytime_monday_config(),
        )
        .await;
        let second_full_state = get_full_state_contract(&state)
            .await
            .expect("second full state should be available");
        let mut scheduler_runtime =
            SchedulerTickTestRuntime::from_state(&state, local_datetime(2025, 1, 6, 8, 59, 50));

        scheduler_runtime
            .run_tick(&state, local_datetime(2025, 1, 6, 9, 0, 10))
            .await;

        assert!(!second_full_state.should_prompt_start_on_boot);
        assert_no_work_session_started_from_boundary(&state).await;
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
        let full_state = get_full_state_contract(&state)
            .await
            .expect("full state should be available");
        let mut scheduler_runtime =
            SchedulerTickTestRuntime::from_state(&state, local_datetime(2025, 1, 6, 8, 59, 50));

        scheduler_runtime
            .run_tick(&state, local_datetime(2025, 1, 6, 9, 0, 10))
            .await;

        assert!(!full_state.should_prompt_start_on_boot);
        assert!(full_state.active_window_summary.is_none());
        assert_work_session_started_from_boundary(&state).await;
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
        let full_state = get_full_state_contract(&state)
            .await
            .expect("full state should be available");
        let mut stop_runtime_effect_shell = RecordingCommandRuntimeEffectShell::default();
        emergency_stop_with_runtime_effects(&state, &mut stop_runtime_effect_shell)
            .await
            .expect("emergency stop should succeed");
        let mut scheduler_runtime =
            SchedulerTickTestRuntime::from_state(&state, local_datetime(2025, 1, 7, 8, 59, 50));

        scheduler_runtime
            .run_tick(&state, local_datetime(2025, 1, 7, 9, 0, 10))
            .await;

        assert_eq!(full_state.status, TimerStatus::Working);
        assert!(!full_state.should_prompt_start_on_boot);
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
            SchedulerTickTestRuntime::from_state(&state, local_datetime(2025, 1, 6, 8, 59, 50));

        scheduler_runtime
            .run_tick(&state, local_datetime(2025, 1, 6, 9, 0, 10))
            .await;

        let full_state = state.timer_manager.lock().await.get_full_state();
        assert_eq!(full_state.status, TimerStatus::Idle);
        assert_eq!(full_state.session_type, SessionType::Work);
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
    async fn resolved_later_allows_next_scheduled_start_boundary() {
        let state = build_prompting_state(
            local_datetime(2025, 1, 6, 10, 0, 0),
            monday_and_tuesday_config("09:00", "18:00"),
        )
        .await;
        let mut runtime_effect_shell = RecordingRuntimeEffectShell::default();
        let mut scheduler_runtime =
            SchedulerTickTestRuntime::from_state(&state, local_datetime(2025, 1, 6, 10, 0, 0));

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
            SchedulerTickTestRuntime::from_state(&state, local_datetime(2025, 1, 7, 8, 59, 50));
        scheduler_runtime
            .run_tick(&state, local_datetime(2025, 1, 7, 9, 0, 10))
            .await;

        assert_work_session_started_from_boundary(&state).await;
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

    #[test]
    fn test_work_duration_secs_boundary_deserialize_rejects_zero() {
        let result = serde_json::from_value::<WorkDurationSecs>(json!(0));
        assert!(result.is_err());
    }

    #[test]
    fn test_long_break_interval_boundary_deserialize_rejects_zero() {
        let result = serde_json::from_value::<LongBreakIntervalCount>(json!(0));
        assert!(result.is_err());
    }

    #[test]
    fn test_day_auto_run_slot_boundary_deserialize_rejects_equal_start_and_end() {
        let result = serde_json::from_value::<DayAutoRunSlot>(json!({
            "enabled": true,
            "startTime": "09:00",
            "endTime": "09:00"
        }));
        assert!(result.is_err());
    }

    #[test]
    fn test_weekly_auto_run_config_boundary_deserialize_rejects_overlap() {
        let result = serde_json::from_value::<WeeklyAutoRunConfig>(json!({
            "mon": {"enabled": true, "startTime": "22:00", "endTime": "02:00"},
            "tue": {"enabled": true, "startTime": "01:00", "endTime": "05:00"},
            "wed": {"enabled": false, "startTime": "09:00", "endTime": "18:00"},
            "thu": {"enabled": false, "startTime": "09:00", "endTime": "18:00"},
            "fri": {"enabled": false, "startTime": "09:00", "endTime": "18:00"},
            "sat": {"enabled": false, "startTime": "09:00", "endTime": "18:00"},
            "sun": {"enabled": false, "startTime": "09:00", "endTime": "18:00"}
        }));
        assert!(result.is_err());
    }

    #[test]
    fn test_weekly_auto_run_config_boundary_deserialize_rejects_invalid_start_time() {
        let result = serde_json::from_value::<WeeklyAutoRunConfig>(json!({
            "mon": {"enabled": true, "startTime": "24:00", "endTime": "02:00"},
            "tue": {"enabled": false, "startTime": "09:00", "endTime": "18:00"},
            "wed": {"enabled": false, "startTime": "09:00", "endTime": "18:00"},
            "thu": {"enabled": false, "startTime": "09:00", "endTime": "18:00"},
            "fri": {"enabled": false, "startTime": "09:00", "endTime": "18:00"},
            "sat": {"enabled": false, "startTime": "09:00", "endTime": "18:00"},
            "sun": {"enabled": false, "startTime": "09:00", "endTime": "18:00"}
        }));
        assert!(result.is_err());
    }

    #[test]
    fn test_validate_post_break_decision_command_state_accepts_idle_pending_state() {
        let result = validate_post_break_decision_command_state(TimerStatus::Idle, true);
        assert!(result.is_ok());
    }

    #[test]
    fn test_validate_post_break_decision_command_state_rejects_idle_without_pending() {
        let result = validate_post_break_decision_command_state(TimerStatus::Idle, false);
        assert!(matches!(
            result,
            Err(PomoliceError::InvalidStateTransition { .. })
        ));
    }

    #[test]
    fn test_validate_post_break_decision_command_state_rejects_non_idle_even_when_pending() {
        let result = validate_post_break_decision_command_state(TimerStatus::Working, true);
        assert!(matches!(
            result,
            Err(PomoliceError::InvalidStateTransition { .. })
        ));
    }

    #[tokio::test]
    async fn test_resolve_post_break_decision_invalid_state_returns_public_command_error() {
        let state = build_test_state_without_persistence_path().await;
        let mut runtime_effect_shell = RecordingCommandRuntimeEffectShell::default();

        let result = resolve_post_break_decision_with_runtime_effects(
            &state,
            PostBreakDecisionAction::Continue,
            &mut runtime_effect_shell,
        )
        .await;

        assert_command_result_public_error(
            result,
            "invalid_state_transition",
            &["resolve post-break decision", "Idle"],
        );
    }

    #[tokio::test]
    async fn test_start_work_rate_limit_returns_public_command_error() {
        let state = build_test_state_without_persistence_path().await;
        let mut first_runtime_effect_shell = RecordingRuntimeEffectShell::default();

        start_work_for_tests(
            &state,
            timer_settings(
                1500,
                300,
                900,
                4,
                PostBreakTransitionPolicy::RequireManualContinue,
            ),
            WorkStartOrigin::ManualOutsideScheduleWindow,
            &mut first_runtime_effect_shell,
        )
        .await
        .expect("first start should succeed");

        {
            let mut timer = state.timer_manager.lock().await;
            timer.emergency_stop();
        }

        let mut second_runtime_effect_shell = RecordingRuntimeEffectShell::default();
        let result = start_work_for_tests(
            &state,
            timer_settings(
                1500,
                300,
                900,
                4,
                PostBreakTransitionPolicy::RequireManualContinue,
            ),
            WorkStartOrigin::ManualOutsideScheduleWindow,
            &mut second_runtime_effect_shell,
        )
        .await;

        assert_command_result_public_error(result, "rate_limited", &["please wait", "ms"]);
        assert_no_start_runtime_effects(&second_runtime_effect_shell);
    }

    #[tokio::test(start_paused = true)]
    async fn test_start_work_rejected_during_break_keeps_break_countdown_running() {
        let state = Arc::new(build_test_state_without_persistence_path().await);
        prepare_fullscreen_break_state(&state).await;
        install_timer_countdown_task(&state).await;

        tokio::task::yield_now().await;
        tokio::time::advance(StdDuration::from_secs(1)).await;
        tokio::task::yield_now().await;

        let remaining_after_first_tick = {
            let timer = state.timer_manager.lock().await;
            let full_state = timer.get_full_state();
            assert_eq!(full_state.status, TimerStatus::OnBreak);
            assert!(full_state.is_fullscreen_break);
            assert!(full_state.remaining_seconds < 300);
            full_state.remaining_seconds
        };

        let mut runtime_effect_shell = RecordingRuntimeEffectShell::default();
        let result = start_work_for_tests(
            state.as_ref(),
            timer_settings(
                1500,
                300,
                900,
                4,
                PostBreakTransitionPolicy::RequireManualContinue,
            ),
            WorkStartOrigin::ManualOutsideScheduleWindow,
            &mut runtime_effect_shell,
        )
        .await;

        assert_command_result_public_error(result, "invalid_state_transition", &["start work"]);

        let remaining_after_rejection = {
            let timer = state.timer_manager.lock().await;
            let full_state = timer.get_full_state();
            assert_eq!(full_state.status, TimerStatus::OnBreak);
            assert!(full_state.is_fullscreen_break);
            assert_eq!(full_state.remaining_seconds, remaining_after_first_tick);
            full_state.remaining_seconds
        };

        tokio::time::advance(StdDuration::from_secs(3)).await;
        tokio::task::yield_now().await;

        let timer = state.timer_manager.lock().await;
        let full_state = timer.get_full_state();
        assert_eq!(full_state.status, TimerStatus::OnBreak);
        assert!(full_state.is_fullscreen_break);
        assert!(full_state.remaining_seconds < remaining_after_rejection);
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
            SchedulerTickTestRuntime::from_state(&state, local_datetime(2025, 1, 6, 8, 59, 50));

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
            SchedulerTickTestRuntime::from_state(&state, local_datetime(2025, 1, 6, 8, 59, 50));

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
            SchedulerTickTestRuntime::from_state(&state, local_datetime(2025, 1, 6, 8, 59, 50));

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
            SchedulerTickTestRuntime::from_state(&state, local_datetime(2025, 1, 6, 8, 59, 50));

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
            SchedulerTickTestRuntime::from_state(&state, local_datetime(2025, 1, 6, 8, 59, 50));

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
            SchedulerTickTestRuntime::from_state(&state, local_datetime(2025, 1, 6, 17, 59, 50));

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
            SchedulerTickTestRuntime::from_state(&state, local_datetime(2025, 1, 6, 17, 59, 50));

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
            SchedulerTickTestRuntime::from_state(&state, local_datetime(2025, 1, 6, 17, 59, 50));

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

    #[tokio::test(start_paused = true)]
    async fn continue_post_break_decision_publishes_work_state() {
        let state = Arc::new(build_test_state_without_persistence_path().await);
        prepare_post_break_decision_pending_state(&state).await;
        install_timer_countdown_task(&state).await;
        let mut runtime_effect_shell = RecordingCommandRuntimeEffectShell::default();

        resolve_post_break_decision_with_runtime_effects(
            &state,
            PostBreakDecisionAction::Continue,
            &mut runtime_effect_shell,
        )
        .await
        .expect("continue decision should succeed");

        let published = assert_single_published_command_timer_state(
            &runtime_effect_shell,
            TimerStatus::Working,
            SessionType::Work,
        );
        assert_eq!(runtime_effect_shell.fullscreen_break_exit_requests, 0);
        tokio::time::advance(StdDuration::from_secs(3)).await;
        tokio::task::yield_now().await;

        let timer = state.timer_manager.lock().await;
        let timer_state = timer.get_state();
        assert_eq!(timer_state.status, TimerStatus::Working);
        assert_eq!(timer_state.session_type, SessionType::Work);
        assert_eq!(timer_state.remaining_seconds, 1500);
        assert!(!timer_state.post_break_decision_pending);
        assert_eq!(published.remaining_seconds, 1500);
    }

    #[tokio::test]
    async fn test_resolve_post_break_decision_continue_after_restore_starts_work() {
        let state = build_test_state_without_persistence_path().await;
        apply_restored_post_break_decision_pending_state(&state).await;
        let mut runtime_effect_shell = RecordingCommandRuntimeEffectShell::default();

        resolve_post_break_decision_with_runtime_effects(
            &state,
            PostBreakDecisionAction::Continue,
            &mut runtime_effect_shell,
        )
        .await
        .expect("restored continue decision should succeed");

        let published = assert_single_published_command_timer_state(
            &runtime_effect_shell,
            TimerStatus::Working,
            SessionType::Work,
        );

        let timer = state.timer_manager.lock().await;
        let timer_state = timer.get_state();
        assert_eq!(timer_state.status, TimerStatus::Working);
        assert_eq!(timer_state.session_type, SessionType::Work);
        assert_eq!(timer_state.remaining_seconds, 1500);
        assert!(!timer_state.post_break_decision_pending);
        assert_eq!(published.remaining_seconds, 1500);
    }

    #[tokio::test]
    async fn test_resolve_post_break_decision_stop_after_restore_returns_to_idle() {
        let state = build_test_state_without_persistence_path().await;
        apply_restored_post_break_decision_pending_state(&state).await;
        let mut runtime_effect_shell = RecordingCommandRuntimeEffectShell::default();

        resolve_post_break_decision_with_runtime_effects(
            &state,
            PostBreakDecisionAction::StopCycle,
            &mut runtime_effect_shell,
        )
        .await
        .expect("restored stop decision should succeed");

        let published = assert_single_published_command_timer_state(
            &runtime_effect_shell,
            TimerStatus::Idle,
            SessionType::Work,
        );

        let timer = state.timer_manager.lock().await;
        let timer_state = timer.get_state();
        assert_eq!(timer_state.status, TimerStatus::Idle);
        assert_eq!(timer_state.session_type, SessionType::Work);
        assert_eq!(timer_state.remaining_seconds, 1500);
        assert!(!timer_state.post_break_decision_pending);
        assert_eq!(published.remaining_seconds, 1500);
    }

    #[tokio::test]
    async fn stop_post_break_decision_publishes_idle_state() {
        let state = build_test_state_without_persistence_path().await;
        prepare_post_break_decision_pending_state(&state).await;
        let mut runtime_effect_shell = RecordingCommandRuntimeEffectShell::default();

        resolve_post_break_decision_with_runtime_effects(
            &state,
            PostBreakDecisionAction::StopCycle,
            &mut runtime_effect_shell,
        )
        .await
        .expect("stop decision should succeed");

        let published = assert_single_published_command_timer_state(
            &runtime_effect_shell,
            TimerStatus::Idle,
            SessionType::Work,
        );
        assert_eq!(runtime_effect_shell.fullscreen_break_exit_requests, 0);

        let timer = state.timer_manager.lock().await;
        assert_eq!(timer.status(), TimerStatus::Idle);
        assert!(!timer.get_state().post_break_decision_pending);
        assert_eq!(published.status, TimerStatus::Idle);
    }

    #[tokio::test]
    async fn emergency_stop_publishes_idle_state_and_requests_break_exit_when_fullscreen() {
        let fullscreen_state = build_test_state_without_persistence_path().await;
        prepare_fullscreen_break_state(&fullscreen_state).await;
        let mut fullscreen_runtime_effect_shell = RecordingCommandRuntimeEffectShell::default();

        emergency_stop_with_runtime_effects(
            &fullscreen_state,
            &mut fullscreen_runtime_effect_shell,
        )
        .await
        .expect("emergency stop should succeed");

        let fullscreen_published = assert_single_published_command_timer_state(
            &fullscreen_runtime_effect_shell,
            TimerStatus::Idle,
            SessionType::Work,
        );
        assert_eq!(
            fullscreen_runtime_effect_shell.fullscreen_break_exit_requests,
            1
        );

        {
            let fullscreen_timer = fullscreen_state.timer_manager.lock().await;
            assert_eq!(fullscreen_timer.status(), TimerStatus::Idle);
            assert!(!fullscreen_timer.get_state().is_fullscreen_break);
        }
        assert_eq!(fullscreen_published.status, TimerStatus::Idle);
        assert!(!fullscreen_published.is_fullscreen_break);

        let non_fullscreen_state = build_test_state_without_persistence_path().await;
        {
            let mut timer = non_fullscreen_state.timer_manager.lock().await;
            timer
                .start_work(
                    timer_settings(
                        1500,
                        300,
                        900,
                        4,
                        PostBreakTransitionPolicy::RequireManualContinue,
                    ),
                    WorkStartOrigin::ManualOutsideScheduleWindow,
                )
                .expect("work session should start");
        }
        let mut non_fullscreen_runtime_effect_shell = RecordingCommandRuntimeEffectShell::default();

        emergency_stop_with_runtime_effects(
            &non_fullscreen_state,
            &mut non_fullscreen_runtime_effect_shell,
        )
        .await
        .expect("emergency stop should succeed");

        assert_single_published_command_timer_state(
            &non_fullscreen_runtime_effect_shell,
            TimerStatus::Idle,
            SessionType::Work,
        );
        assert_eq!(
            non_fullscreen_runtime_effect_shell.fullscreen_break_exit_requests,
            0
        );
    }

    #[tokio::test(start_paused = true)]
    async fn test_emergency_stop_prevents_previous_countdown_from_affecting_next_work_session() {
        let state = Arc::new(build_test_state_without_persistence_path().await);
        prepare_fullscreen_break_state(&state).await;
        install_timer_countdown_task(&state).await;

        tokio::task::yield_now().await;
        tokio::time::advance(StdDuration::from_secs(1)).await;
        tokio::task::yield_now().await;

        {
            let timer = state.timer_manager.lock().await;
            let timer_state = timer.get_state();
            assert_eq!(timer_state.status, TimerStatus::OnBreak);
            assert_eq!(timer_state.remaining_seconds, 299);
            assert!(timer_state.is_fullscreen_break);
        }

        let mut stop_runtime_effect_shell = RecordingCommandRuntimeEffectShell::default();
        emergency_stop_with_runtime_effects(&state, &mut stop_runtime_effect_shell)
            .await
            .expect("emergency stop should succeed");

        {
            let timer = state.timer_manager.lock().await;
            let timer_state = timer.get_state();
            assert_eq!(timer_state.status, TimerStatus::Idle);
            assert!(!timer_state.is_fullscreen_break);
        }

        let mut start_runtime_effect_shell =
            ActiveCountdownRuntimeEffectShell::new(Arc::clone(&state));
        start_work_for_tests(
            &state,
            timer_settings(
                1500,
                300,
                900,
                4,
                PostBreakTransitionPolicy::RequireManualContinue,
            ),
            WorkStartOrigin::ScheduleStartBoundary,
            &mut start_runtime_effect_shell,
        )
        .await
        .expect("new work session should start");

        {
            let timer = state.timer_manager.lock().await;
            let timer_state = timer.get_state();
            assert_eq!(timer_state.status, TimerStatus::Working);
            assert_eq!(timer_state.session_type, SessionType::Work);
            assert_eq!(timer_state.remaining_seconds, 1500);
        }

        tokio::task::yield_now().await;
        tokio::time::advance(StdDuration::from_secs(1)).await;
        tokio::task::yield_now().await;

        let timer = state.timer_manager.lock().await;
        let timer_state = timer.get_state();
        assert_eq!(timer_state.status, TimerStatus::Working);
        assert_eq!(timer_state.session_type, SessionType::Work);
        assert_eq!(timer_state.remaining_seconds, 1499);
    }

    #[tokio::test]
    async fn duration_sync_publishes_updated_idle_timer_state() {
        let state = build_test_state_without_persistence_path().await;
        let mut runtime_effect_shell = RecordingCommandRuntimeEffectShell::default();
        let expected_work_duration_secs = command_work_duration_secs();

        sync_timer_duration_settings_with_runtime_effects(
            &state,
            expected_work_duration_secs,
            command_short_break_duration_secs(),
            command_long_break_duration_secs(),
            command_long_break_interval(),
            &mut runtime_effect_shell,
        )
        .await
        .expect("duration settings sync should succeed");

        let published = assert_single_published_command_timer_state(
            &runtime_effect_shell,
            TimerStatus::Idle,
            SessionType::Work,
        );
        assert_eq!(runtime_effect_shell.fullscreen_break_exit_requests, 0);

        let timer = state.timer_manager.lock().await;
        assert_eq!(timer.status(), TimerStatus::Idle);
        assert_eq!(
            timer.get_state().settings.work_duration_secs,
            expected_work_duration_secs
        );
        assert_eq!(
            published.remaining_seconds,
            expected_work_duration_secs.get()
        );
    }

    #[tokio::test]
    async fn automation_policy_sync_publishes_updated_policy_state() {
        let state = build_test_state_without_persistence_path().await;
        let mut runtime_effect_shell = RecordingCommandRuntimeEffectShell::default();

        sync_automation_policies_with_runtime_effects(
            &state,
            false,
            WeeklyAutoRunConfig::default(),
            PostBreakTransitionPolicy::StartNextWorkAutomatically,
            &mut runtime_effect_shell,
        )
        .await
        .expect("automation policy sync should succeed");

        assert_single_published_command_timer_state(
            &runtime_effect_shell,
            TimerStatus::Idle,
            SessionType::Work,
        );
        assert_eq!(runtime_effect_shell.fullscreen_break_exit_requests, 0);

        let timer = state.timer_manager.lock().await;
        assert_eq!(
            timer.get_state().settings.post_break_transition_policy,
            PostBreakTransitionPolicy::StartNextWorkAutomatically
        );
    }

    #[tokio::test]
    async fn start_work_persists_started_work_state() {
        let temp_dir = create_temp_dir("start-work-persists-started-work");
        let state = build_test_state_with_persistence_dir(temp_dir.clone()).await;
        let settings = timer_settings(
            1500,
            300,
            900,
            4,
            PostBreakTransitionPolicy::RequireManualContinue,
        );
        let mut runtime_effect_shell = RecordingRuntimeEffectShell::default();

        start_work_for_tests(
            &state,
            settings,
            WorkStartOrigin::ManualOutsideScheduleWindow,
            &mut runtime_effect_shell,
        )
        .await
        .expect("work session should start");

        assert_single_published_start_timer_state(
            &runtime_effect_shell,
            TimerStatus::Working,
            SessionType::Work,
        );
        assert_start_work_started_duration(&runtime_effect_shell, 1500);

        let persisted = persisted_timer_state(&state);
        assert_eq!(persisted.status, TimerStatus::Working);
        assert_eq!(persisted.session_type, SessionType::Work);
        assert_eq!(persisted.remaining_seconds, 1500);

        cleanup_temp_dir(&temp_dir);
    }

    #[tokio::test]
    async fn continue_post_break_decision_persists_next_work_state() {
        let temp_dir = create_temp_dir("continue-post-break-persists-work");
        let state = build_test_state_with_persistence_dir(temp_dir.clone()).await;
        prepare_post_break_decision_pending_state(&state).await;
        let mut runtime_effect_shell = RecordingCommandRuntimeEffectShell::default();

        resolve_post_break_decision_with_runtime_effects(
            &state,
            PostBreakDecisionAction::Continue,
            &mut runtime_effect_shell,
        )
        .await
        .expect("continue decision should succeed");

        assert_single_published_command_timer_state(
            &runtime_effect_shell,
            TimerStatus::Working,
            SessionType::Work,
        );

        let persisted = persisted_timer_state(&state);
        assert_eq!(persisted.status, TimerStatus::Working);
        assert_eq!(persisted.session_type, SessionType::Work);
        assert_eq!(persisted.remaining_seconds, 1500);
        assert!(!persisted.post_break_decision_pending);

        cleanup_temp_dir(&temp_dir);
    }

    #[tokio::test]
    async fn stop_post_break_decision_persists_idle_state() {
        let temp_dir = create_temp_dir("stop-post-break-persists-idle");
        let state = build_test_state_with_persistence_dir(temp_dir.clone()).await;
        prepare_post_break_decision_pending_state(&state).await;
        let mut runtime_effect_shell = RecordingCommandRuntimeEffectShell::default();

        resolve_post_break_decision_with_runtime_effects(
            &state,
            PostBreakDecisionAction::StopCycle,
            &mut runtime_effect_shell,
        )
        .await
        .expect("stop decision should succeed");

        assert_single_published_command_timer_state(
            &runtime_effect_shell,
            TimerStatus::Idle,
            SessionType::Work,
        );

        let persisted = persisted_timer_state(&state);
        assert_eq!(persisted.status, TimerStatus::Idle);
        assert_eq!(persisted.session_type, SessionType::Work);
        assert!(!persisted.post_break_decision_pending);

        cleanup_temp_dir(&temp_dir);
    }

    #[tokio::test]
    async fn emergency_stop_persists_idle_state() {
        let temp_dir = create_temp_dir("emergency-stop-persists-idle");
        let state = build_test_state_with_persistence_dir(temp_dir.clone()).await;
        prepare_fullscreen_break_state(&state).await;
        let mut runtime_effect_shell = RecordingCommandRuntimeEffectShell::default();

        emergency_stop_with_runtime_effects(&state, &mut runtime_effect_shell)
            .await
            .expect("emergency stop should succeed");

        assert_single_published_command_timer_state(
            &runtime_effect_shell,
            TimerStatus::Idle,
            SessionType::Work,
        );
        assert_eq!(runtime_effect_shell.fullscreen_break_exit_requests, 1);

        let persisted = persisted_timer_state(&state);
        assert_eq!(persisted.status, TimerStatus::Idle);
        assert!(!persisted.is_fullscreen_break);

        cleanup_temp_dir(&temp_dir);
    }

    #[tokio::test]
    async fn duration_sync_persists_updated_settings() {
        let temp_dir = create_temp_dir("duration-sync-persists-settings");
        let state = build_test_state_with_persistence_dir(temp_dir.clone()).await;
        let mut runtime_effect_shell = RecordingCommandRuntimeEffectShell::default();
        let expected_work_duration_secs = command_work_duration_secs();
        let expected_short_break_duration_secs = command_short_break_duration_secs();
        let expected_long_break_duration_secs = command_long_break_duration_secs();
        let expected_long_break_interval = command_long_break_interval();

        sync_timer_duration_settings_with_runtime_effects(
            &state,
            expected_work_duration_secs,
            expected_short_break_duration_secs,
            expected_long_break_duration_secs,
            expected_long_break_interval,
            &mut runtime_effect_shell,
        )
        .await
        .expect("duration settings sync should succeed");

        assert_single_published_command_timer_state(
            &runtime_effect_shell,
            TimerStatus::Idle,
            SessionType::Work,
        );

        let persisted = persisted_timer_state(&state);
        assert_eq!(persisted.status, TimerStatus::Idle);
        assert_eq!(
            persisted.remaining_seconds,
            expected_work_duration_secs.get()
        );
        assert_eq!(
            persisted.settings.work_duration_secs,
            expected_work_duration_secs
        );
        assert_eq!(
            persisted.settings.short_break_duration_secs,
            expected_short_break_duration_secs
        );
        assert_eq!(
            persisted.settings.long_break_duration_secs,
            expected_long_break_duration_secs
        );
        assert_eq!(
            persisted.settings.long_break_interval,
            expected_long_break_interval
        );

        cleanup_temp_dir(&temp_dir);
    }

    #[tokio::test]
    async fn automation_policy_sync_persists_updated_policy() {
        let temp_dir = create_temp_dir("automation-policy-sync-persists-policy");
        let state = build_test_state_with_persistence_dir(temp_dir.clone()).await;
        let mut runtime_effect_shell = RecordingCommandRuntimeEffectShell::default();
        let expected_weekly_config = make_enabled_daytime_monday_config();

        sync_automation_policies_with_runtime_effects(
            &state,
            true,
            expected_weekly_config.clone(),
            PostBreakTransitionPolicy::StartNextWorkAutomatically,
            &mut runtime_effect_shell,
        )
        .await
        .expect("automation policy sync should succeed");

        assert_single_published_command_timer_state(
            &runtime_effect_shell,
            TimerStatus::Idle,
            SessionType::Work,
        );

        let persisted = persisted_timer_state(&state);
        assert!(persisted.auto_run_feature_enabled);
        assert_eq!(persisted.weekly_auto_run_config, expected_weekly_config);
        assert_eq!(
            persisted.settings.post_break_transition_policy,
            PostBreakTransitionPolicy::StartNextWorkAutomatically
        );

        cleanup_temp_dir(&temp_dir);
    }

    #[tokio::test]
    async fn continue_post_break_decision_returns_persistence_error_without_rolling_back_work_start(
    ) {
        let continue_state = build_test_state_with_injected_persistence_error().await;
        prepare_post_break_decision_pending_state(&continue_state).await;
        let mut continue_runtime_effect_shell = RecordingCommandRuntimeEffectShell::default();

        let continue_result = resolve_post_break_decision_with_runtime_effects(
            &continue_state,
            PostBreakDecisionAction::Continue,
            &mut continue_runtime_effect_shell,
        )
        .await;

        assert_persistence_save_error(&continue_result);
        let published = assert_single_published_command_timer_state(
            &continue_runtime_effect_shell,
            TimerStatus::Working,
            SessionType::Work,
        );
        assert_eq!(published.remaining_seconds, 1500);

        let timer_state = continue_state
            .timer_manager
            .lock()
            .await
            .get_state()
            .clone();
        assert_eq!(timer_state.status, TimerStatus::Working);
        assert_eq!(timer_state.session_type, SessionType::Work);
        assert_eq!(timer_state.remaining_seconds, 1500);
        assert!(!timer_state.post_break_decision_pending);
    }

    #[tokio::test]
    async fn emergency_stop_returns_persistence_error_without_rolling_back_idle_state() {
        let emergency_state = build_test_state_with_injected_persistence_error().await;
        prepare_fullscreen_break_state(&emergency_state).await;
        let mut emergency_runtime_effect_shell = RecordingCommandRuntimeEffectShell::default();

        let emergency_result = emergency_stop_with_runtime_effects(
            &emergency_state,
            &mut emergency_runtime_effect_shell,
        )
        .await;

        assert_persistence_save_error(&emergency_result);
        let published = assert_single_published_command_timer_state(
            &emergency_runtime_effect_shell,
            TimerStatus::Idle,
            SessionType::Work,
        );
        assert_eq!(
            emergency_runtime_effect_shell.fullscreen_break_exit_requests,
            1
        );
        assert!(!published.is_fullscreen_break);

        let timer_state = emergency_state
            .timer_manager
            .lock()
            .await
            .get_state()
            .clone();
        assert_eq!(timer_state.status, TimerStatus::Idle);
        assert!(!timer_state.is_fullscreen_break);
    }

    #[tokio::test]
    async fn duration_sync_returns_persistence_error_without_rolling_back_settings() {
        let duration_state = build_test_state_with_injected_persistence_error().await;
        let mut duration_runtime_effect_shell = RecordingCommandRuntimeEffectShell::default();
        let expected_work_duration_secs = command_work_duration_secs();

        let duration_result = sync_timer_duration_settings_with_runtime_effects(
            &duration_state,
            expected_work_duration_secs,
            command_short_break_duration_secs(),
            command_long_break_duration_secs(),
            command_long_break_interval(),
            &mut duration_runtime_effect_shell,
        )
        .await;

        assert_persistence_save_error(&duration_result);
        let published = assert_single_published_command_timer_state(
            &duration_runtime_effect_shell,
            TimerStatus::Idle,
            SessionType::Work,
        );
        assert_eq!(
            published.remaining_seconds,
            expected_work_duration_secs.get()
        );

        let timer_state = duration_state
            .timer_manager
            .lock()
            .await
            .get_state()
            .clone();
        assert_eq!(
            timer_state.settings.work_duration_secs,
            expected_work_duration_secs
        );
        assert_eq!(
            timer_state.remaining_seconds,
            expected_work_duration_secs.get()
        );
    }

    #[tokio::test]
    async fn automation_policy_sync_returns_persistence_error_without_rolling_back_policy() {
        let automation_state = build_test_state_with_injected_persistence_error().await;
        let mut automation_runtime_effect_shell = RecordingCommandRuntimeEffectShell::default();
        let expected_weekly_config = WeeklyAutoRunConfig::default();

        let automation_result = sync_automation_policies_with_runtime_effects(
            &automation_state,
            false,
            expected_weekly_config.clone(),
            PostBreakTransitionPolicy::StartNextWorkAutomatically,
            &mut automation_runtime_effect_shell,
        )
        .await;

        assert_persistence_save_error(&automation_result);
        assert_single_published_command_timer_state(
            &automation_runtime_effect_shell,
            TimerStatus::Idle,
            SessionType::Work,
        );

        let timer_state = automation_state
            .timer_manager
            .lock()
            .await
            .get_state()
            .clone();
        assert!(!timer_state.auto_run_feature_enabled);
        assert_eq!(timer_state.weekly_auto_run_config, expected_weekly_config);
        assert_eq!(
            timer_state.settings.post_break_transition_policy,
            PostBreakTransitionPolicy::StartNextWorkAutomatically
        );
    }

    #[tokio::test]
    async fn test_timer_command_persistence_failure_returns_public_command_error() {
        let state = build_test_state_with_injected_persistence_error().await;
        prepare_post_break_decision_pending_state(&state).await;
        let mut runtime_effect_shell = RecordingCommandRuntimeEffectShell::default();

        let result = resolve_post_break_decision_with_runtime_effects(
            &state,
            PostBreakDecisionAction::Continue,
            &mut runtime_effect_shell,
        )
        .await;

        assert_command_result_public_error(
            result,
            "persistence_error",
            &[
                "failed to serialize persisted timer state",
                "expected value",
            ],
        );
    }

    #[tokio::test]
    async fn test_timer_command_persistence_task_join_returns_public_command_error() {
        let state = build_test_state_with_panicking_persistence_hook().await;
        prepare_post_break_decision_pending_state(&state).await;
        let mut runtime_effect_shell = RecordingCommandRuntimeEffectShell::default();

        let result = resolve_post_break_decision_with_runtime_effects(
            &state,
            PostBreakDecisionAction::Continue,
            &mut runtime_effect_shell,
        )
        .await;

        assert_command_result_public_error(
            result,
            "persistence_error",
            &[
                "simulated persistence worker panic",
                "C:\\Users\\nak",
                "secret",
            ],
        );
    }

    #[tokio::test]
    async fn test_start_work_returns_persistence_error_when_runtime_save_fails() {
        let temp_dir = create_temp_dir("start-work-save-fails");
        let invalid_app_data_path = temp_dir.join("app-data-file");
        std::fs::write(&invalid_app_data_path, "not a directory")
            .expect("invalid app data file should be created");

        let state = build_test_state_with_persistence_dir(invalid_app_data_path).await;
        let settings = TimerSettings::new(
            WorkDurationSecs::try_new(1500).expect("work duration should be valid"),
            ShortBreakDurationSecs::try_new(300).expect("short break duration should be valid"),
            LongBreakDurationSecs::try_new(900).expect("long break duration should be valid"),
            LongBreakIntervalCount::try_new(4).expect("long break interval should be valid"),
            PostBreakTransitionPolicy::RequireManualContinue,
        );
        let mut runtime_effect_shell = RecordingRuntimeEffectShell::default();

        let result = start_work_for_tests(
            &state,
            settings,
            WorkStartOrigin::ManualOutsideScheduleWindow,
            &mut runtime_effect_shell,
        )
        .await;

        assert!(matches!(result, Err(PomoliceError::PersistenceSave(_))));

        cleanup_temp_dir(&temp_dir);
    }

    #[tokio::test]
    async fn test_start_work_persistence_failure_does_not_roll_back_started_session() {
        let temp_dir = create_temp_dir("start-work-save-fails-keeps-started-session");
        let invalid_app_data_path = temp_dir.join("app-data-file");
        std::fs::write(&invalid_app_data_path, "not a directory")
            .expect("invalid app data file should be created");

        let state = build_test_state_with_persistence_dir(invalid_app_data_path).await;
        let settings = TimerSettings::new(
            WorkDurationSecs::try_new(1500).expect("work duration should be valid"),
            ShortBreakDurationSecs::try_new(300).expect("short break duration should be valid"),
            LongBreakDurationSecs::try_new(900).expect("long break duration should be valid"),
            LongBreakIntervalCount::try_new(4).expect("long break interval should be valid"),
            PostBreakTransitionPolicy::RequireManualContinue,
        );
        let mut runtime_effect_shell = RecordingRuntimeEffectShell::default();

        let result = start_work_for_tests(
            &state,
            settings,
            WorkStartOrigin::ManualOutsideScheduleWindow,
            &mut runtime_effect_shell,
        )
        .await;

        assert_persistence_save_error(&result);

        let full_state = {
            let timer_manager = state.timer_manager.lock().await;
            timer_manager.get_full_state()
        };

        assert_eq!(full_state.status, TimerStatus::Working);
        assert_eq!(full_state.session_type, SessionType::Work);
        assert_eq!(full_state.remaining_seconds, 1500);

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
            SchedulerTickTestRuntime::from_state(&state, local_datetime(2025, 1, 7, 8, 59, 50));
        scheduler_runtime
            .run_tick(&state, local_datetime(2025, 1, 7, 9, 0, 10))
            .await;

        assert_work_session_started_from_boundary(&state).await;

        cleanup_temp_dir(&temp_dir);
    }

    #[tokio::test]
    async fn test_emergency_stop_exit_uses_break_mode_transition_coordinator() {
        let state = Arc::new(build_test_state_without_persistence_path().await);
        let events = Arc::new(StdMutex::new(Vec::new()));
        let release_first = Arc::new(StdAtomicBool::new(false));

        let first_transition = {
            let state = Arc::clone(&state);
            let events = Arc::clone(&events);
            let release_first = Arc::clone(&release_first);
            tokio::spawn(async move {
                run_emergency_stop_exit_uses_break_mode_transition_coordinator_for_tests(
                    state,
                    events,
                    "first-start",
                    "first-end",
                    release_first,
                )
                .await;
            })
        };

        wait_for_transition_event_count(&events, 1, "the first emergency-stop exit transition")
            .await;

        let second_transition = {
            let state = Arc::clone(&state);
            let events = Arc::clone(&events);
            tokio::spawn(async move {
                run_emergency_stop_exit_uses_break_mode_transition_coordinator_for_tests(
                    state,
                    events,
                    "second-start",
                    "second-end",
                    Arc::new(StdAtomicBool::new(true)),
                )
                .await;
            })
        };

        tokio::time::sleep(StdDuration::from_millis(50)).await;
        assert_eq!(
            events
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .as_slice(),
            ["first-start"]
        );

        release_first.store(true, Ordering::Release);

        first_transition
            .await
            .expect("first emergency-stop transition should complete");
        second_transition
            .await
            .expect("second emergency-stop transition should complete");

        assert_eq!(
            events
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .as_slice(),
            ["first-start", "first-end", "second-start", "second-end"]
        );
    }
}
