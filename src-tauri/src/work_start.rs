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
    let mut effects = AppHandleStartWorkRuntimeEffects { app };
    start_manual_work(state, settings, manual_start_origin, &mut effects).await?;
    Ok(())
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
mod tests {
    use super::*;
    use crate::error::PomoliceError;
    use crate::external_adapters::clock::testing::FixedClock;
    use crate::external_adapters::clock::Clock;
    use crate::external_adapters::persistence::PersistenceManager;
    use crate::types::{SessionType, TimerState, WorkDurationSecs, WorkStartOrigin};
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
        order: Vec<&'static str>,
        timer_ticks: Vec<PublishedTimerState>,
        tray_updates: Vec<PublishedTimerState>,
        work_started_durations: Vec<u64>,
        timer_task_spawn_count: usize,
    }

    impl StartWorkRuntimeEffects for RecordingStartWorkEffects {
        fn emit_timer_tick(&mut self, tick_payload: &TimerTickPayload) {
            self.order.push("timer_tick");
            self.timer_ticks
                .push(PublishedTimerState::from_payload(tick_payload));
        }

        fn update_tray(&mut self, tick_payload: &TimerTickPayload) {
            self.order.push("tray");
            self.tray_updates
                .push(PublishedTimerState::from_payload(tick_payload));
        }

        fn emit_work_started(&mut self, work_duration_secs: u64) {
            self.order.push("work_started");
            self.work_started_durations.push(work_duration_secs);
        }

        fn spawn_timer_task<'a>(
            &'a mut self,
            _state: &'a AppState,
        ) -> Pin<Box<dyn Future<Output = ()> + Send + 'a>> {
            self.order.push("spawn_timer_task");
            self.timer_task_spawn_count += 1;
            Box::pin(async {})
        }

        fn persist_current_state<'a>(
            &'a mut self,
            state: &'a AppState,
            policy: StartWorkPersistencePolicy,
        ) -> Pin<Box<dyn Future<Output = Result<()>> + Send + 'a>> {
            self.order.push("persist_current_state");
            Box::pin(async move { persist_current_state_after_start(state, policy).await })
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
            effects.order,
            [
                "timer_tick",
                "tray",
                "work_started",
                "spawn_timer_task",
                "persist_current_state",
            ]
        );
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
        assert!(effects.order.is_empty());
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
    async fn manual_command_start_publishes_started_work_before_persisting() {
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
    async fn boot_prompt_start_publishes_started_work_before_persisting_and_opens_boundaries() {
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
    async fn scheduler_auto_start_publishes_started_work_before_persisting() {
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

    #[tokio::test]
    async fn accepted_start_persists_the_started_work_snapshot() {
        let state = build_state_without_persistence_path().await;
        let mut effects = RecordingStartWorkEffects::default();

        start_manual_work(
            &state,
            TimerSettings::default(),
            WorkStartOrigin::ManualOutsideScheduleWindow,
            &mut effects,
        )
        .await
        .expect("manual start should succeed");

        let timer_state: TimerState = state.timer_manager.lock().await.get_state().clone();
        assert_eq!(timer_state.status, TimerStatus::Working);
        assert_eq!(timer_state.session_type, SessionType::Work);
        assert_eq!(timer_state.remaining_seconds, default_work_duration_secs());
        assert!(!timer_state.post_break_decision_pending);
    }
}
