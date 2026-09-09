use std::future::Future;
use std::pin::Pin;

use chrono::{DateTime, Local};
use tauri::{AppHandle, Emitter};

use crate::app_handle_timer_effects;
use crate::error::{PomoliceError, Result};
use crate::logging_event::{
    log_persistence_write_failure, log_warn_event, BackendLogEvent,
    PERSISTENCE_WRITE_TRIGGER_SCHEDULER_AUTO_START,
};
use crate::schedule_boundary::is_now_inside_schedule_window;
use crate::state::AppState;
use crate::types::{
    TimerSettings, TimerStatus, TimerTickPayload, WeeklyAutoRunConfig, WorkStartOrigin,
};

pub(crate) enum StartWorkRequestOrigin {
    ManualCommand {
        work_start_origin: WorkStartOrigin,
    },
    BootPrompt,
    SchedulerAutoStart {
        timer_status_at_tick_start: TimerStatus,
        boundary_id: Option<String>,
    },
}

#[derive(Clone, Copy)]
pub(crate) enum StartWorkPersistencePolicy {
    ReturnFailure,
    LogAndContinue { trigger: &'static str },
}

pub(crate) struct StartWorkRequest {
    pub(crate) settings: TimerSettings,
    pub(crate) origin: StartWorkRequestOrigin,
    pub(crate) persistence_policy: StartWorkPersistencePolicy,
}

struct AcceptedStartWork {
    old_task: Option<tokio::task::JoinHandle<()>>,
    tick_payload: TimerTickPayload,
    work_duration_secs: u64,
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) enum StartWorkAttempt {
    Accepted,
    NoOp,
}

struct AppHandleStartWorkRuntimeEffects<'a> {
    app: &'a AppHandle,
}

impl StartWorkRuntimeEffects for AppHandleStartWorkRuntimeEffects<'_> {
    fn emit_timer_tick(&mut self, tick_payload: &TimerTickPayload) {
        app_handle_timer_effects::emit_timer_tick(self.app, tick_payload);
    }

    fn update_tray(&mut self, tick_payload: &TimerTickPayload) {
        app_handle_timer_effects::update_tray_icon(self.app, tick_payload);
    }

    fn emit_work_started(&mut self, work_duration_secs: u64) {
        let _ = self.app.emit(
            "work_started",
            serde_json::json!({ "duration": work_duration_secs }),
        );
    }

    fn spawn_timer_task<'a>(
        &'a mut self,
        _state: &'a AppState,
    ) -> Pin<Box<dyn Future<Output = ()> + Send + 'a>> {
        Box::pin(async move {
            crate::session_pipeline::spawn_timer_task(self.app).await;
        })
    }
}

pub(crate) trait StartWorkRuntimeEffects {
    fn emit_timer_tick(&mut self, tick_payload: &TimerTickPayload);
    fn update_tray(&mut self, tick_payload: &TimerTickPayload);
    fn emit_work_started(&mut self, work_duration_secs: u64);
    fn spawn_timer_task<'a>(
        &'a mut self,
        state: &'a AppState,
    ) -> Pin<Box<dyn Future<Output = ()> + Send + 'a>>;
    fn persist_current_state<'a>(
        &'a mut self,
        state: &'a AppState,
        policy: StartWorkPersistencePolicy,
    ) -> Pin<Box<dyn Future<Output = Result<()>> + Send + 'a>> {
        Box::pin(async move { persist_current_state_after_start(state, policy).await })
    }
}

pub(crate) fn determine_manual_start_origin(
    now_local: DateTime<Local>,
    auto_run_feature_enabled: bool,
    weekly_auto_run_config: &WeeklyAutoRunConfig,
) -> WorkStartOrigin {
    if auto_run_feature_enabled
        && is_now_inside_schedule_window(now_local, weekly_auto_run_config).is_some()
    {
        WorkStartOrigin::ManualInsideScheduleWindow
    } else {
        WorkStartOrigin::ManualOutsideScheduleWindow
    }
}

pub(crate) async fn start_manual_work_from_command(
    state: &AppState,
    app: &AppHandle,
    settings: TimerSettings,
) -> Result<()> {
    let mut effects = AppHandleStartWorkRuntimeEffects { app };
    start_manual_work_from_command_with_effects(state, settings, &mut effects).await?;
    Ok(())
}

pub(crate) async fn start_manual_work_from_command_with_effects(
    state: &AppState,
    settings: TimerSettings,
    effects: &mut impl StartWorkRuntimeEffects,
) -> Result<StartWorkAttempt> {
    let now_local = state.clock.now_local();
    let (auto_run_feature_enabled, weekly_auto_run_config) = {
        let timer = state.timer_manager.lock().await;
        let timer_state = timer.get_state();
        (
            timer_state.auto_run_feature_enabled,
            timer_state.weekly_auto_run_config.clone(),
        )
    };
    let manual_start_origin =
        determine_manual_start_origin(now_local, auto_run_feature_enabled, &weekly_auto_run_config);

    start_manual_work(state, settings, manual_start_origin, effects).await
}

pub(crate) async fn resolve_boot_auto_start_prompt_from_command(
    state: &AppState,
    app: &AppHandle,
    start_now: bool,
) -> Result<()> {
    let mut effects = AppHandleStartWorkRuntimeEffects { app };
    resolve_boot_auto_start_prompt_with_effects(state, start_now, &mut effects).await
}

pub(crate) async fn resolve_boot_auto_start_prompt_with_effects(
    state: &AppState,
    start_now: bool,
    effects: &mut impl StartWorkRuntimeEffects,
) -> Result<()> {
    let prompt_is_waiting = state.startup_auto_start_prompt_is_waiting();
    let can_resolve_prompt = {
        let timer_manager = state.timer_manager.lock().await;
        timer_manager.status() == TimerStatus::Idle && prompt_is_waiting
    };

    if !can_resolve_prompt {
        if prompt_is_waiting {
            state.allow_start_boundaries_after_startup_prompt();
        }
        return Ok(());
    }

    let timer_settings = {
        let timer_manager = state.timer_manager.lock().await;
        timer_manager.get_state().settings.clone()
    };

    if start_now {
        start_boot_prompt_work(state, timer_settings, effects).await?;
        return Ok(());
    }

    state.allow_start_boundaries_after_startup_prompt();
    Ok(())
}

pub(crate) async fn start_manual_work(
    state: &AppState,
    settings: TimerSettings,
    work_start_origin: WorkStartOrigin,
    effects: &mut impl StartWorkRuntimeEffects,
) -> Result<StartWorkAttempt> {
    run_start_work(
        state,
        StartWorkRequest {
            settings,
            origin: StartWorkRequestOrigin::ManualCommand { work_start_origin },
            persistence_policy: StartWorkPersistencePolicy::ReturnFailure,
        },
        effects,
    )
    .await
}

pub(crate) async fn start_boot_prompt_work(
    state: &AppState,
    settings: TimerSettings,
    effects: &mut impl StartWorkRuntimeEffects,
) -> Result<StartWorkAttempt> {
    run_start_work(
        state,
        StartWorkRequest {
            settings,
            origin: StartWorkRequestOrigin::BootPrompt,
            persistence_policy: StartWorkPersistencePolicy::ReturnFailure,
        },
        effects,
    )
    .await
}

#[cfg(test)]
pub(crate) async fn start_scheduler_work(
    state: &AppState,
    timer_status_at_tick_start: TimerStatus,
    settings: TimerSettings,
    effects: &mut impl StartWorkRuntimeEffects,
) -> Result<StartWorkAttempt> {
    start_scheduler_work_with_boundary_context(
        state,
        timer_status_at_tick_start,
        settings,
        None,
        effects,
    )
    .await
}

pub(crate) async fn start_scheduler_work_with_boundary_context(
    state: &AppState,
    timer_status_at_tick_start: TimerStatus,
    settings: TimerSettings,
    boundary_id: Option<&str>,
    effects: &mut impl StartWorkRuntimeEffects,
) -> Result<StartWorkAttempt> {
    run_start_work(
        state,
        StartWorkRequest {
            settings,
            origin: StartWorkRequestOrigin::SchedulerAutoStart {
                timer_status_at_tick_start,
                boundary_id: boundary_id.map(str::to_string),
            },
            persistence_policy: StartWorkPersistencePolicy::LogAndContinue {
                trigger: PERSISTENCE_WRITE_TRIGGER_SCHEDULER_AUTO_START,
            },
        },
        effects,
    )
    .await
}

async fn run_start_work(
    state: &AppState,
    request: StartWorkRequest,
    effects: &mut impl StartWorkRuntimeEffects,
) -> Result<StartWorkAttempt> {
    let Some(accepted_start) = prepare_accepted_start(state, &request).await? else {
        if matches!(request.origin, StartWorkRequestOrigin::BootPrompt) {
            state.allow_start_boundaries_after_startup_prompt();
        }
        return Ok(StartWorkAttempt::NoOp);
    };

    effects.emit_timer_tick(&accepted_start.tick_payload);
    effects.update_tray(&accepted_start.tick_payload);
    effects.emit_work_started(accepted_start.work_duration_secs);

    if let Some(handle) = accepted_start.old_task {
        let _ = handle.await;
    }

    effects.spawn_timer_task(state).await;

    if matches!(request.origin, StartWorkRequestOrigin::BootPrompt) {
        state.allow_start_boundaries_after_startup_prompt();
    }

    effects
        .persist_current_state(state, request.persistence_policy)
        .await?;

    Ok(StartWorkAttempt::Accepted)
}

async fn prepare_accepted_start(
    state: &AppState,
    request: &StartWorkRequest,
) -> Result<Option<AcceptedStartWork>> {
    let accepted_start = {
        let mut timer = state.timer_manager.lock().await;
        match &request.origin {
            StartWorkRequestOrigin::ManualCommand { work_start_origin } => {
                let (tick_payload, work_duration_secs) =
                    timer.start_work(request.settings.clone(), *work_start_origin)?;
                Some(AcceptedStartWork {
                    old_task: timer.cancel_and_take_task(),
                    tick_payload,
                    work_duration_secs,
                })
            }
            StartWorkRequestOrigin::BootPrompt => {
                match timer.start_work(
                    request.settings.clone(),
                    WorkStartOrigin::ManualInsideScheduleWindow,
                ) {
                    Ok((tick_payload, work_duration_secs)) => Some(AcceptedStartWork {
                        old_task: timer.cancel_and_take_task(),
                        tick_payload,
                        work_duration_secs,
                    }),
                    Err(PomoliceError::InvalidStateTransition { .. }) => None,
                    Err(error) => return Err(error),
                }
            }
            StartWorkRequestOrigin::SchedulerAutoStart {
                timer_status_at_tick_start,
                boundary_id,
            } => {
                if *timer_status_at_tick_start != TimerStatus::Idle
                    || timer.status() != TimerStatus::Idle
                {
                    None
                } else if let Ok((tick_payload, work_duration_secs)) = timer.start_work(
                    request.settings.clone(),
                    WorkStartOrigin::ScheduleStartBoundary,
                ) {
                    Some(AcceptedStartWork {
                        old_task: timer.cancel_and_take_task(),
                        tick_payload,
                        work_duration_secs,
                    })
                } else {
                    log_scheduler_start_rejection(boundary_id.as_deref());
                    None
                }
            }
        }
    };

    Ok(accepted_start)
}

async fn persist_current_state_after_start(
    state: &AppState,
    policy: StartWorkPersistencePolicy,
) -> Result<()> {
    match state.persist_current_state_async().await {
        Ok(()) => Ok(()),
        Err(error) => match policy {
            StartWorkPersistencePolicy::ReturnFailure => Err(error),
            StartWorkPersistencePolicy::LogAndContinue { trigger } => {
                log_persistence_write_failure(trigger, &error);
                Ok(())
            }
        },
    }
}

fn log_scheduler_start_rejection(boundary_id: Option<&str>) {
    let fields = boundary_id
        .map(|id| vec![("boundary_id", id.to_string())])
        .unwrap_or_default();
    log_warn_event(
        BackendLogEvent::SchedulerSkippedAutoStartDueToRejection,
        &fields,
    );
}

#[cfg(test)]
mod timer_command_behavior_tests {
    use super::*;
    use crate::external_adapters::clock::{testing::FixedClock, Clock};
    use crate::external_adapters::persistence::{PersistenceLoadOutcome, PersistenceManager};
    use crate::types::{
        LongBreakDurationSecs, LongBreakIntervalCount, PostBreakTransitionPolicy, SessionType,
        ShortBreakDurationSecs, TimerState, WorkDurationSecs, WorkStartOrigin,
    };
    use chrono::{Local, TimeZone, Utc};
    use std::future::Future;
    use std::path::{Path, PathBuf};
    use std::pin::Pin;
    use std::sync::Arc;
    use std::time::{Duration as StdDuration, SystemTime, UNIX_EPOCH};

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

    async fn start_work_for_tests(
        state: &AppState,
        settings: TimerSettings,
        manual_start_origin: WorkStartOrigin,
        runtime_effect_shell: &mut impl StartWorkRuntimeEffects,
    ) -> Result<()> {
        start_manual_work(state, settings, manual_start_origin, runtime_effect_shell).await?;
        Ok(())
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
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::PomoliceError;
    use crate::external_adapters::clock::testing::FixedClock;
    use crate::external_adapters::clock::Clock;
    use crate::external_adapters::persistence::PersistenceManager;
    use crate::types::{SessionType, WorkDurationSecs, WorkStartOrigin};
    use chrono::{Local, TimeZone, Utc};
    use std::path::PathBuf;
    use std::sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    };
    use std::time::Duration;

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

    #[derive(Default)]
    struct RecordingStartWorkEffects {
        timer_ticks: Vec<PublishedTimerState>,
        tray_updates: Vec<PublishedTimerState>,
        work_started_durations: Vec<u64>,
        timer_task_spawn_count: usize,
    }

    impl StartWorkRuntimeEffects for RecordingStartWorkEffects {
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
        ) -> Pin<Box<dyn Future<Output = ()> + Send + 'a>> {
            self.timer_task_spawn_count += 1;
            Box::pin(async {})
        }
    }

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

    async fn build_state_without_persistence_path() -> AppState {
        let clock = fixed_clock();
        let persistence = PersistenceManager::new(as_clock_trait(&clock), None);
        let (state, _) =
            AppState::build_startup_state_for_tests(as_clock_trait(&clock), persistence).await;
        state
    }

    async fn build_state_with_persistence_failure() -> AppState {
        let clock = fixed_clock();
        let persistence = PersistenceManager::new(as_clock_trait(&clock), Some(PathBuf::from(".")))
            .with_injected_save_error_hook(Arc::new(|| {
                Some(
                    crate::external_adapters::persistence::SaveError::Serialize {
                        source: serde_json::from_str::<serde_json::Value>("{")
                            .expect_err("invalid JSON should fail to parse"),
                    },
                )
            }));
        let (state, _) =
            AppState::build_startup_state_for_tests(as_clock_trait(&clock), persistence).await;
        state
    }

    fn settings_with_work_duration(work_duration_secs: u64) -> TimerSettings {
        TimerSettings {
            work_duration_secs: WorkDurationSecs::try_new(work_duration_secs)
                .expect("work duration should be valid"),
            ..TimerSettings::default()
        }
    }

    fn default_work_duration_secs() -> u64 {
        TimerSettings::default().work_duration_secs.get()
    }

    fn assert_accepted_start_effects(
        effects: &RecordingStartWorkEffects,
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

    fn assert_no_start_effects(effects: &RecordingStartWorkEffects) {
        assert!(effects.timer_ticks.is_empty());
        assert!(effects.tray_updates.is_empty());
        assert!(effects.work_started_durations.is_empty());
        assert_eq!(effects.timer_task_spawn_count, 0);
    }

    fn assert_persistence_failure(result: &Result<StartWorkAttempt>) {
        assert!(matches!(result, Err(PomoliceError::PersistenceSave(_))));
    }

    async fn force_timer_to_break(state: &AppState) {
        let mut timer = state.timer_manager.lock().await;
        timer
            .start_work(
                TimerSettings::default(),
                WorkStartOrigin::ManualOutsideScheduleWindow,
            )
            .expect("work session should start");
        timer.transition_to_break();
        assert_eq!(timer.status(), TimerStatus::OnBreak);
    }

    async fn force_timer_to_working(state: &AppState) {
        let mut timer = state.timer_manager.lock().await;
        timer
            .start_work(
                TimerSettings::default(),
                WorkStartOrigin::ManualOutsideScheduleWindow,
            )
            .expect("work session should start");
        assert_eq!(timer.status(), TimerStatus::Working);
    }

    #[tokio::test]
    async fn manual_command_start_publishes_started_work_with_the_requested_duration() {
        let state = build_state_without_persistence_path().await;
        let mut effects = RecordingStartWorkEffects::default();

        let result = start_manual_work(
            &state,
            settings_with_work_duration(1800),
            WorkStartOrigin::ManualOutsideScheduleWindow,
            &mut effects,
        )
        .await;

        assert_eq!(
            result.expect("manual start should succeed"),
            StartWorkAttempt::Accepted
        );
        assert_accepted_start_effects(&effects, 1800);
    }

    #[tokio::test(start_paused = true)]
    async fn failed_superseded_timer_task_join_does_not_block_replacement_start() {
        let state = build_state_without_persistence_path().await;
        let failed_task_ran = Arc::new(AtomicBool::new(false));
        let failed_task_ran_for_task = Arc::clone(&failed_task_ran);
        {
            let mut timer = state.timer_manager.lock().await;
            timer.spawn_timer_task(move || {
                let failed_task_ran = Arc::clone(&failed_task_ran_for_task);
                async move {
                    failed_task_ran.store(true, Ordering::Release);
                    panic!("superseded timer task failed");
                }
            });
        }
        tokio::task::yield_now().await;
        tokio::time::advance(Duration::from_secs(1)).await;
        tokio::task::yield_now().await;
        assert!(
            failed_task_ran.load(Ordering::Acquire),
            "superseded timer task should fail before replacement starts"
        );

        let mut effects = RecordingStartWorkEffects::default();

        let result = start_manual_work(
            &state,
            TimerSettings::default(),
            WorkStartOrigin::ManualOutsideScheduleWindow,
            &mut effects,
        )
        .await;

        assert_eq!(
            result.expect("failed old task join should stay nonfatal"),
            StartWorkAttempt::Accepted
        );
        assert_accepted_start_effects(&effects, default_work_duration_secs());
    }

    #[tokio::test]
    async fn boot_prompt_start_publishes_started_work_and_opens_boundaries() {
        let state = build_state_without_persistence_path().await;
        assert!(state.record_initial_full_state_auto_start_prompt_decision(true));
        let mut effects = RecordingStartWorkEffects::default();

        let result = start_boot_prompt_work(&state, TimerSettings::default(), &mut effects).await;

        assert_eq!(
            result.expect("boot prompt start should succeed"),
            StartWorkAttempt::Accepted
        );
        assert_accepted_start_effects(&effects, default_work_duration_secs());
        assert!(state.start_boundaries_allowed_after_startup_prompt());
    }

    #[tokio::test]
    async fn scheduler_auto_start_publishes_started_work() {
        let state = build_state_without_persistence_path().await;
        let mut effects = RecordingStartWorkEffects::default();

        let result = start_scheduler_work(
            &state,
            TimerStatus::Idle,
            TimerSettings::default(),
            &mut effects,
        )
        .await;

        assert_eq!(
            result.expect("scheduler start should succeed"),
            StartWorkAttempt::Accepted
        );
        assert_accepted_start_effects(&effects, default_work_duration_secs());
    }

    #[tokio::test]
    async fn rejected_manual_command_start_publishes_no_start_effects() {
        let state = build_state_without_persistence_path().await;
        force_timer_to_break(&state).await;
        let mut effects = RecordingStartWorkEffects::default();

        let result = start_manual_work(
            &state,
            TimerSettings::default(),
            WorkStartOrigin::ManualOutsideScheduleWindow,
            &mut effects,
        )
        .await;

        assert!(matches!(
            result,
            Err(PomoliceError::InvalidStateTransition { .. })
        ));
        assert_no_start_effects(&effects);
    }

    #[tokio::test]
    async fn rejected_boot_prompt_start_publishes_no_start_effects_and_opens_boundaries() {
        let state = build_state_without_persistence_path().await;
        assert!(state.record_initial_full_state_auto_start_prompt_decision(true));
        force_timer_to_working(&state).await;
        let mut effects = RecordingStartWorkEffects::default();

        let result = start_boot_prompt_work(&state, TimerSettings::default(), &mut effects).await;

        assert_eq!(
            result.expect("boot prompt rejection should be a no-op"),
            StartWorkAttempt::NoOp
        );
        assert_no_start_effects(&effects);
        assert!(state.start_boundaries_allowed_after_startup_prompt());
    }

    #[tokio::test]
    async fn rejected_scheduler_auto_start_publishes_no_start_effects() {
        let state = build_state_without_persistence_path().await;
        let mut effects = RecordingStartWorkEffects::default();

        let result = start_scheduler_work(
            &state,
            TimerStatus::Working,
            TimerSettings::default(),
            &mut effects,
        )
        .await;

        assert_eq!(
            result.expect("scheduler rejection should be a no-op"),
            StartWorkAttempt::NoOp
        );
        assert_no_start_effects(&effects);
        assert_eq!(state.timer_manager.lock().await.status(), TimerStatus::Idle);
    }

    #[tokio::test]
    async fn manual_command_persistence_failure_is_returned_after_publish() {
        let state = build_state_with_persistence_failure().await;
        let mut effects = RecordingStartWorkEffects::default();

        let result = start_manual_work(
            &state,
            TimerSettings::default(),
            WorkStartOrigin::ManualOutsideScheduleWindow,
            &mut effects,
        )
        .await;

        assert_persistence_failure(&result);
        assert_accepted_start_effects(&effects, default_work_duration_secs());
        assert_eq!(
            state.timer_manager.lock().await.status(),
            TimerStatus::Working
        );
    }

    #[tokio::test]
    async fn boot_prompt_persistence_failure_is_returned_after_publish() {
        let state = build_state_with_persistence_failure().await;
        assert!(state.record_initial_full_state_auto_start_prompt_decision(true));
        let mut effects = RecordingStartWorkEffects::default();

        let result = start_boot_prompt_work(&state, TimerSettings::default(), &mut effects).await;

        assert_persistence_failure(&result);
        assert_accepted_start_effects(&effects, default_work_duration_secs());
        assert!(state.start_boundaries_allowed_after_startup_prompt());
        assert_eq!(
            state.timer_manager.lock().await.status(),
            TimerStatus::Working
        );
    }

    #[tokio::test]
    async fn scheduler_persistence_failure_is_nonfatal_after_publish() {
        let state = build_state_with_persistence_failure().await;
        let mut effects = RecordingStartWorkEffects::default();

        let result = start_scheduler_work(
            &state,
            TimerStatus::Idle,
            TimerSettings::default(),
            &mut effects,
        )
        .await;

        assert_eq!(
            result.expect("scheduler persistence failure should be nonfatal"),
            StartWorkAttempt::Accepted
        );
        assert_accepted_start_effects(&effects, default_work_duration_secs());
        assert_eq!(
            state.timer_manager.lock().await.status(),
            TimerStatus::Working
        );
    }
}
