use std::future::Future;
use std::pin::Pin;

use tauri::{AppHandle, Emitter, Manager};

use crate::logging_event::{
    log_error_event, log_info_event, log_persistence_write_failure, BackendLogEvent,
    PERSISTENCE_WRITE_TRIGGER_INPUT_PENALTY, PERSISTENCE_WRITE_TRIGGER_SESSION_COMPLETION,
};
use crate::managers::input_monitor::InputMonitorManager;
use crate::managers::window::{WindowState, WindowStateManager};
use crate::state;
use crate::tray;
use crate::types;

type BreakModeTransitionResult = Result<(), (&'static str, crate::error::PomoliceError)>;
type TimerTickFuture = Pin<Box<dyn Future<Output = ()> + Send>>;

struct FinalizedSessionCompletionState {
    tick_payload: types::TimerTickPayload,
    post_break_transition_policy: types::PostBreakTransitionPolicy,
    post_break_decision_pending: bool,
    scheduled_stop_state: types::ScheduledStopState,
    timer_start_source: Option<types::TimerStartSource>,
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
        tray::update_tray_icon(self.app, tick_payload);
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

trait BreakLockRuntimeEffects {
    fn capture_main_window_state_for_break<'a>(
        &'a mut self,
    ) -> Pin<Box<dyn Future<Output = crate::error::Result<Option<WindowState>>> + Send + 'a>>;
    fn enter_fullscreen_break_lock<'a>(
        &'a mut self,
    ) -> Pin<Box<dyn Future<Output = crate::error::Result<()>> + Send + 'a>>;
    fn release_fullscreen_break_lock(
        &mut self,
        window_manager: &mut WindowStateManager,
    ) -> crate::error::Result<()>;
    fn emit_break_started(&mut self);
    fn emit_break_completed(&mut self);
}

struct AppHandleBreakLockRuntimeEffects<'a> {
    app: &'a AppHandle,
}

impl BreakLockRuntimeEffects for AppHandleBreakLockRuntimeEffects<'_> {
    fn capture_main_window_state_for_break<'a>(
        &'a mut self,
    ) -> Pin<Box<dyn Future<Output = crate::error::Result<Option<WindowState>>> + Send + 'a>> {
        Box::pin(
            async move { WindowStateManager::capture_main_window_state_for_break(self.app).await },
        )
    }

    fn enter_fullscreen_break_lock<'a>(
        &'a mut self,
    ) -> Pin<Box<dyn Future<Output = crate::error::Result<()>> + Send + 'a>> {
        Box::pin(async move {
            WindowStateManager::enter_fullscreen_all_monitors(self.app.clone()).await
        })
    }

    fn release_fullscreen_break_lock(
        &mut self,
        window_manager: &mut WindowStateManager,
    ) -> crate::error::Result<()> {
        window_manager.exit_fullscreen_all_monitors(self.app)
    }

    fn emit_break_started(&mut self) {
        let _ = self.app.emit("break_started", serde_json::json!({}));
    }

    fn emit_break_completed(&mut self) {
        let _ = self.app.emit("break_completed", serde_json::json!({}));
    }
}

fn lib_handle_timer_tick_future(app: AppHandle) -> TimerTickFuture {
    Box::pin(async move {
        let state_handle = app.state::<state::AppState>();

        let (should_complete_session, tick_payload) = {
            let mut timer = state_handle.timer_manager.lock().await;

            if timer.status() == types::TimerStatus::Idle {
                return;
            }

            let (completed, payload) = timer.tick();

            (completed, payload)
        };

        let _ = app.emit("timer_tick", &tick_payload);
        tray::update_tray_icon(&app, &tick_payload);

        if should_complete_session {
            lib_handle_session_complete(app).await;
        }
    })
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
        let _ = self.app.emit("timer_tick", tick_payload);
    }

    fn update_tray(&mut self, tick_payload: &types::TimerTickPayload) {
        tray::update_tray_icon(self.app, tick_payload);
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
        scheduled_stop_state: state.scheduled_stop_state,
        timer_start_source: state.timer_start_source,
    }
}

fn log_session_completion_finalized(
    transition_outcome: &types::SessionTransitionOutcome,
    finalized_state: &FinalizedSessionCompletionState,
) {
    let transition_reason = transition_outcome.transition_reason;
    let post_break_transition_policy = finalized_state.post_break_transition_policy;
    let post_break_decision_pending = finalized_state.post_break_decision_pending;
    let scheduled_stop_state = finalized_state.scheduled_stop_state;
    let timer_start_source = finalized_state.timer_start_source;

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
            ("scheduled_stop_state", format!("{scheduled_stop_state:?}")),
            ("timer_start_source", format!("{timer_start_source:?}")),
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

async fn start_input_monitor_for_break(state_handle: &state::AppState) -> InputMonitorManager {
    let mut input_monitor = state_handle.input_monitor.lock().await;
    input_monitor.start();
    input_monitor.clone()
}

async fn rollback_break_enter_failure(
    state_handle: &state::AppState,
    input_monitor: &InputMonitorManager,
    effects: &mut (impl BreakLockRuntimeEffects + Send),
) {
    input_monitor.rollback_break_enter_failure();

    let mut window_manager = state_handle.window_manager.lock().await;
    if let Err(error) = effects.release_fullscreen_break_lock(&mut window_manager) {
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

            emit_startup_resume_events_with_effects(effects, &tick_payload);
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
            let input_monitor = start_input_monitor_for_break(state_handle).await;

            let saved_state = effects
                .capture_main_window_state_for_break()
                .await
                .map_err(|error| {
                    input_monitor.rollback_break_enter_failure();
                    ("capture_main_window_state_for_break", error)
                })?;

            {
                let mut window_manager = state_handle.window_manager.lock().await;
                window_manager.set_saved_state(saved_state);
            }

            if let Err(error) = effects.enter_fullscreen_break_lock().await {
                rollback_break_enter_failure(state_handle, &input_monitor, effects).await;
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
            {
                let mut input_monitor = state_handle.input_monitor.lock().await;
                input_monitor.stop();
            }

            {
                let mut window_manager = state_handle.window_manager.lock().await;
                effects
                    .release_fullscreen_break_lock(&mut window_manager)
                    .map_err(|error| ("exit_fullscreen_all_monitors", error))?;
                log_info_event(BackendLogEvent::SessionFullscreenBreakExited, &[]);
            }

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
            {
                let mut input_monitor = state_handle.input_monitor.lock().await;
                input_monitor.stop();
            }

            {
                let mut window_manager = state_handle.window_manager.lock().await;
                if let Err(error) = window_manager.exit_fullscreen_all_monitors(&app) {
                    log_break_mode_failure(
                        BackendLogEvent::SessionFullscreenBreakExitFailed,
                        "escape_combo_exit_fullscreen_all_monitors",
                        &error,
                    );
                }
            }
        })
        .await;
}

pub(crate) async fn lib_handle_external_break_exit(app: AppHandle) {
    let state_handle = app.state::<state::AppState>();

    state_handle
        .run_break_mode_transition(async {
            {
                let mut input_monitor = state_handle.input_monitor.lock().await;
                input_monitor.stop();
            }

            {
                let mut window_manager = state_handle.window_manager.lock().await;
                if let Err(error) = window_manager.exit_fullscreen_all_monitors(&app) {
                    log_break_mode_failure(
                        BackendLogEvent::SessionFullscreenBreakExitFailed,
                        "external_break_exit_fullscreen_all_monitors",
                        &error,
                    );
                }
            }
        })
        .await;
}

#[cfg(test)]
mod emit_order_tests {
    use super::{
        apply_session_completion_runtime_effects, emit_session_completion_events_with_effects,
        enter_break_mode_with_effects, exit_break_mode_with_effects, BreakLockRuntimeEffects,
        SessionPipelineRuntimeEffects,
    };
    use crate::error::PomoliceError;
    use crate::managers::clock::testing::FixedClock;
    use crate::managers::clock::Clock;
    use crate::managers::persistence::PersistenceManager;
    use crate::managers::window::{MainWindowRestorePlan, WindowState, WindowStateManager};
    use crate::state::AppState;
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
        ) -> Pin<Box<dyn Future<Output = crate::error::Result<Option<WindowState>>> + Send + 'a>>
        {
            Box::pin(async move {
                if let Some(error) = self.capture_error.take() {
                    return Err(error);
                }

                Ok(self.captured_window_state.clone())
            })
        }

        fn enter_fullscreen_break_lock<'a>(
            &'a mut self,
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

        fn release_fullscreen_break_lock(
            &mut self,
            window_manager: &mut WindowStateManager,
        ) -> crate::error::Result<()> {
            if let Some(error) = self.release_error.take() {
                return Err(error);
            }

            self.released_restore_plans
                .push(window_manager.main_window_restore_plan());
            self.lock_state = RecordedBreakLockState::Unlocked;
            Ok(())
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
    use crate::logging_event::PERSISTENCE_WRITE_TRIGGER_INPUT_PENALTY;
    use crate::managers::clock::testing::FixedClock;
    use crate::managers::clock::Clock;
    use crate::managers::persistence::PersistenceManager;
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
}
