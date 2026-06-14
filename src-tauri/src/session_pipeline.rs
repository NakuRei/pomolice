use std::future::Future;
use std::pin::Pin;

use tauri::{AppHandle, Emitter, Manager};

type TimerTickFuture = Pin<Box<dyn Future<Output = ()> + Send>>;

use crate::logging_event::{
    log_error_event, log_info_event, log_persistence_write_failure, BackendLogEvent,
    PERSISTENCE_WRITE_TRIGGER_INPUT_PENALTY, PERSISTENCE_WRITE_TRIGGER_SESSION_COMPLETION,
};
use crate::state;
use crate::tray;
use crate::types;

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

async fn start_input_monitor_for_break(
    state_handle: &state::AppState,
) -> crate::managers::input_monitor::InputMonitorManager {
    let mut input_monitor = state_handle.input_monitor.lock().await;
    input_monitor.start();
    input_monitor.clone()
}

async fn rollback_break_enter_failure(
    state_handle: &state::AppState,
    app: &AppHandle,
    input_monitor: &crate::managers::input_monitor::InputMonitorManager,
) {
    input_monitor.rollback_break_enter_failure();

    let mut window_manager = state_handle.window_manager.lock().await;
    if let Err(error) = window_manager.exit_fullscreen_all_monitors(app) {
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

    let enter_result = state_handle
        .run_break_mode_transition(async {
            let input_monitor = start_input_monitor_for_break(&state_handle).await;

            let saved_state =
                crate::managers::window::WindowStateManager::capture_main_window_state_for_break(
                    &app,
                )
                .await
                .map_err(|error| {
                    input_monitor.rollback_break_enter_failure();
                    ("capture_main_window_state_for_break", error)
                })?;

            {
                let mut window_manager = state_handle.window_manager.lock().await;
                window_manager.set_saved_state(saved_state);
            }

            if let Err(error) =
                crate::managers::window::WindowStateManager::enter_fullscreen_all_monitors(
                    app.clone(),
                )
                .await
            {
                rollback_break_enter_failure(&state_handle, &app, &input_monitor).await;
                return Err(("enter_fullscreen_all_monitors", error));
            }

            {
                log_info_event(BackendLogEvent::SessionFullscreenBreakEntered, &[]);
            }

            {
                let mut timer = state_handle.timer_manager.lock().await;
                timer.set_fullscreen_break(true);
            }

            let _ = app.emit("break_started", serde_json::json!({}));
            Ok::<(), (&'static str, crate::error::PomoliceError)>(())
        })
        .await;

    if let Err((step, error)) = enter_result {
        log_break_mode_failure(
            BackendLogEvent::SessionFullscreenBreakEnterFailed,
            step,
            &error,
        );
    }
}

async fn exit_break_mode(app: AppHandle) {
    log_info_event(BackendLogEvent::SessionFullscreenBreakExitStarted, &[]);
    let state_handle = app.state::<state::AppState>();

    let exit_result = state_handle
        .run_break_mode_transition(async {
            {
                let mut input_monitor = state_handle.input_monitor.lock().await;
                input_monitor.stop();
            }

            {
                let mut window_manager = state_handle.window_manager.lock().await;
                window_manager
                    .exit_fullscreen_all_monitors(&app)
                    .map_err(|error| ("exit_fullscreen_all_monitors", error))?;
                log_info_event(BackendLogEvent::SessionFullscreenBreakExited, &[]);
            }

            {
                let mut timer = state_handle.timer_manager.lock().await;
                timer.set_fullscreen_break(false);
            }

            let _ = app.emit("break_completed", serde_json::json!({}));
            Ok::<(), (&'static str, crate::error::PomoliceError)>(())
        })
        .await;

    if let Err((step, error)) = exit_result {
        log_break_mode_failure(
            BackendLogEvent::SessionFullscreenBreakExitFailed,
            step,
            &error,
        );
    }
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
        emit_session_completion_events_with_effects, start_input_monitor_for_break,
        SessionPipelineRuntimeEffects,
    };
    use crate::error::PomoliceError;
    use crate::managers::clock::testing::FixedClock;
    use crate::managers::clock::Clock;
    use crate::managers::persistence::PersistenceManager;
    use crate::state::AppState;
    use crate::types::{SessionTransitionOutcome, SessionType, TimerState, TimerTickPayload};
    use chrono::{Local, TimeZone, Utc};
    use std::future::Future;
    use std::pin::Pin;
    use std::sync::atomic::AtomicBool as StdAtomicBool;
    use std::sync::{Arc, Mutex as StdMutex};
    use std::time::Duration as StdDuration;

    #[derive(Default)]
    struct RecordingSessionPipelineRuntimeEffects {
        emitted_event_names: Vec<&'static str>,
        tray_update_count: usize,
    }

    impl SessionPipelineRuntimeEffects for RecordingSessionPipelineRuntimeEffects {
        fn emit_event<T: serde::Serialize>(&mut self, event_name: &'static str, _payload: &T) {
            self.emitted_event_names.push(event_name);
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

    async fn run_enter_break_mode_failure_for_tests(
        state: &AppState,
    ) -> Result<(), (&'static str, PomoliceError)> {
        let input_monitor = start_input_monitor_for_break(state).await;
        Err::<(), _>(PomoliceError::WindowControl(
            "simulated enter failure".to_string(),
        ))
        .map_err(|error| {
            input_monitor.rollback_break_enter_failure();
            ("enter_fullscreen_all_monitors", error)
        })
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
    async fn test_enter_break_mode_failure_stops_input_monitor_before_returning() {
        let state = build_test_state_without_persistence_path().await;

        let result = run_enter_break_mode_failure_for_tests(&state).await;

        assert!(matches!(
            result,
            Err(("enter_fullscreen_all_monitors", PomoliceError::WindowControl(message)))
                if message == "simulated enter failure"
        ));

        let input_monitor = state.input_monitor.lock().await;
        assert!(
            !input_monitor.is_enabled_for_tests(),
            "enter-break fullscreen failure must roll input monitoring back before returning"
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
