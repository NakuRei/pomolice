use std::future::Future;
use std::pin::Pin;

use tauri::{AppHandle, Emitter, Manager};

use crate::app_handle_timer_effects;
use crate::error::{PomoliceError, Result};
use crate::logging_event::{
    log_error_event, log_info_event, log_persistence_write_failure, BackendLogEvent,
    PERSISTENCE_WRITE_TRIGGER_INPUT_PENALTY, PERSISTENCE_WRITE_TRIGGER_SESSION_COMPLETION,
};
use crate::state;
use crate::types;

type BreakModeTransitionResult =
    std::result::Result<(), (&'static str, crate::error::PomoliceError)>;
type TimerTickFuture = Pin<Box<dyn Future<Output = ()> + Send>>;

struct FinalizedSessionCompletionState {
    tick_payload: types::TimerTickPayload,
    post_break_transition_policy: types::PostBreakTransitionPolicy,
    post_break_decision_pending: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PipelineEmitEvent {
    SessionCompleted,
    AutoRunStopped,
    TimerTick,
}

const TIMER_TICK_ONLY_EMIT_ORDER: [PipelineEmitEvent; 1] = [PipelineEmitEvent::TimerTick];

const SESSION_COMPLETION_EMIT_ORDER_DEFAULT: [PipelineEmitEvent; 2] = [
    PipelineEmitEvent::SessionCompleted,
    PipelineEmitEvent::TimerTick,
];

const SESSION_COMPLETION_EMIT_ORDER_SCHEDULED_STOP: [PipelineEmitEvent; 3] = [
    PipelineEmitEvent::SessionCompleted,
    PipelineEmitEvent::AutoRunStopped,
    PipelineEmitEvent::TimerTick,
];

fn session_completion_emit_order(
    transition_reason: types::SessionTransitionReason,
) -> &'static [PipelineEmitEvent] {
    match transition_reason {
        types::SessionTransitionReason::ScheduledStopExecuted => {
            &SESSION_COMPLETION_EMIT_ORDER_SCHEDULED_STOP
        }
        _ => &SESSION_COMPLETION_EMIT_ORDER_DEFAULT,
    }
}

pub(crate) trait SessionPipelineRuntimeEffects {
    fn emit_event<T: serde::Serialize>(&mut self, event_name: &'static str, payload: &T);
    fn update_tray(&mut self, tick_payload: &types::TimerTickPayload);
    fn enter_break_mode<'a>(&'a mut self) -> Pin<Box<dyn Future<Output = ()> + Send + 'a>>;
    fn exit_break_mode<'a>(&'a mut self) -> Pin<Box<dyn Future<Output = ()> + Send + 'a>>;
    fn persist_timer_state<'a>(&'a mut self) -> Pin<Box<dyn Future<Output = ()> + Send + 'a>>;
    fn restart_timer_task<'a>(&'a mut self) -> Pin<Box<dyn Future<Output = ()> + Send + 'a>>;
}

struct AppHandleSessionPipelineRuntimeEffects<'a> {
    app: &'a AppHandle,
}

impl SessionPipelineRuntimeEffects for AppHandleSessionPipelineRuntimeEffects<'_> {
    fn emit_event<T: serde::Serialize>(&mut self, event_name: &'static str, payload: &T) {
        let _ = self.app.emit(event_name, payload);
    }

    fn update_tray(&mut self, tick_payload: &types::TimerTickPayload) {
        app_handle_timer_effects::update_tray_icon(self.app, tick_payload);
    }

    fn enter_break_mode<'a>(&'a mut self) -> Pin<Box<dyn Future<Output = ()> + Send + 'a>> {
        Box::pin(async move {
            enter_break_mode(self.app.clone()).await;
        })
    }

    fn exit_break_mode<'a>(&'a mut self) -> Pin<Box<dyn Future<Output = ()> + Send + 'a>> {
        Box::pin(async move {
            exit_break_mode(self.app.clone()).await;
        })
    }

    fn persist_timer_state<'a>(&'a mut self) -> Pin<Box<dyn Future<Output = ()> + Send + 'a>> {
        Box::pin(async move {
            persist_timer_state_with_trigger(
                self.app,
                PERSISTENCE_WRITE_TRIGGER_SESSION_COMPLETION,
            )
            .await;
        })
    }

    fn restart_timer_task<'a>(&'a mut self) -> Pin<Box<dyn Future<Output = ()> + Send + 'a>> {
        Box::pin(async move {
            restart_timer_task(self.app).await;
        })
    }
}

pub(crate) trait TimerCommandRuntimeEffectShell {
    fn emit_tick(&mut self, tick_payload: &types::TimerTickPayload);
    fn update_tray(&mut self, tick_payload: &types::TimerTickPayload);
    fn spawn_timer_task<'a>(
        &'a mut self,
        state_handle: &'a state::AppState,
    ) -> Pin<Box<dyn Future<Output = ()> + Send + 'a>>;
    fn request_fullscreen_break_exit(&mut self);
}

struct AppHandleTimerCommandRuntimeEffectShell<'a> {
    app: &'a AppHandle,
}

impl TimerCommandRuntimeEffectShell for AppHandleTimerCommandRuntimeEffectShell<'_> {
    fn emit_tick(&mut self, tick_payload: &types::TimerTickPayload) {
        app_handle_timer_effects::emit_timer_tick(self.app, tick_payload);
    }

    fn update_tray(&mut self, tick_payload: &types::TimerTickPayload) {
        app_handle_timer_effects::update_tray_icon(self.app, tick_payload);
    }

    fn spawn_timer_task<'a>(
        &'a mut self,
        _state_handle: &'a state::AppState,
    ) -> Pin<Box<dyn Future<Output = ()> + Send + 'a>> {
        Box::pin(async move {
            spawn_timer_task(self.app).await;
        })
    }

    fn request_fullscreen_break_exit(&mut self) {
        let app_handle = self.app.clone();
        tauri::async_runtime::spawn(async move {
            lib_handle_external_break_exit(app_handle).await;
        });
    }
}

pub(crate) async fn resolve_post_break_decision_from_command(
    app: &AppHandle,
    state_handle: &state::AppState,
    action: types::PostBreakDecisionAction,
) -> Result<()> {
    let mut effects = AppHandleTimerCommandRuntimeEffectShell { app };
    resolve_post_break_decision_with_runtime_effects(state_handle, action, &mut effects).await
}

pub(crate) async fn emergency_stop_from_command(
    app: &AppHandle,
    state_handle: &state::AppState,
) -> Result<()> {
    let mut effects = AppHandleTimerCommandRuntimeEffectShell { app };
    emergency_stop_with_runtime_effects(state_handle, &mut effects).await
}

pub(crate) async fn sync_timer_duration_settings_from_command(
    app: &AppHandle,
    state_handle: &state::AppState,
    work_duration_secs: types::WorkDurationSecs,
    short_break_duration_secs: types::ShortBreakDurationSecs,
    long_break_duration_secs: types::LongBreakDurationSecs,
    long_break_interval: types::LongBreakIntervalCount,
) -> Result<()> {
    let mut effects = AppHandleTimerCommandRuntimeEffectShell { app };
    sync_timer_duration_settings_with_runtime_effects(
        state_handle,
        work_duration_secs,
        short_break_duration_secs,
        long_break_duration_secs,
        long_break_interval,
        &mut effects,
    )
    .await
}

pub(crate) async fn sync_automation_policies_from_command(
    app: &AppHandle,
    state_handle: &state::AppState,
    auto_run_feature_enabled: bool,
    weekly_auto_run_config: types::WeeklyAutoRunConfig,
    post_break_transition_policy: types::PostBreakTransitionPolicy,
) -> Result<()> {
    let mut effects = AppHandleTimerCommandRuntimeEffectShell { app };
    sync_automation_policies_with_runtime_effects(
        state_handle,
        auto_run_feature_enabled,
        weekly_auto_run_config,
        post_break_transition_policy,
        &mut effects,
    )
    .await
}

pub(crate) fn validate_post_break_decision_command_state(
    timer_status: types::TimerStatus,
    post_break_decision_pending: bool,
) -> Result<()> {
    if timer_status == types::TimerStatus::Idle && post_break_decision_pending {
        return Ok(());
    }

    Err(PomoliceError::InvalidStateTransition {
        action: "resolve post-break decision".to_string(),
        current_state: timer_status,
    })
}

pub(crate) async fn resolve_post_break_decision_with_runtime_effects<S>(
    state_handle: &state::AppState,
    action: types::PostBreakDecisionAction,
    runtime_effect_shell: &mut S,
) -> Result<()>
where
    S: TimerCommandRuntimeEffectShell,
{
    let (old_task, tick_payload, should_spawn_timer_task) = {
        let mut timer = state_handle.timer_manager.lock().await;
        validate_post_break_decision_command_state(
            timer.status(),
            timer.get_state().post_break_decision_pending,
        )?;
        let tick_payload = timer.resolve_post_break_decision(action)?;
        let should_spawn_timer_task = action == types::PostBreakDecisionAction::Continue;
        let old_task = if should_spawn_timer_task {
            timer.cancel_and_take_task()
        } else {
            None
        };
        (old_task, tick_payload, should_spawn_timer_task)
    };

    emit_tick_and_update_tray(runtime_effect_shell, &tick_payload);

    if let Some(handle) = old_task {
        let _ = handle.await;
    }

    if should_spawn_timer_task {
        runtime_effect_shell.spawn_timer_task(state_handle).await;
    }

    persist_current_state_after_command_effects(state_handle).await
}

pub(crate) async fn emergency_stop_with_runtime_effects<S>(
    state_handle: &state::AppState,
    runtime_effect_shell: &mut S,
) -> Result<()>
where
    S: TimerCommandRuntimeEffectShell,
{
    let (was_fullscreen_break, tick_payload, old_task) = {
        let mut timer = state_handle.timer_manager.lock().await;
        let old = timer.cancel_and_take_task();
        let (was_fullscreen, payload) = timer.emergency_stop();
        (was_fullscreen, payload, old)
    };

    emit_tick_and_update_tray(runtime_effect_shell, &tick_payload);

    if let Some(handle) = old_task {
        let _ = handle.await;
    }

    if was_fullscreen_break {
        runtime_effect_shell.request_fullscreen_break_exit();
    }

    persist_current_state_after_command_effects(state_handle).await
}

pub(crate) async fn sync_timer_duration_settings_with_runtime_effects<S>(
    state_handle: &state::AppState,
    work_duration_secs: types::WorkDurationSecs,
    short_break_duration_secs: types::ShortBreakDurationSecs,
    long_break_duration_secs: types::LongBreakDurationSecs,
    long_break_interval: types::LongBreakIntervalCount,
    runtime_effect_shell: &mut S,
) -> Result<()>
where
    S: TimerCommandRuntimeEffectShell,
{
    let tick_payload = {
        let mut timer = state_handle.timer_manager.lock().await;
        timer.sync_timer_duration_settings(
            work_duration_secs,
            short_break_duration_secs,
            long_break_duration_secs,
            long_break_interval,
        )?
    };

    emit_tick_and_update_tray(runtime_effect_shell, &tick_payload);
    persist_current_state_after_command_effects(state_handle).await
}

pub(crate) async fn sync_automation_policies_with_runtime_effects<S>(
    state_handle: &state::AppState,
    auto_run_feature_enabled: bool,
    weekly_auto_run_config: types::WeeklyAutoRunConfig,
    post_break_transition_policy: types::PostBreakTransitionPolicy,
    runtime_effect_shell: &mut S,
) -> Result<()>
where
    S: TimerCommandRuntimeEffectShell,
{
    let tick_payload = {
        let mut timer = state_handle.timer_manager.lock().await;
        timer.sync_automation_policies(
            auto_run_feature_enabled,
            weekly_auto_run_config,
            post_break_transition_policy,
        )
    };

    emit_tick_and_update_tray(runtime_effect_shell, &tick_payload);
    persist_current_state_after_command_effects(state_handle).await
}

fn emit_tick_and_update_tray<S>(
    runtime_effect_shell: &mut S,
    tick_payload: &types::TimerTickPayload,
) where
    S: TimerCommandRuntimeEffectShell,
{
    runtime_effect_shell.emit_tick(tick_payload);
    runtime_effect_shell.update_tray(tick_payload);
}

async fn persist_current_state_after_command_effects(state_handle: &state::AppState) -> Result<()> {
    state_handle.persist_current_state_async().await?;
    Ok(())
}

trait BreakLockRuntimeEffects {
    fn capture_main_window_state_for_break<'a>(
        &'a mut self,
        state_handle: &'a state::AppState,
    ) -> Pin<Box<dyn Future<Output = crate::error::Result<()>> + Send + 'a>>;
    fn enter_fullscreen_break_lock<'a>(
        &'a mut self,
        state_handle: &'a state::AppState,
    ) -> Pin<Box<dyn Future<Output = crate::error::Result<()>> + Send + 'a>>;
    fn release_fullscreen_break_lock<'a>(
        &'a mut self,
        state_handle: &'a state::AppState,
    ) -> Pin<Box<dyn Future<Output = crate::error::Result<()>> + Send + 'a>>;
    fn emit_break_started(&mut self);
    fn emit_break_completed(&mut self);
}

struct AppHandleBreakLockRuntimeEffects<'a> {
    app: &'a AppHandle,
}

impl BreakLockRuntimeEffects for AppHandleBreakLockRuntimeEffects<'_> {
    fn capture_main_window_state_for_break<'a>(
        &'a mut self,
        state_handle: &'a state::AppState,
    ) -> Pin<Box<dyn Future<Output = crate::error::Result<()>> + Send + 'a>> {
        Box::pin(async move {
            state_handle
                .capture_main_window_state_for_break(self.app)
                .await
        })
    }

    fn enter_fullscreen_break_lock<'a>(
        &'a mut self,
        state_handle: &'a state::AppState,
    ) -> Pin<Box<dyn Future<Output = crate::error::Result<()>> + Send + 'a>> {
        Box::pin(async move {
            state_handle
                .enter_fullscreen_break_lock(self.app.clone())
                .await
        })
    }

    fn release_fullscreen_break_lock<'a>(
        &'a mut self,
        state_handle: &'a state::AppState,
    ) -> Pin<Box<dyn Future<Output = crate::error::Result<()>> + Send + 'a>> {
        Box::pin(async move { state_handle.release_fullscreen_break_lock(self.app).await })
    }

    fn emit_break_started(&mut self) {
        let _ = self.app.emit("break_started", serde_json::json!({}));
    }

    fn emit_break_completed(&mut self) {
        let _ = self.app.emit("break_completed", serde_json::json!({}));
    }
}

trait TimerTickRuntimeEffects {
    fn emit_timer_tick(&mut self, tick_payload: &types::TimerTickPayload);
    fn update_tray(&mut self, tick_payload: &types::TimerTickPayload);
    fn complete_session_after_tick<'a>(
        &'a mut self,
    ) -> Pin<Box<dyn Future<Output = ()> + Send + 'a>>;
}

struct AppHandleTimerTickRuntimeEffects<'a> {
    app: &'a AppHandle,
}

impl TimerTickRuntimeEffects for AppHandleTimerTickRuntimeEffects<'_> {
    fn emit_timer_tick(&mut self, tick_payload: &types::TimerTickPayload) {
        app_handle_timer_effects::emit_timer_tick(self.app, tick_payload);
    }

    fn update_tray(&mut self, tick_payload: &types::TimerTickPayload) {
        app_handle_timer_effects::update_tray_icon(self.app, tick_payload);
    }

    fn complete_session_after_tick<'a>(
        &'a mut self,
    ) -> Pin<Box<dyn Future<Output = ()> + Send + 'a>> {
        Box::pin(async move {
            lib_handle_session_complete(self.app.clone()).await;
        })
    }
}

fn lib_handle_timer_tick_future(app: AppHandle) -> TimerTickFuture {
    Box::pin(async move {
        let state_handle = app.state::<state::AppState>();
        let mut effects = AppHandleTimerTickRuntimeEffects { app: &app };

        handle_timer_tick_with_effects(&state_handle, &mut effects).await;
    })
}

async fn handle_timer_tick_with_effects(
    state_handle: &state::AppState,
    effects: &mut (impl TimerTickRuntimeEffects + Send),
) {
    let (should_complete_session, tick_payload) = {
        let mut timer = state_handle.timer_manager.lock().await;

        if timer.status() == types::TimerStatus::Idle {
            return;
        }

        let (completed, payload) = timer.tick();

        (completed, payload)
    };

    effects.emit_timer_tick(&tick_payload);
    effects.update_tray(&tick_payload);

    if should_complete_session {
        effects.complete_session_after_tick().await;
    }
}

trait InputPenaltyRuntimeEffects {
    fn persist_input_penalty<'a>(&'a mut self) -> Pin<Box<dyn Future<Output = ()> + Send + 'a>>;
    fn emit_timer_tick(&mut self, tick_payload: &types::TimerTickPayload);
    fn update_tray(&mut self, tick_payload: &types::TimerTickPayload);
}

struct AppHandleInputPenaltyRuntimeEffects<'a> {
    app: &'a AppHandle,
}

impl InputPenaltyRuntimeEffects for AppHandleInputPenaltyRuntimeEffects<'_> {
    fn persist_input_penalty<'a>(&'a mut self) -> Pin<Box<dyn Future<Output = ()> + Send + 'a>> {
        Box::pin(async move {
            persist_timer_state_with_trigger(self.app, PERSISTENCE_WRITE_TRIGGER_INPUT_PENALTY)
                .await;
        })
    }

    fn emit_timer_tick(&mut self, tick_payload: &types::TimerTickPayload) {
        app_handle_timer_effects::emit_timer_tick(self.app, tick_payload);
    }

    fn update_tray(&mut self, tick_payload: &types::TimerTickPayload) {
        app_handle_timer_effects::update_tray_icon(self.app, tick_payload);
    }
}

async fn apply_input_penalty_with_effects(
    state_handle: &state::AppState,
    effects: &mut (impl InputPenaltyRuntimeEffects + Send),
) {
    let tick_payload = {
        let mut timer = state_handle.timer_manager.lock().await;
        timer.apply_input_penalty()
    };

    if let Some(ref payload) = tick_payload {
        effects.persist_input_penalty().await;
        effects.emit_timer_tick(payload);
        effects.update_tray(payload);
    }
}

fn transition_requires_task_cancellation(
    transition_reason: types::SessionTransitionReason,
) -> bool {
    matches!(
        transition_reason,
        types::SessionTransitionReason::BreakWaitingDecision
            | types::SessionTransitionReason::ScheduledStopExecuted
    )
}

fn break_mode_requirements(transition_outcome: &types::SessionTransitionOutcome) -> (bool, bool) {
    let requires_enter_break_mode =
        transition_outcome.transition_reason == types::SessionTransitionReason::WorkToBreak;
    let requires_exit_break_mode = matches!(
        transition_outcome.completed_session_type,
        types::SessionType::ShortBreak | types::SessionType::LongBreak
    );

    (requires_enter_break_mode, requires_exit_break_mode)
}

async fn complete_session_transition(app: &AppHandle) -> types::SessionTransitionOutcome {
    let state_handle = app.state::<state::AppState>();
    let mut timer = state_handle.timer_manager.lock().await;
    let completed_payload = match timer.get_state().session_type {
        types::SessionType::Work => timer.complete_work_session_transition(),
        types::SessionType::ShortBreak | types::SessionType::LongBreak => {
            timer.complete_break_session_transition()
        }
    };
    let transition_outcome =
        types::SessionTransitionOutcome::from_completed_payload(&completed_payload);

    if transition_requires_task_cancellation(transition_outcome.transition_reason) {
        let _ = timer.cancel_and_take_task();
    }

    transition_outcome
}

async fn finalized_session_completion_state(
    state_handle: &state::AppState,
) -> FinalizedSessionCompletionState {
    let timer = state_handle.timer_manager.lock().await;
    let state = timer.get_state();

    FinalizedSessionCompletionState {
        tick_payload: state.to_tick_payload(),
        post_break_transition_policy: state.settings.post_break_transition_policy,
        post_break_decision_pending: state.post_break_decision_pending,
    }
}

fn log_session_completion_finalized(
    transition_outcome: &types::SessionTransitionOutcome,
    finalized_state: &FinalizedSessionCompletionState,
) {
    let transition_reason = transition_outcome.transition_reason;
    let post_break_transition_policy = finalized_state.post_break_transition_policy;
    let post_break_decision_pending = finalized_state.post_break_decision_pending;

    log_info_event(
        BackendLogEvent::TimerTransitionCompleted,
        &[
            (
                "transition_stage",
                "session_completion_pipeline_finalized".to_string(),
            ),
            ("transition_reason", format!("{transition_reason:?}")),
            ("policy", format!("{post_break_transition_policy:?}")),
            (
                "post_break_decision_pending",
                post_break_decision_pending.to_string(),
            ),
        ],
    );
}

fn emit_pipeline_events_with_effects(
    effects: &mut impl SessionPipelineRuntimeEffects,
    emit_order: &[PipelineEmitEvent],
    transition_outcome: Option<&types::SessionTransitionOutcome>,
    tick_payload: &types::TimerTickPayload,
) {
    for emit_event in emit_order {
        match emit_event {
            PipelineEmitEvent::SessionCompleted => {
                let Some(transition_outcome) = transition_outcome else {
                    debug_assert!(
                        false,
                        "session_completed emission requires a transition outcome"
                    );
                    continue;
                };
                let completed_payload =
                    types::SessionCompletedPayload::from_transition_outcome(transition_outcome);
                effects.emit_event("session_completed", &completed_payload);
            }
            PipelineEmitEvent::AutoRunStopped => {
                let payload = serde_json::json!({});
                effects.emit_event("auto_run_stopped", &payload);
            }
            PipelineEmitEvent::TimerTick => {
                effects.emit_event("timer_tick", tick_payload);
                effects.update_tray(tick_payload);
            }
        }
    }
}

fn emit_startup_resume_events_with_effects(
    effects: &mut impl SessionPipelineRuntimeEffects,
    tick_payload: &types::TimerTickPayload,
) {
    emit_pipeline_events_with_effects(effects, &TIMER_TICK_ONLY_EMIT_ORDER, None, tick_payload);
}

fn emit_session_completion_events_with_effects(
    effects: &mut impl SessionPipelineRuntimeEffects,
    transition_outcome: &types::SessionTransitionOutcome,
    tick_payload: &types::TimerTickPayload,
) {
    emit_pipeline_events_with_effects(
        effects,
        session_completion_emit_order(transition_outcome.transition_reason),
        Some(transition_outcome),
        tick_payload,
    );
}

async fn persist_timer_state_with_trigger(app: &AppHandle, trigger: &'static str) {
    let state_handle = app.state::<state::AppState>();
    if let Err(error) = state_handle.persist_current_state_async().await {
        log_persistence_write_failure(trigger, &error);
    }
}

fn log_break_mode_failure(
    event: BackendLogEvent,
    step: &'static str,
    error: &crate::error::PomoliceError,
) {
    log_error_event(event, &[("step", step.to_string())], Some(error));
}

async fn rollback_break_enter_failure(
    state_handle: &state::AppState,
    effects: &mut (impl BreakLockRuntimeEffects + Send),
) {
    state_handle
        .rollback_input_monitor_break_enter_failure()
        .await;

    if let Err(error) = effects.release_fullscreen_break_lock(state_handle).await {
        log_break_mode_failure(
            BackendLogEvent::SessionFullscreenBreakEnterFailed,
            "rollback_exit_fullscreen_all_monitors",
            &error,
        );
    }
}

async fn restart_timer_task(app: &AppHandle) {
    spawn_timer_task(app).await;
}

pub(crate) async fn spawn_timer_task(app: &AppHandle) {
    let state_handle = app.state::<state::AppState>();
    let mut timer = state_handle.timer_manager.lock().await;
    let app = app.clone();
    timer.spawn_timer_task(move || {
        let app = app.clone();
        lib_handle_timer_tick_future(app)
    });
}

async fn apply_session_completion_runtime_effects(
    state_handle: &state::AppState,
    transition_outcome: &types::SessionTransitionOutcome,
    restart_timer_task_after_effects: bool,
    effects: &mut (impl SessionPipelineRuntimeEffects + Send),
) {
    let (requires_enter_break_mode, requires_exit_break_mode) =
        break_mode_requirements(transition_outcome);

    if requires_enter_break_mode {
        effects.enter_break_mode().await;
    } else if requires_exit_break_mode {
        effects.exit_break_mode().await;
    }

    let finalized_state = finalized_session_completion_state(state_handle).await;

    log_session_completion_finalized(transition_outcome, &finalized_state);
    emit_session_completion_events_with_effects(
        effects,
        transition_outcome,
        &finalized_state.tick_payload,
    );

    effects.persist_timer_state().await;

    if restart_timer_task_after_effects {
        effects.restart_timer_task().await;
    }
}

pub(crate) async fn lib_handle_session_complete(app: AppHandle) {
    let transition_outcome = complete_session_transition(&app).await;
    let state_handle = app.state::<state::AppState>();
    let mut effects = AppHandleSessionPipelineRuntimeEffects { app: &app };
    apply_session_completion_runtime_effects(
        &state_handle,
        &transition_outcome,
        false,
        &mut effects,
    )
    .await;
}

pub(crate) async fn apply_startup_restore_with_effects(
    state_handle: &state::AppState,
    initialization_outcome: state::AppInitializationOutcome,
    effects: &mut (impl SessionPipelineRuntimeEffects + Send),
) {
    let state::AppInitializationOutcome {
        initial_tick_payload,
        restore_status: _,
        startup_action,
    } = initialization_outcome;

    match startup_action {
        state::StartupRestoreAction::None => {}
        state::StartupRestoreAction::RestartRestoredTimerTask => {
            let Some(tick_payload) = initial_tick_payload else {
                debug_assert!(
                    false,
                    "running startup restore must provide an initial tick payload"
                );
                return;
            };

            if tick_payload.status == types::TimerStatus::OnBreak {
                effects.enter_break_mode().await;
            }

            // Re-read the state after break-mode entry so the resume payload
            // reports the engaged fullscreen-break flag, like the session
            // completion pipeline does.
            let resume_tick_payload = {
                let timer = state_handle.timer_manager.lock().await;
                timer.get_state().to_tick_payload()
            };
            emit_startup_resume_events_with_effects(effects, &resume_tick_payload);
            effects.restart_timer_task().await;
        }
        state::StartupRestoreAction::FinalizeExpiredRestoredWork(transition_outcome) => {
            let restart_timer_task = transition_outcome.next_status != types::TimerStatus::Idle;
            apply_session_completion_runtime_effects(
                state_handle,
                &transition_outcome,
                restart_timer_task,
                effects,
            )
            .await;
        }
    }
}

pub(crate) async fn lib_handle_startup_restore(
    app: AppHandle,
    initialization_outcome: state::AppInitializationOutcome,
) {
    let state_handle = app.state::<state::AppState>();
    let mut effects = AppHandleSessionPipelineRuntimeEffects { app: &app };
    apply_startup_restore_with_effects(&state_handle, initialization_outcome, &mut effects).await;
}

async fn enter_break_mode(app: AppHandle) {
    log_info_event(BackendLogEvent::SessionFullscreenBreakEnterStarted, &[]);
    let state_handle = app.state::<state::AppState>();
    let mut effects = AppHandleBreakLockRuntimeEffects { app: &app };

    let enter_result = enter_break_mode_with_effects(&state_handle, &mut effects).await;

    if let Err((step, error)) = enter_result {
        log_break_mode_failure(
            BackendLogEvent::SessionFullscreenBreakEnterFailed,
            step,
            &error,
        );
    }
}

async fn enter_break_mode_with_effects(
    state_handle: &state::AppState,
    effects: &mut (impl BreakLockRuntimeEffects + Send),
) -> BreakModeTransitionResult {
    state_handle
        .run_break_mode_transition(async {
            state_handle.start_input_monitor_for_break().await;

            if let Err(error) = effects
                .capture_main_window_state_for_break(state_handle)
                .await
            {
                state_handle
                    .rollback_input_monitor_break_enter_failure()
                    .await;
                return Err(("capture_main_window_state_for_break", error));
            }

            if let Err(error) = effects.enter_fullscreen_break_lock(state_handle).await {
                rollback_break_enter_failure(state_handle, effects).await;
                return Err(("enter_fullscreen_all_monitors", error));
            }

            {
                log_info_event(BackendLogEvent::SessionFullscreenBreakEntered, &[]);
            }

            {
                let mut timer = state_handle.timer_manager.lock().await;
                timer.set_fullscreen_break(true);
            }

            effects.emit_break_started();
            Ok(())
        })
        .await
}

async fn exit_break_mode(app: AppHandle) {
    log_info_event(BackendLogEvent::SessionFullscreenBreakExitStarted, &[]);
    let state_handle = app.state::<state::AppState>();
    let mut effects = AppHandleBreakLockRuntimeEffects { app: &app };

    let exit_result = exit_break_mode_with_effects(&state_handle, &mut effects).await;

    if let Err((step, error)) = exit_result {
        log_break_mode_failure(
            BackendLogEvent::SessionFullscreenBreakExitFailed,
            step,
            &error,
        );
    }
}

async fn exit_break_mode_with_effects(
    state_handle: &state::AppState,
    effects: &mut (impl BreakLockRuntimeEffects + Send),
) -> BreakModeTransitionResult {
    state_handle
        .run_break_mode_transition(async {
            state_handle.stop_input_monitor_for_break().await;

            effects
                .release_fullscreen_break_lock(state_handle)
                .await
                .map_err(|error| ("exit_fullscreen_all_monitors", error))?;
            log_info_event(BackendLogEvent::SessionFullscreenBreakExited, &[]);

            {
                let mut timer = state_handle.timer_manager.lock().await;
                timer.set_fullscreen_break(false);
            }

            effects.emit_break_completed();
            Ok(())
        })
        .await
}

pub(crate) async fn lib_handle_input_detected(app: AppHandle) {
    let state_handle = app.state::<state::AppState>();
    let mut effects = AppHandleInputPenaltyRuntimeEffects { app: &app };
    apply_input_penalty_with_effects(&state_handle, &mut effects).await;
}

pub(crate) async fn lib_handle_escape_combo(app: AppHandle) {
    let state_handle = app.state::<state::AppState>();

    state_handle
        .run_break_mode_transition(async {
            state_handle.stop_input_monitor_for_break().await;

            if let Err(error) = state_handle.release_fullscreen_break_lock(&app).await {
                log_break_mode_failure(
                    BackendLogEvent::SessionFullscreenBreakExitFailed,
                    "escape_combo_exit_fullscreen_all_monitors",
                    &error,
                );
            }
        })
        .await;
}

pub(crate) async fn lib_handle_external_break_exit(app: AppHandle) {
    let state_handle = app.state::<state::AppState>();

    state_handle
        .run_break_mode_transition(async {
            state_handle.stop_input_monitor_for_break().await;

            if let Err(error) = state_handle.release_fullscreen_break_lock(&app).await {
                log_break_mode_failure(
                    BackendLogEvent::SessionFullscreenBreakExitFailed,
                    "external_break_exit_fullscreen_all_monitors",
                    &error,
                );
            }
        })
        .await;
}

#[cfg(test)]
mod timer_command_behavior_tests {
    use super::*;
    use crate::external_adapters::clock::{testing::FixedClock, Clock};
    use crate::external_adapters::persistence::{PersistenceLoadOutcome, PersistenceManager};
    use crate::state::AppState;
    use crate::types::{
        DayAutoRunSlot, HhMm, LongBreakDurationSecs, LongBreakIntervalCount,
        PostBreakDecisionAction, PostBreakTransitionPolicy, SessionType, ShortBreakDurationSecs,
        TimerSettings, TimerState, TimerStatus, TimerTickPayload, WeeklyAutoRunConfig,
        WorkDurationSecs, WorkStartOrigin,
    };
    use crate::work_start::{start_manual_work, StartWorkRuntimeEffects};
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

    async fn start_work_for_tests(
        state: &AppState,
        settings: TimerSettings,
        manual_start_origin: WorkStartOrigin,
        runtime_effect_shell: &mut impl StartWorkRuntimeEffects,
    ) -> Result<()> {
        start_manual_work(state, settings, manual_start_origin, runtime_effect_shell).await?;
        Ok(())
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
    async fn resolve_post_break_decision_accepts_idle_pending_state() {
        let state = build_test_state_without_persistence_path().await;
        prepare_post_break_decision_pending_state(&state).await;
        let mut effects = RecordingCommandRuntimeEffectShell::default();
        resolve_post_break_decision_with_runtime_effects(
            &state,
            PostBreakDecisionAction::Continue,
            &mut effects,
        )
        .await
        .expect("idle pending decision should resolve");
        assert_single_published_command_timer_state(
            &effects,
            TimerStatus::Working,
            SessionType::Work,
        );
    }

    #[tokio::test]
    async fn resolve_post_break_decision_rejects_idle_without_pending_preserving_state() {
        let state = build_test_state_without_persistence_path().await;
        let before = state.timer_manager.lock().await.get_state().clone();
        let mut effects = RecordingCommandRuntimeEffectShell::default();
        let result = resolve_post_break_decision_with_runtime_effects(
            &state,
            PostBreakDecisionAction::Continue,
            &mut effects,
        )
        .await;
        assert!(matches!(
            result,
            Err(PomoliceError::InvalidStateTransition { .. })
        ));
        assert_eq!(
            format!("{:?}", state.timer_manager.lock().await.get_state()),
            format!("{before:?}")
        );
        assert!(effects.emitted_tick_snapshots.is_empty());
        assert!(effects.tray_tick_snapshots.is_empty());
        assert_eq!(effects.fullscreen_break_exit_requests, 0);
    }

    #[tokio::test]
    async fn resolve_post_break_decision_rejects_non_idle_pending_preserving_state() {
        let state = build_test_state_without_persistence_path().await;
        {
            let mut timer = state.timer_manager.lock().await;
            let _ = timer.apply_persisted_state(TimerState {
                status: TimerStatus::Working,
                post_break_decision_pending: true,
                ..TimerState::default()
            });
        }
        let before = state.timer_manager.lock().await.get_state().clone();
        let mut effects = RecordingCommandRuntimeEffectShell::default();
        let result = resolve_post_break_decision_with_runtime_effects(
            &state,
            PostBreakDecisionAction::Continue,
            &mut effects,
        )
        .await;
        assert!(matches!(
            result,
            Err(PomoliceError::InvalidStateTransition { .. })
        ));
        assert_eq!(
            format!("{:?}", state.timer_manager.lock().await.get_state()),
            format!("{before:?}")
        );
        assert!(effects.emitted_tick_snapshots.is_empty());
        assert!(effects.tray_tick_snapshots.is_empty());
        assert_eq!(effects.fullscreen_break_exit_requests, 0);
    }

    #[tokio::test]
    async fn emergency_stop_from_fullscreen_break_publishes_idle_and_requests_exit() {
        let state = build_test_state_without_persistence_path().await;
        prepare_fullscreen_break_state(&state).await;
        let mut effects = RecordingCommandRuntimeEffectShell::default();
        emergency_stop_with_runtime_effects(&state, &mut effects)
            .await
            .expect("emergency stop should succeed");
        assert_single_published_command_timer_state(&effects, TimerStatus::Idle, SessionType::Work);
        assert_eq!(effects.fullscreen_break_exit_requests, 1);
        let timer = state.timer_manager.lock().await;
        assert_eq!(timer.status(), TimerStatus::Idle);
        assert!(!timer.get_state().is_fullscreen_break);
    }
}

#[cfg(test)]
mod emit_order_tests {
    use super::{
        apply_session_completion_runtime_effects, emit_session_completion_events_with_effects,
        enter_break_mode_with_effects, exit_break_mode_with_effects,
        handle_timer_tick_with_effects, BreakLockRuntimeEffects, SessionPipelineRuntimeEffects,
        TimerTickRuntimeEffects,
    };
    use crate::error::PomoliceError;
    use crate::external_adapters::clock::testing::FixedClock;
    use crate::external_adapters::clock::Clock;
    use crate::external_adapters::persistence::PersistenceManager;
    use crate::external_adapters::window_lock::{MainWindowRestorePlan, WindowState};
    use crate::state::{self, AppState};
    use crate::types::{
        PostBreakTransitionPolicy, SessionTransitionOutcome, SessionType, TimerSettings,
        TimerState, TimerStatus, TimerTickPayload,
    };
    use chrono::{Local, TimeZone, Utc};
    use std::future::Future;
    use std::pin::Pin;
    use std::sync::atomic::AtomicBool as StdAtomicBool;
    use std::sync::{Arc, Mutex as StdMutex};
    use std::time::Duration as StdDuration;
    use tauri::{PhysicalPosition, PhysicalSize};

    #[derive(Default)]
    struct RecordingSessionPipelineRuntimeEffects {
        emitted_event_names: Vec<&'static str>,
        emitted_events: Vec<RecordedPipelineEvent>,
        tray_update_count: usize,
    }

    #[derive(Debug, Default, PartialEq, Eq)]
    struct TimerTickEffectArtifact {
        timer_tick_emissions: usize,
        tray_updates: usize,
        session_completions: usize,
    }

    #[derive(Default)]
    struct RecordingTimerTickRuntimeEffects {
        artifact: TimerTickEffectArtifact,
    }

    struct RecordedPipelineEvent {
        event_name: &'static str,
        event_payload: serde_json::Value,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    enum RecordedBreakLockState {
        Unlocked,
        PartiallyLocked,
        Locked,
    }

    struct RecordingBreakLockRuntimeEffects {
        lock_state: RecordedBreakLockState,
        captured_window_state: Option<WindowState>,
        capture_error: Option<PomoliceError>,
        enter_error: Option<PomoliceError>,
        release_error: Option<PomoliceError>,
        released_restore_plans: Vec<MainWindowRestorePlan>,
        emitted_event_names: Vec<&'static str>,
    }

    impl Default for RecordingBreakLockRuntimeEffects {
        fn default() -> Self {
            Self {
                lock_state: RecordedBreakLockState::Unlocked,
                captured_window_state: None,
                capture_error: None,
                enter_error: None,
                release_error: None,
                released_restore_plans: Vec::new(),
                emitted_event_names: Vec::new(),
            }
        }
    }

    impl RecordingBreakLockRuntimeEffects {
        fn with_enter_error(message: &str) -> Self {
            Self {
                enter_error: Some(PomoliceError::WindowControl(message.to_string())),
                ..Self::default()
            }
        }

        fn with_release_error(mut self, message: &str) -> Self {
            self.release_error = Some(PomoliceError::WindowControl(message.to_string()));
            self
        }

        fn with_captured_window_state(mut self, window_state: WindowState) -> Self {
            self.captured_window_state = Some(window_state);
            self
        }
    }

    impl BreakLockRuntimeEffects for RecordingBreakLockRuntimeEffects {
        fn capture_main_window_state_for_break<'a>(
            &'a mut self,
            state_handle: &'a state::AppState,
        ) -> Pin<Box<dyn Future<Output = crate::error::Result<()>> + Send + 'a>> {
            Box::pin(async move {
                if let Some(error) = self.capture_error.take() {
                    return Err(error);
                }

                let mut window_manager = state_handle.window_manager.lock().await;
                window_manager.set_saved_state(self.captured_window_state.clone());
                Ok(())
            })
        }

        fn enter_fullscreen_break_lock<'a>(
            &'a mut self,
            _state_handle: &'a state::AppState,
        ) -> Pin<Box<dyn Future<Output = crate::error::Result<()>> + Send + 'a>> {
            Box::pin(async move {
                if let Some(error) = self.enter_error.take() {
                    self.lock_state = RecordedBreakLockState::PartiallyLocked;
                    return Err(error);
                }

                self.lock_state = RecordedBreakLockState::Locked;
                Ok(())
            })
        }

        fn release_fullscreen_break_lock<'a>(
            &'a mut self,
            state_handle: &'a state::AppState,
        ) -> Pin<Box<dyn Future<Output = crate::error::Result<()>> + Send + 'a>> {
            Box::pin(async move {
                if let Some(error) = self.release_error.take() {
                    return Err(error);
                }

                let window_manager = state_handle.window_manager.lock().await;
                self.released_restore_plans
                    .push(window_manager.main_window_restore_plan());
                self.lock_state = RecordedBreakLockState::Unlocked;
                Ok(())
            })
        }

        fn emit_break_started(&mut self) {
            self.emitted_event_names.push("break_started");
        }

        fn emit_break_completed(&mut self) {
            self.emitted_event_names.push("break_completed");
        }
    }

    impl SessionPipelineRuntimeEffects for RecordingSessionPipelineRuntimeEffects {
        fn emit_event<T: serde::Serialize>(&mut self, event_name: &'static str, payload: &T) {
            self.emitted_event_names.push(event_name);
            self.emitted_events.push(RecordedPipelineEvent {
                event_name,
                event_payload: serde_json::to_value(payload)
                    .expect("pipeline event payload should serialize"),
            });
        }

        fn update_tray(&mut self, _tick_payload: &TimerTickPayload) {
            self.tray_update_count += 1;
        }

        fn enter_break_mode<'a>(&'a mut self) -> Pin<Box<dyn Future<Output = ()> + Send + 'a>> {
            Box::pin(async {})
        }

        fn exit_break_mode<'a>(&'a mut self) -> Pin<Box<dyn Future<Output = ()> + Send + 'a>> {
            Box::pin(async {})
        }

        fn persist_timer_state<'a>(&'a mut self) -> Pin<Box<dyn Future<Output = ()> + Send + 'a>> {
            Box::pin(async {})
        }

        fn restart_timer_task<'a>(&'a mut self) -> Pin<Box<dyn Future<Output = ()> + Send + 'a>> {
            Box::pin(async {})
        }
    }

    impl TimerTickRuntimeEffects for RecordingTimerTickRuntimeEffects {
        fn emit_timer_tick(&mut self, _tick_payload: &TimerTickPayload) {
            self.artifact.timer_tick_emissions += 1;
        }

        fn update_tray(&mut self, _tick_payload: &TimerTickPayload) {
            self.artifact.tray_updates += 1;
        }

        fn complete_session_after_tick<'a>(
            &'a mut self,
        ) -> Pin<Box<dyn Future<Output = ()> + Send + 'a>> {
            Box::pin(async move {
                self.artifact.session_completions += 1;
            })
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

    async fn run_escape_combo_exit_with_transition_for_tests(
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

                while !release.load(std::sync::atomic::Ordering::Acquire) {
                    tokio::time::sleep(StdDuration::from_millis(10)).await;
                }

                let mut events = events
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner);
                events.push(completed_label);
            })
            .await;
    }

    async fn replace_timer_state(app_state: &AppState, timer_state: TimerState) {
        let mut timer = app_state.timer_manager.lock().await;
        let _ = timer.apply_persisted_state(timer_state);
    }

    #[tokio::test]
    async fn test_idle_tick_with_zero_remaining_has_no_observable_effects() {
        let state = build_test_state_without_persistence_path().await;
        replace_timer_state(
            &state,
            TimerState {
                status: TimerStatus::Idle,
                session_type: SessionType::Work,
                remaining_seconds: 0,
                current_cycle: 3,
                completed_pomodoros: 8,
                break_interaction_count: 5,
                is_fullscreen_break: false,
                ..TimerState::default()
            },
        )
        .await;

        let mut effects = RecordingTimerTickRuntimeEffects::default();
        handle_timer_tick_with_effects(&state, &mut effects).await;

        assert_eq!(effects.artifact, TimerTickEffectArtifact::default());

        let timer = state.timer_manager.lock().await;
        let timer_state = timer.get_state();
        assert_eq!(timer_state.status, TimerStatus::Idle);
        assert_eq!(timer_state.session_type, SessionType::Work);
        assert_eq!(timer_state.remaining_seconds, 0);
        assert_eq!(timer_state.current_cycle, 3);
        assert_eq!(timer_state.completed_pomodoros, 8);
        assert_eq!(timer_state.break_interaction_count, 5);
        assert!(!timer_state.is_fullscreen_break);
    }

    #[tokio::test]
    async fn test_work_tick_at_boundary_emits_tick_and_requests_session_completion() {
        let state = build_test_state_without_persistence_path().await;
        replace_timer_state(
            &state,
            TimerState {
                status: TimerStatus::Working,
                session_type: SessionType::Work,
                remaining_seconds: 1,
                ..TimerState::default()
            },
        )
        .await;

        let mut effects = RecordingTimerTickRuntimeEffects::default();
        handle_timer_tick_with_effects(&state, &mut effects).await;

        assert_eq!(
            effects.artifact,
            TimerTickEffectArtifact {
                timer_tick_emissions: 1,
                tray_updates: 1,
                session_completions: 1,
            }
        );

        let timer = state.timer_manager.lock().await;
        let timer_state = timer.get_state();
        assert_eq!(timer_state.status, TimerStatus::Working);
        assert_eq!(timer_state.session_type, SessionType::Work);
        assert_eq!(timer_state.remaining_seconds, 0);
    }

    fn timer_settings(
        work_duration_secs: u64,
        short_break_duration_secs: u64,
        long_break_duration_secs: u64,
        long_break_interval: u32,
    ) -> TimerSettings {
        TimerSettings::try_from_raw(
            work_duration_secs,
            short_break_duration_secs,
            long_break_duration_secs,
            long_break_interval,
            PostBreakTransitionPolicy::RequireManualContinue,
        )
        .expect("timer settings should be valid")
    }

    fn restorable_window_state_for_tests() -> WindowState {
        WindowState::from_restorable_main_window_geometry(
            PhysicalPosition { x: 120, y: 80 },
            PhysicalSize {
                width: 1024,
                height: 768,
            },
        )
        .expect("window geometry should be restorable")
    }

    async fn complete_work_session_with_recorded_effects(
        app_state: &AppState,
        timer_state: TimerState,
    ) -> RecordingSessionPipelineRuntimeEffects {
        replace_timer_state(app_state, timer_state).await;

        let transition_outcome = {
            let mut timer = app_state.timer_manager.lock().await;
            let completed_payload = timer.complete_work_session_transition();
            SessionTransitionOutcome::from_completed_payload(&completed_payload)
        };

        let mut effects = RecordingSessionPipelineRuntimeEffects::default();
        apply_session_completion_runtime_effects(
            app_state,
            &transition_outcome,
            false,
            &mut effects,
        )
        .await;
        effects
    }

    fn emitted_timer_tick_payload(
        effects: &RecordingSessionPipelineRuntimeEffects,
    ) -> &serde_json::Value {
        &effects
            .emitted_events
            .iter()
            .find(|event| event.event_name == "timer_tick")
            .expect("timer_tick event should be emitted")
            .event_payload
    }

    fn assert_session_completion_emits(
        transition_outcome: &SessionTransitionOutcome,
        expected_event_names: &[&'static str],
    ) {
        let tick_payload = TimerState::default().to_tick_payload();
        let mut effects = RecordingSessionPipelineRuntimeEffects::default();

        emit_session_completion_events_with_effects(
            &mut effects,
            transition_outcome,
            &tick_payload,
        );

        assert_eq!(effects.emitted_event_names.as_slice(), expected_event_names);
        assert_eq!(effects.tray_update_count, 1);
    }

    #[test]
    fn test_work_to_break_session_completion_emits_session_completed_then_timer_tick() {
        assert_session_completion_emits(
            &SessionTransitionOutcome::work_to_break(SessionType::ShortBreak),
            &["session_completed", "timer_tick"],
        );
    }

    #[tokio::test]
    async fn test_work_completion_emits_short_break_tick_with_completed_count_and_configured_duration(
    ) {
        let state = build_test_state_without_persistence_path().await;
        let effects = complete_work_session_with_recorded_effects(
            &state,
            TimerState {
                status: TimerStatus::Working,
                session_type: SessionType::Work,
                remaining_seconds: 0,
                current_cycle: 1,
                completed_pomodoros: 7,
                settings: timer_settings(1500, 420, 1200, 4),
                ..TimerState::default()
            },
        )
        .await;

        assert_eq!(
            effects.emitted_event_names.as_slice(),
            ["session_completed", "timer_tick"]
        );
        assert_eq!(effects.tray_update_count, 1);

        let timer_tick_payload = emitted_timer_tick_payload(&effects);
        assert_eq!(
            timer_tick_payload["status"],
            serde_json::to_value(TimerStatus::OnBreak).expect("status should serialize")
        );
        assert_eq!(
            timer_tick_payload["session_type"],
            serde_json::to_value(SessionType::ShortBreak).expect("session type should serialize")
        );
        assert_eq!(
            timer_tick_payload["remaining_seconds"],
            serde_json::json!(420)
        );
        assert_eq!(
            timer_tick_payload["completed_pomodoros"],
            serde_json::json!(8)
        );
    }

    #[tokio::test]
    async fn test_work_completion_emits_long_break_tick_with_completed_count_and_configured_duration(
    ) {
        let state = build_test_state_without_persistence_path().await;
        let effects = complete_work_session_with_recorded_effects(
            &state,
            TimerState {
                status: TimerStatus::Working,
                session_type: SessionType::Work,
                remaining_seconds: 0,
                current_cycle: 4,
                completed_pomodoros: 7,
                settings: timer_settings(1500, 420, 1200, 4),
                ..TimerState::default()
            },
        )
        .await;

        assert_eq!(
            effects.emitted_event_names.as_slice(),
            ["session_completed", "timer_tick"]
        );
        assert_eq!(effects.tray_update_count, 1);

        let timer_tick_payload = emitted_timer_tick_payload(&effects);
        assert_eq!(
            timer_tick_payload["status"],
            serde_json::to_value(TimerStatus::OnBreak).expect("status should serialize")
        );
        assert_eq!(
            timer_tick_payload["session_type"],
            serde_json::to_value(SessionType::LongBreak).expect("session type should serialize")
        );
        assert_eq!(
            timer_tick_payload["remaining_seconds"],
            serde_json::json!(1200)
        );
        assert_eq!(
            timer_tick_payload["completed_pomodoros"],
            serde_json::json!(8)
        );
    }

    #[test]
    fn test_break_waiting_decision_session_completion_emits_session_completed_then_timer_tick() {
        assert_session_completion_emits(
            &SessionTransitionOutcome::break_waiting_decision(SessionType::ShortBreak),
            &["session_completed", "timer_tick"],
        );
    }

    #[test]
    fn test_break_auto_started_session_completion_emits_session_completed_then_timer_tick() {
        assert_session_completion_emits(
            &SessionTransitionOutcome::break_auto_started(SessionType::ShortBreak),
            &["session_completed", "timer_tick"],
        );
    }

    #[test]
    fn test_scheduled_stop_session_completion_emits_auto_run_stopped_before_timer_tick() {
        assert_session_completion_emits(
            &SessionTransitionOutcome::scheduled_stop_executed(SessionType::Work),
            &["session_completed", "auto_run_stopped", "timer_tick"],
        );
    }

    #[tokio::test]
    async fn test_break_mode_transition_coordinator_serializes_escape_combo_exit() {
        let state = Arc::new(build_test_state_without_persistence_path().await);
        let events = Arc::new(StdMutex::new(Vec::new()));
        let release_first = Arc::new(StdAtomicBool::new(false));

        let first_transition = {
            let state = Arc::clone(&state);
            let events = Arc::clone(&events);
            let release_first = Arc::clone(&release_first);
            tokio::spawn(async move {
                run_escape_combo_exit_with_transition_for_tests(
                    state,
                    events,
                    "first-start",
                    "first-end",
                    release_first,
                )
                .await;
            })
        };

        wait_for_transition_event_count(&events, 1, "the first escape-combo exit transition").await;

        let second_transition = {
            let state = Arc::clone(&state);
            let events = Arc::clone(&events);
            tokio::spawn(async move {
                run_escape_combo_exit_with_transition_for_tests(
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

        release_first.store(true, std::sync::atomic::Ordering::Release);

        first_transition
            .await
            .expect("first escape-combo transition should complete");
        second_transition
            .await
            .expect("second escape-combo transition should complete");

        assert_eq!(
            events
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .as_slice(),
            ["first-start", "first-end", "second-start", "second-end"]
        );
    }

    #[tokio::test]
    async fn test_failed_break_entry_releases_partial_fullscreen_lock() {
        let state = build_test_state_without_persistence_path().await;
        replace_timer_state(
            &state,
            TimerState {
                status: TimerStatus::OnBreak,
                session_type: SessionType::ShortBreak,
                remaining_seconds: 1,
                ..TimerState::default()
            },
        )
        .await;
        let mut effects =
            RecordingBreakLockRuntimeEffects::with_enter_error("simulated enter failure");

        let result = enter_break_mode_with_effects(&state, &mut effects).await;

        assert!(matches!(
            result,
            Err(("enter_fullscreen_all_monitors", PomoliceError::WindowControl(message)))
                if message == "simulated enter failure"
        ));
        assert_eq!(effects.lock_state, RecordedBreakLockState::Unlocked);
        assert_eq!(effects.emitted_event_names, Vec::<&'static str>::new());

        {
            let input_monitor = state.input_monitor.lock().await;
            assert!(
                !input_monitor.is_enabled_for_tests(),
                "failed break entry must leave input monitoring disabled"
            );
        }

        let timer = state.timer_manager.lock().await;
        assert!(
            !timer.get_state().is_fullscreen_break,
            "failed break entry must not be exposed as an active fullscreen break"
        );
    }

    #[tokio::test]
    async fn test_failed_break_entry_release_uses_captured_window_state() {
        let state = build_test_state_without_persistence_path().await;
        replace_timer_state(
            &state,
            TimerState {
                status: TimerStatus::OnBreak,
                session_type: SessionType::ShortBreak,
                remaining_seconds: 1,
                ..TimerState::default()
            },
        )
        .await;
        let captured_window_state = restorable_window_state_for_tests();
        let mut effects =
            RecordingBreakLockRuntimeEffects::with_enter_error("simulated enter failure")
                .with_captured_window_state(captured_window_state.clone());

        let result = enter_break_mode_with_effects(&state, &mut effects).await;

        assert!(matches!(
            result,
            Err(("enter_fullscreen_all_monitors", PomoliceError::WindowControl(message)))
                if message == "simulated enter failure"
        ));
        assert_eq!(
            effects.released_restore_plans,
            [MainWindowRestorePlan::Saved(captured_window_state)]
        );
    }

    #[tokio::test]
    async fn test_failed_break_entry_keeps_original_error_when_lock_release_fails() {
        let state = build_test_state_without_persistence_path().await;
        let mut effects =
            RecordingBreakLockRuntimeEffects::with_enter_error("simulated enter failure")
                .with_release_error("simulated release failure");

        let result = enter_break_mode_with_effects(&state, &mut effects).await;

        assert!(matches!(
            result,
            Err(("enter_fullscreen_all_monitors", PomoliceError::WindowControl(message)))
                if message == "simulated enter failure"
        ));
        assert_eq!(effects.lock_state, RecordedBreakLockState::PartiallyLocked);
        assert_eq!(effects.emitted_event_names, Vec::<&'static str>::new());

        {
            let input_monitor = state.input_monitor.lock().await;
            assert!(
                !input_monitor.is_enabled_for_tests(),
                "input monitoring rollback should not depend on lock release success"
            );
        }
    }

    #[tokio::test]
    async fn test_break_entry_success_marks_lock_active_and_emits_started() {
        let state = build_test_state_without_persistence_path().await;
        replace_timer_state(
            &state,
            TimerState {
                status: TimerStatus::OnBreak,
                session_type: SessionType::ShortBreak,
                remaining_seconds: 1,
                ..TimerState::default()
            },
        )
        .await;
        let mut effects = RecordingBreakLockRuntimeEffects::default();

        enter_break_mode_with_effects(&state, &mut effects)
            .await
            .expect("break entry should succeed");

        assert_eq!(effects.lock_state, RecordedBreakLockState::Locked);
        assert_eq!(effects.emitted_event_names, ["break_started"]);

        let timer = state.timer_manager.lock().await;
        assert!(timer.get_state().is_fullscreen_break);
    }

    #[tokio::test]
    async fn test_break_exit_releases_lock_clears_flag_and_emits_completed() {
        let state = build_test_state_without_persistence_path().await;
        replace_timer_state(
            &state,
            TimerState {
                status: TimerStatus::OnBreak,
                session_type: SessionType::ShortBreak,
                remaining_seconds: 1,
                is_fullscreen_break: true,
                ..TimerState::default()
            },
        )
        .await;
        {
            let mut input_monitor = state.input_monitor.lock().await;
            input_monitor.start();
        }
        let mut effects = RecordingBreakLockRuntimeEffects {
            lock_state: RecordedBreakLockState::Locked,
            ..RecordingBreakLockRuntimeEffects::default()
        };

        exit_break_mode_with_effects(&state, &mut effects)
            .await
            .expect("break exit should succeed");

        assert_eq!(effects.lock_state, RecordedBreakLockState::Unlocked);
        assert_eq!(effects.emitted_event_names, ["break_completed"]);

        {
            let input_monitor = state.input_monitor.lock().await;
            assert!(!input_monitor.is_enabled_for_tests());
        }

        let timer = state.timer_manager.lock().await;
        assert!(!timer.get_state().is_fullscreen_break);
    }

    #[tokio::test]
    async fn test_break_exit_uses_window_state_captured_on_break_entry() {
        let state = build_test_state_without_persistence_path().await;
        replace_timer_state(
            &state,
            TimerState {
                status: TimerStatus::OnBreak,
                session_type: SessionType::ShortBreak,
                remaining_seconds: 1,
                ..TimerState::default()
            },
        )
        .await;
        let captured_window_state = restorable_window_state_for_tests();
        let mut effects = RecordingBreakLockRuntimeEffects::default()
            .with_captured_window_state(captured_window_state.clone());

        enter_break_mode_with_effects(&state, &mut effects)
            .await
            .expect("break entry should succeed");
        exit_break_mode_with_effects(&state, &mut effects)
            .await
            .expect("break exit should succeed");

        assert_eq!(
            effects.released_restore_plans,
            [MainWindowRestorePlan::Saved(captured_window_state)]
        );
    }
}

#[cfg(test)]
mod tests {
    use super::{apply_input_penalty_with_effects, InputPenaltyRuntimeEffects};
    use crate::external_adapters::clock::testing::FixedClock;
    use crate::external_adapters::clock::Clock;
    use crate::external_adapters::persistence::PersistenceManager;
    use crate::logging_event::PERSISTENCE_WRITE_TRIGGER_INPUT_PENALTY;
    use crate::state::AppState;
    use crate::types::{SessionType, TimerState, TimerStatus, TimerTickPayload};
    use chrono::{Local, TimeZone, Utc};
    use std::future::Future;
    use std::pin::Pin;
    use std::sync::Arc;
    use std::time::{Duration, Instant};

    #[derive(Debug, Default, PartialEq, Eq)]
    struct InputPenaltyEffectArtifact {
        persistence_triggers: Vec<&'static str>,
        timer_tick_emit_count: usize,
        tray_update_count: usize,
    }

    #[derive(Default)]
    struct RecordingInputPenaltyRuntimeEffects {
        artifact: InputPenaltyEffectArtifact,
    }

    impl RecordingInputPenaltyRuntimeEffects {
        fn artifact(&self) -> &InputPenaltyEffectArtifact {
            &self.artifact
        }
    }

    impl InputPenaltyRuntimeEffects for RecordingInputPenaltyRuntimeEffects {
        fn persist_input_penalty<'a>(
            &'a mut self,
        ) -> Pin<Box<dyn Future<Output = ()> + Send + 'a>> {
            Box::pin(async move {
                self.artifact
                    .persistence_triggers
                    .push(PERSISTENCE_WRITE_TRIGGER_INPUT_PENALTY);
            })
        }

        fn emit_timer_tick(&mut self, _tick_payload: &TimerTickPayload) {
            self.artifact.timer_tick_emit_count += 1;
        }

        fn update_tray(&mut self, _tick_payload: &TimerTickPayload) {
            self.artifact.tray_update_count += 1;
        }
    }

    fn fixed_clock() -> Arc<FixedClock> {
        let now_utc = Utc
            .with_ymd_and_hms(2025, 2, 1, 9, 0, 0)
            .single()
            .expect("valid UTC datetime");
        Arc::new(FixedClock::new(now_utc.with_timezone(&Local)))
    }

    fn as_clock_trait(clock: &Arc<FixedClock>) -> Arc<dyn Clock + Send + Sync> {
        Arc::<FixedClock>::clone(clock) as Arc<dyn Clock + Send + Sync>
    }

    async fn build_test_state_without_persistence_path() -> AppState {
        let clock = fixed_clock();
        let persistence = PersistenceManager::new(as_clock_trait(&clock), None);
        let (app_state, _) =
            AppState::build_startup_state_for_tests(as_clock_trait(&clock), persistence).await;
        app_state
    }

    async fn replace_timer_state(app_state: &AppState, timer_state: TimerState) {
        let mut timer = app_state.timer_manager.lock().await;
        let _ = timer.apply_persisted_state(timer_state);
    }

    #[tokio::test]
    async fn test_input_penalty_side_effects() {
        let app_state = build_test_state_without_persistence_path().await;
        replace_timer_state(
            &app_state,
            TimerState {
                status: TimerStatus::OnBreak,
                session_type: SessionType::ShortBreak,
                remaining_seconds: 1,
                ..TimerState::default()
            },
        )
        .await;

        let mut effects = RecordingInputPenaltyRuntimeEffects::default();
        apply_input_penalty_with_effects(&app_state, &mut effects).await;

        assert_eq!(
            effects.artifact().persistence_triggers,
            [PERSISTENCE_WRITE_TRIGGER_INPUT_PENALTY]
        );
        assert_eq!(effects.artifact().timer_tick_emit_count, 1);
        assert_eq!(effects.artifact().tray_update_count, 1);

        let skipped_app_state = build_test_state_without_persistence_path().await;
        let mut skipped_effects = RecordingInputPenaltyRuntimeEffects::default();

        apply_input_penalty_with_effects(&skipped_app_state, &mut skipped_effects).await;

        assert_eq!(
            skipped_effects.artifact(),
            &InputPenaltyEffectArtifact::default()
        );
    }

    #[tokio::test]
    async fn test_skipped_input_penalty_has_no_side_effects() {
        let app_state = build_test_state_without_persistence_path().await;
        replace_timer_state(
            &app_state,
            TimerState {
                status: TimerStatus::OnBreak,
                session_type: SessionType::ShortBreak,
                remaining_seconds: 1,
                last_penalty_time: Some(Instant::now() + Duration::from_secs(60)),
                break_interaction_count: 1,
                ..TimerState::default()
            },
        )
        .await;

        let mut effects = RecordingInputPenaltyRuntimeEffects::default();
        apply_input_penalty_with_effects(&app_state, &mut effects).await;

        assert_eq!(effects.artifact(), &InputPenaltyEffectArtifact::default());
    }

    #[tokio::test]
    async fn test_working_input_penalty_has_no_observable_effects() {
        let app_state = build_test_state_without_persistence_path().await;
        replace_timer_state(
            &app_state,
            TimerState {
                status: TimerStatus::Working,
                session_type: SessionType::Work,
                remaining_seconds: 123,
                break_interaction_count: 7,
                ..TimerState::default()
            },
        )
        .await;

        let mut effects = RecordingInputPenaltyRuntimeEffects::default();
        apply_input_penalty_with_effects(&app_state, &mut effects).await;

        assert_eq!(effects.artifact(), &InputPenaltyEffectArtifact::default());

        let timer = app_state.timer_manager.lock().await;
        let timer_state = timer.get_state();
        assert_eq!(timer_state.status, TimerStatus::Working);
        assert_eq!(timer_state.session_type, SessionType::Work);
        assert_eq!(timer_state.remaining_seconds, 123);
        assert_eq!(timer_state.break_interaction_count, 7);
    }
}
