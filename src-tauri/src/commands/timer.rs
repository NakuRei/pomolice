use tauri::{Manager, State, WebviewWindow};

use crate::commands::input_rejection::ObservedCommandArg;
use crate::error::{PomoliceError, Result};
use crate::logging_event::{log_info_event, log_warn_event, BackendLogEvent};
use crate::schedule_boundary::ActiveWindow;
use crate::state::AppState;
use crate::types::{
    FullStatePayload, LongBreakDurationSecs, LongBreakIntervalCount, PostBreakDecisionAction,
    PostBreakTransitionPolicy, ShortBreakDurationSecs, TimerSettings, TimerStatus,
    WeeklyAutoRunConfig, WorkDurationSecs,
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

    let (mut full_state_payload, timer_status, active_window) = {
        let timer_manager = state.timer_manager.lock().await;
        (
            timer_manager.get_full_state(),
            timer_manager.status(),
            timer_manager
                .observe_auto_run_boundary_scope()
                .active_window_at(now_local),
        )
    };

    let active_window_summary = active_window.as_ref().map(ActiveWindow::to_summary_payload);

    let should_prompt_start_on_boot = state.record_initial_full_state_auto_start_prompt_decision(
        timer_status == TimerStatus::Idle && active_window.is_some(),
    );

    full_state_payload.boot_auto_start_prompt_window = if should_prompt_start_on_boot {
        active_window_summary.clone()
    } else {
        None
    };
    full_state_payload.active_window_summary = active_window_summary;
    Ok(full_state_payload)
}

#[cfg(test)]
pub(crate) async fn get_full_state_for_tests(state: &AppState) -> Result<FullStatePayload> {
    get_full_state_contract(state).await
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
    use crate::external_adapters::persistence::PersistenceManager;
    use crate::session_pipeline::{
        resolve_post_break_decision_with_runtime_effects,
        sync_automation_policies_with_runtime_effects, TimerCommandRuntimeEffectShell,
    };
    use crate::work_start::StartWorkRuntimeEffects;
    use chrono::{Local, LocalResult, Utc};
    use std::future::Future;
    use std::path::PathBuf;
    use std::pin::Pin;
    use std::sync::Arc;

    use crate::types::{DayAutoRunSlot, HhMm, SessionType, TimerTickPayload, WorkStartOrigin};
    use chrono::TimeZone;

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

    fn assert_no_start_runtime_effects(runtime_effect_shell: &RecordingRuntimeEffectShell) {
        assert!(runtime_effect_shell.emitted_tick_snapshots.is_empty());
        assert!(runtime_effect_shell.tray_tick_snapshots.is_empty());
        assert!(runtime_effect_shell.work_started_durations.is_empty());
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
        assert!(full_state.boot_auto_start_prompt_window.is_some());
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
        assert!(full_state.boot_auto_start_prompt_window.is_none());
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

        assert!(first_full_state.boot_auto_start_prompt_window.is_some());
        assert_daytime_monday_window(&first_full_state);
        assert!(second_full_state.boot_auto_start_prompt_window.is_none());
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
        assert!(full_state.boot_auto_start_prompt_window.is_none());
        assert_daytime_monday_window(&full_state);
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
}
