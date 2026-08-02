use chrono::{DateTime, Duration as ChronoDuration, Local};
use std::collections::HashSet;
use std::future::Future;
use std::pin::Pin;
use std::time::Duration;
use tauri::{AppHandle, Emitter, Manager};
use tokio::time::MissedTickBehavior;
use tokio_util::sync::CancellationToken;

use crate::logging_event::{
    log_info_event, log_persistence_write_failure, BackendLogEvent,
    PERSISTENCE_WRITE_TRIGGER_SCHEDULER_STOP_PENDING,
};
use crate::schedule_boundary::{collect_crossed_end_boundaries, collect_crossed_start_boundaries};
use crate::state;
use crate::timer_state_transition::{AutoRunBoundaryChangeIdentity, ScheduleEndBoundaryResult};
use crate::types::{BoundaryOccurrenceId, TimerSettings, TimerStatus, WeeklyAutoRunConfig};
use crate::work_start::{self, StartWorkAttempt, StartWorkRuntimeEffects};

const SCHEDULER_INTERVAL_SECONDS: u64 = 1;
const PROCESSED_BOUNDARY_RETENTION_DAYS: i64 = 8;

#[derive(Default)]
struct ProcessedBoundaryOccurrences {
    occurrences: HashSet<BoundaryOccurrenceId>,
}

impl ProcessedBoundaryOccurrences {
    fn record_if_first_seen(&mut self, boundary: &BoundaryOccurrenceId) -> bool {
        self.occurrences.insert(boundary.clone())
    }

    fn clear_for_schedule_topology_change(&mut self) {
        self.occurrences.clear();
    }

    fn discard_stale_or_invalid(&mut self, now_local: DateTime<Local>) {
        let oldest_epoch_to_keep =
            (now_local - ChronoDuration::days(PROCESSED_BOUNDARY_RETENTION_DAYS)).timestamp();

        self.occurrences.retain(|boundary_id| {
            boundary_id
                .epoch_seconds()
                .is_some_and(|epoch_seconds| epoch_seconds >= oldest_epoch_to_keep)
        });
    }
}

struct SchedulerRuntimeState {
    processed_boundaries: ProcessedBoundaryOccurrences,
    last_checked_local: DateTime<Local>,
    start_boundaries_blocked_until_idle: bool,
    last_observed_auto_run_boundary_change: AutoRunBoundaryChangeIdentity,
}

struct SchedulerTimerSnapshot {
    status: TimerStatus,
    auto_run_feature_enabled: bool,
    weekly_auto_run_config: WeeklyAutoRunConfig,
    timer_settings: TimerSettings,
    auto_run_boundary_change_identity: AutoRunBoundaryChangeIdentity,
}

struct SchedulerTickPlan {
    end_boundary_previous_local: DateTime<Local>,
    should_evaluate_start_boundaries: bool,
}

trait SchedulerRuntimeEffects {
    fn emit_auto_run_stop_pending(&mut self);
    fn persist_current_state_with_trigger<'a>(
        &'a mut self,
        trigger: &'static str,
    ) -> Pin<Box<dyn Future<Output = ()> + Send + 'a>>;
}

struct AppHandleSchedulerRuntimeEffects<'a> {
    app: &'a AppHandle,
}

impl SchedulerRuntimeEffects for AppHandleSchedulerRuntimeEffects<'_> {
    fn emit_auto_run_stop_pending(&mut self) {
        let _ = self
            .app
            .emit("auto_run_stop_pending", serde_json::json!({}));
    }

    fn persist_current_state_with_trigger<'a>(
        &'a mut self,
        trigger: &'static str,
    ) -> Pin<Box<dyn Future<Output = ()> + Send + 'a>> {
        Box::pin(async move {
            persist_current_state_with_trigger(self.app, trigger).await;
        })
    }
}

impl StartWorkRuntimeEffects for AppHandleSchedulerRuntimeEffects<'_> {
    fn emit_timer_tick(&mut self, tick_payload: &crate::types::TimerTickPayload) {
        let _ = self.app.emit("timer_tick", tick_payload);
    }

    fn update_tray(&mut self, tick_payload: &crate::types::TimerTickPayload) {
        state::update_tray_icon(self.app, tick_payload);
    }

    fn emit_work_started(&mut self, work_duration_secs: u64) {
        let _ = self.app.emit(
            "work_started",
            serde_json::json!({ "duration": work_duration_secs }),
        );
    }

    fn spawn_timer_task<'a>(
        &'a mut self,
        _state: &'a state::AppState,
    ) -> Pin<Box<dyn Future<Output = ()> + Send + 'a>> {
        Box::pin(async move {
            crate::session_pipeline::spawn_timer_task(self.app).await;
        })
    }
}

#[cfg(test)]
pub(crate) struct SchedulerTickTestRuntime {
    runtime_state: SchedulerRuntimeState,
}

#[cfg(test)]
impl SchedulerTickTestRuntime {
    pub(crate) async fn from_state(
        state_handle: &state::AppState,
        previous_local: DateTime<Local>,
    ) -> Self {
        let timer_snapshot = load_scheduler_timer_snapshot(state_handle).await;
        Self::new(
            previous_local,
            timer_snapshot.auto_run_boundary_change_identity,
        )
    }

    fn new(
        previous_local: DateTime<Local>,
        last_observed_auto_run_boundary_change: AutoRunBoundaryChangeIdentity,
    ) -> Self {
        Self {
            runtime_state: SchedulerRuntimeState {
                processed_boundaries: ProcessedBoundaryOccurrences::default(),
                last_checked_local: previous_local,
                start_boundaries_blocked_until_idle: false,
                last_observed_auto_run_boundary_change,
            },
        }
    }

    pub(crate) async fn run_tick(
        &mut self,
        state_handle: &state::AppState,
        now_local: DateTime<Local>,
    ) {
        let mut effects = NoopSchedulerRuntimeEffects;
        run_scheduler_tick_with_effects(
            state_handle,
            &mut self.runtime_state,
            now_local,
            &mut effects,
        )
        .await;
    }
}

#[cfg(test)]
struct NoopSchedulerRuntimeEffects;

#[cfg(test)]
impl SchedulerRuntimeEffects for NoopSchedulerRuntimeEffects {
    fn emit_auto_run_stop_pending(&mut self) {}

    fn persist_current_state_with_trigger<'a>(
        &'a mut self,
        _trigger: &'static str,
    ) -> Pin<Box<dyn Future<Output = ()> + Send + 'a>> {
        Box::pin(async {})
    }
}

#[cfg(test)]
impl StartWorkRuntimeEffects for NoopSchedulerRuntimeEffects {
    fn emit_timer_tick(&mut self, _tick_payload: &crate::types::TimerTickPayload) {}

    fn update_tray(&mut self, _tick_payload: &crate::types::TimerTickPayload) {}

    fn emit_work_started(&mut self, _work_duration_secs: u64) {}

    fn spawn_timer_task<'a>(
        &'a mut self,
        _state: &'a state::AppState,
    ) -> Pin<Box<dyn Future<Output = ()> + Send + 'a>> {
        Box::pin(async {})
    }
}

async fn persist_current_state_with_trigger(app: &AppHandle, trigger: &'static str) {
    let state_handle = app.state::<state::AppState>();

    if let Err(error) = state_handle.persist_current_state_async().await {
        log_persistence_write_failure(trigger, &error);
    }
}

async fn start_scheduler_task_with_spawner<F>(
    state_handle: &state::AppState,
    spawn_scheduler_loop: F,
) where
    F: FnOnce(CancellationToken, DateTime<Local>, bool) -> tauri::async_runtime::JoinHandle<()>,
{
    state_handle.cancel_and_wait_scheduler_task().await;

    let now_local = state_handle.clock.now_local();
    let startup_timer_status = {
        let timer_manager = state_handle.timer_manager.lock().await;
        timer_manager.status()
    };

    let mut start_boundaries_blocked_until_idle = false;
    let mut initial_previous_local = now_local;

    if matches!(
        startup_timer_status,
        TimerStatus::Working | TimerStatus::OnBreak
    ) {
        start_boundaries_blocked_until_idle = true;
        initial_previous_local = state_handle
            .restored_running_saved_at_local()
            .unwrap_or(now_local - ChronoDuration::days(7));
    }

    let cancellation_token = CancellationToken::new();
    let scheduler_join_handle = spawn_scheduler_loop(
        cancellation_token.clone(),
        initial_previous_local,
        start_boundaries_blocked_until_idle,
    );

    state_handle.store_scheduler_task(cancellation_token, scheduler_join_handle);
}

fn spawn_app_scheduler_loop(
    app: AppHandle,
    scheduler_cancellation_token: CancellationToken,
    initial_previous_local: DateTime<Local>,
    start_boundaries_blocked_until_idle: bool,
) -> tauri::async_runtime::JoinHandle<()> {
    tauri::async_runtime::spawn(async move {
        run_app_scheduler_loop(
            app,
            scheduler_cancellation_token,
            initial_previous_local,
            start_boundaries_blocked_until_idle,
        )
        .await;
    })
}

async fn run_app_scheduler_loop(
    scheduler_app: AppHandle,
    scheduler_cancellation_token: CancellationToken,
    initial_previous_local: DateTime<Local>,
    start_boundaries_blocked_until_idle: bool,
) {
    let mut scheduler_interval =
        tokio::time::interval(Duration::from_secs(SCHEDULER_INTERVAL_SECONDS));
    scheduler_interval.set_missed_tick_behavior(MissedTickBehavior::Skip);

    let state_handle = scheduler_app.state::<state::AppState>();
    let timer_snapshot = load_scheduler_timer_snapshot(&state_handle).await;
    let mut runtime_state = SchedulerRuntimeState {
        processed_boundaries: ProcessedBoundaryOccurrences::default(),
        last_checked_local: initial_previous_local,
        start_boundaries_blocked_until_idle,
        last_observed_auto_run_boundary_change: timer_snapshot.auto_run_boundary_change_identity,
    };

    log_info_event(
        BackendLogEvent::SchedulerStarted,
        &[
            (
                "previous_local",
                runtime_state.last_checked_local.to_rfc3339(),
            ),
            (
                "blocked_until_idle",
                runtime_state
                    .start_boundaries_blocked_until_idle
                    .to_string(),
            ),
        ],
    );

    loop {
        tokio::select! {
            () = scheduler_cancellation_token.cancelled() => {
                log_info_event(BackendLogEvent::SchedulerReceivedCancellation, &[]);
                break;
            }
            _ = scheduler_interval.tick() => {
                lib_run_scheduler_tick(scheduler_app.clone(), &mut runtime_state).await;
            }
        }
    }

    log_info_event(BackendLogEvent::SchedulerStopped, &[]);
}

pub async fn lib_start_scheduler_task(app: AppHandle) {
    let state_handle = app.state::<state::AppState>();
    let scheduler_app = app.clone();
    start_scheduler_task_with_spawner(
        &state_handle,
        move |cancellation_token, previous, blocked| {
            spawn_app_scheduler_loop(scheduler_app, cancellation_token, previous, blocked)
        },
    )
    .await;
}

async fn lib_run_scheduler_tick(app: AppHandle, runtime_state: &mut SchedulerRuntimeState) {
    let state_handle = app.state::<state::AppState>();
    let now_local = state_handle.clock.now_local();
    let mut effects = AppHandleSchedulerRuntimeEffects { app: &app };

    run_scheduler_tick_with_effects(&state_handle, runtime_state, now_local, &mut effects).await;
}

async fn run_scheduler_tick_with_effects<S>(
    state_handle: &state::AppState,
    runtime_state: &mut SchedulerRuntimeState,
    now_local: DateTime<Local>,
    effects: &mut S,
) where
    S: SchedulerRuntimeEffects + StartWorkRuntimeEffects,
{
    let previous_local = runtime_state.last_checked_local;

    let timer_snapshot = load_scheduler_timer_snapshot(state_handle).await;
    let resumed_start_boundary_evaluation =
        resume_start_boundary_evaluation_when_idle(runtime_state, timer_snapshot.status);

    let tick_plan = build_scheduler_tick_plan(
        state_handle,
        runtime_state,
        now_local,
        &timer_snapshot,
        resumed_start_boundary_evaluation,
    );

    process_crossed_end_boundaries_with_effects(
        state_handle,
        runtime_state,
        &timer_snapshot.weekly_auto_run_config,
        timer_snapshot.auto_run_feature_enabled,
        tick_plan.end_boundary_previous_local,
        now_local,
        effects,
    )
    .await;

    if tick_plan.should_evaluate_start_boundaries {
        process_crossed_start_boundaries_with_effects(
            state_handle,
            runtime_state,
            &timer_snapshot,
            previous_local,
            now_local,
            effects,
        )
        .await;
    }

    runtime_state
        .processed_boundaries
        .discard_stale_or_invalid(now_local);
    runtime_state.last_checked_local = now_local;
}

async fn load_scheduler_timer_snapshot(state_handle: &state::AppState) -> SchedulerTimerSnapshot {
    let timer_manager = state_handle.timer_manager.lock().await;
    let auto_run_boundary_observation = timer_manager.observe_auto_run_boundary_scope();
    let timer_state = timer_manager.get_state();

    SchedulerTimerSnapshot {
        status: timer_manager.status(),
        auto_run_feature_enabled: auto_run_boundary_observation.auto_run_feature_enabled,
        weekly_auto_run_config: auto_run_boundary_observation.weekly_auto_run_config,
        timer_settings: timer_state.settings.clone(),
        auto_run_boundary_change_identity: auto_run_boundary_observation.change_identity,
    }
}

fn resume_start_boundary_evaluation_when_idle(
    runtime_state: &mut SchedulerRuntimeState,
    timer_status: TimerStatus,
) -> bool {
    if runtime_state.start_boundaries_blocked_until_idle && timer_status == TimerStatus::Idle {
        runtime_state.start_boundaries_blocked_until_idle = false;
        log_info_event(
            BackendLogEvent::SchedulerResumedStartBoundaryEvaluation,
            &[],
        );
        return true;
    }

    false
}

fn build_scheduler_tick_plan(
    state_handle: &state::AppState,
    runtime_state: &mut SchedulerRuntimeState,
    now_local: DateTime<Local>,
    timer_snapshot: &SchedulerTimerSnapshot,
    resumed_start_boundary_evaluation: bool,
) -> SchedulerTickPlan {
    let auto_run_boundary_scope_changed = timer_snapshot.auto_run_boundary_change_identity
        != runtime_state.last_observed_auto_run_boundary_change;
    let mut skip_start_boundaries_this_tick = false;

    let end_boundary_previous_local = if auto_run_boundary_scope_changed {
        runtime_state
            .processed_boundaries
            .clear_for_schedule_topology_change();
        runtime_state.last_checked_local = now_local;
        runtime_state.last_observed_auto_run_boundary_change =
            timer_snapshot.auto_run_boundary_change_identity;
        skip_start_boundaries_this_tick = true;
        log_info_event(
            BackendLogEvent::SchedulerConfigRevisionChanged,
            &[(
                "config_revision",
                timer_snapshot.auto_run_boundary_change_identity.to_string(),
            )],
        );
        now_local - ChronoDuration::seconds(1)
    } else {
        runtime_state.last_checked_local
    };

    let should_evaluate_start_boundaries = timer_snapshot.auto_run_feature_enabled
        && state_handle.start_boundaries_allowed_after_startup_prompt()
        && !runtime_state.start_boundaries_blocked_until_idle
        && !resumed_start_boundary_evaluation
        && !skip_start_boundaries_this_tick;

    SchedulerTickPlan {
        end_boundary_previous_local,
        should_evaluate_start_boundaries,
    }
}

async fn process_crossed_end_boundaries_with_effects(
    state_handle: &state::AppState,
    runtime_state: &mut SchedulerRuntimeState,
    weekly_auto_run_config: &WeeklyAutoRunConfig,
    auto_run_feature_enabled: bool,
    previous_local: DateTime<Local>,
    now_local: DateTime<Local>,
    effects: &mut impl SchedulerRuntimeEffects,
) {
    if !auto_run_feature_enabled {
        return;
    }

    let crossed_end_boundaries =
        collect_crossed_end_boundaries(previous_local, now_local, weekly_auto_run_config);

    for end_boundary in crossed_end_boundaries {
        if !runtime_state
            .processed_boundaries
            .record_if_first_seen(&end_boundary)
        {
            continue;
        }

        log_info_event(
            BackendLogEvent::SchedulerCrossedEndBoundary,
            &[("boundary_id", end_boundary.as_str().to_string())],
        );
        let did_mark_stop_pending =
            mark_scheduled_stop_pending_with_effects(state_handle, effects).await;
        if did_mark_stop_pending {
            log_info_event(
                BackendLogEvent::SchedulerMarkedStopPending,
                &[("boundary_id", end_boundary.as_str().to_string())],
            );
        }
    }
}

async fn process_crossed_start_boundaries_with_effects<S>(
    state_handle: &state::AppState,
    runtime_state: &mut SchedulerRuntimeState,
    timer_snapshot: &SchedulerTimerSnapshot,
    previous_local: DateTime<Local>,
    now_local: DateTime<Local>,
    effects: &mut S,
) where
    S: SchedulerRuntimeEffects + StartWorkRuntimeEffects,
{
    let crossed_start_boundaries = collect_crossed_start_boundaries(
        previous_local,
        now_local,
        &timer_snapshot.weekly_auto_run_config,
    );

    for start_boundary in crossed_start_boundaries {
        if !runtime_state
            .processed_boundaries
            .record_if_first_seen(&start_boundary)
        {
            continue;
        }

        log_info_event(
            BackendLogEvent::SchedulerCrossedStartBoundary,
            &[("boundary_id", start_boundary.as_str().to_string())],
        );
        let did_start = start_work_from_scheduler_when_idle_with_effects(
            state_handle,
            timer_snapshot.status,
            timer_snapshot.timer_settings.clone(),
            &start_boundary,
            effects,
        )
        .await;

        if did_start {
            log_info_event(
                BackendLogEvent::SchedulerStartedWorkFromBoundary,
                &[("boundary_id", start_boundary.as_str().to_string())],
            );
        }
    }
}

async fn mark_scheduled_stop_pending_with_effects(
    state_handle: &state::AppState,
    effects: &mut impl SchedulerRuntimeEffects,
) -> bool {
    let should_persist = {
        let mut timer_manager = state_handle.timer_manager.lock().await;
        timer_manager.apply_schedule_end_boundary_to_current_session()
            == ScheduleEndBoundaryResult::StopScheduledAtSessionBoundary
    };

    if should_persist {
        effects.emit_auto_run_stop_pending();
        effects
            .persist_current_state_with_trigger(PERSISTENCE_WRITE_TRIGGER_SCHEDULER_STOP_PENDING)
            .await;
    }

    should_persist
}

async fn start_work_from_scheduler_when_idle_with_effects<S>(
    state_handle: &state::AppState,
    timer_status_at_tick_start: TimerStatus,
    timer_settings: TimerSettings,
    start_boundary: &BoundaryOccurrenceId,
    effects: &mut S,
) -> bool
where
    S: SchedulerRuntimeEffects + StartWorkRuntimeEffects,
{
    matches!(
        work_start::start_scheduler_work_with_boundary_context(
            state_handle,
            timer_status_at_tick_start,
            timer_settings,
            Some(start_boundary.as_str()),
            effects,
        )
        .await,
        Ok(StartWorkAttempt::Accepted)
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::Result;
    use crate::external_adapters::clock::testing::FixedClock;
    use crate::external_adapters::clock::Clock;
    use crate::external_adapters::persistence::{PersistenceManager, SaveError};
    use crate::log_redaction::RedactionClass;
    use crate::logging_event::{
        log_persistence_write_failure, PERSISTENCE_WRITE_TRIGGER_SCHEDULER_AUTO_START,
        PERSISTENCE_WRITE_TRIGGER_SCHEDULER_STOP_PENDING,
    };
    use crate::session_pipeline::{
        sync_automation_policies_with_runtime_effects, TimerCommandRuntimeEffectShell,
    };
    use crate::types::{
        DayAutoRunSlot, HhMm, PostBreakTransitionPolicy, SessionTransitionReason, SessionType,
        TimerTickPayload, WeeklyAutoRunConfig, WorkStartOrigin,
    };
    use crate::work_start::StartWorkPersistencePolicy;
    use chrono::{LocalResult, TimeZone, Utc};
    use std::cell::RefCell;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::{Arc, Once};

    struct SchedulerTestLogger;

    static SCHEDULER_TEST_LOGGER: SchedulerTestLogger = SchedulerTestLogger;
    static SCHEDULER_TEST_LOGGER_INIT: Once = Once::new();

    thread_local! {
        static CAPTURED_LOG_MESSAGES: RefCell<Option<Vec<String>>> = const { RefCell::new(None) };
    }

    impl log::Log for SchedulerTestLogger {
        fn enabled(&self, metadata: &log::Metadata<'_>) -> bool {
            metadata.level() <= log::Level::Info
        }

        fn log(&self, record: &log::Record<'_>) {
            if !self.enabled(record.metadata()) {
                return;
            }

            CAPTURED_LOG_MESSAGES.with(|captured_messages| {
                if let Some(messages) = captured_messages.borrow_mut().as_mut() {
                    messages.push(record.args().to_string());
                }
            });
        }

        fn flush(&self) {}
    }

    fn begin_log_capture() {
        SCHEDULER_TEST_LOGGER_INIT.call_once(|| {
            log::set_logger(&SCHEDULER_TEST_LOGGER)
                .expect("scheduler tests should install the log collector first");
            log::set_max_level(log::LevelFilter::Info);
        });

        CAPTURED_LOG_MESSAGES.with(|captured_messages| {
            let mut captured_messages = captured_messages.borrow_mut();
            assert!(
                captured_messages.is_none(),
                "log capture should not be nested"
            );
            *captured_messages = Some(Vec::new());
        });
    }

    fn finish_log_capture() -> Vec<String> {
        CAPTURED_LOG_MESSAGES.with(|captured_messages| {
            captured_messages
                .borrow_mut()
                .take()
                .expect("log capture should be active")
        })
    }

    #[derive(Debug, Clone, Copy, PartialEq)]
    struct RecordedTickPayload {
        status: TimerStatus,
        remaining_seconds: u64,
        session_type: SessionType,
    }

    impl RecordedTickPayload {
        fn from_payload(payload: &TimerTickPayload) -> Self {
            Self {
                status: payload.status,
                remaining_seconds: payload.remaining_seconds,
                session_type: payload.session_type,
            }
        }
    }

    #[derive(Default)]
    struct RecordingSchedulerRuntimeEffects {
        timer_tick_payloads: Vec<RecordedTickPayload>,
        tray_update_payloads: Vec<RecordedTickPayload>,
        work_started_durations: Vec<u64>,
        timer_task_spawn_count: usize,
        auto_run_stop_pending_count: usize,
        persist_triggers: Vec<&'static str>,
        simulate_persistence_failure: bool,
        logged_persistence_failure_count: usize,
    }

    impl RecordingSchedulerRuntimeEffects {
        fn record_persistence_attempt(&mut self, trigger: &'static str) {
            self.persist_triggers.push(trigger);
            if self.simulate_persistence_failure {
                self.logged_persistence_failure_count += 1;
                let error = std::io::Error::other("simulated scheduler persistence failure");
                log_persistence_write_failure(trigger, &error);
            }
        }
    }

    impl SchedulerRuntimeEffects for RecordingSchedulerRuntimeEffects {
        fn emit_auto_run_stop_pending(&mut self) {
            self.auto_run_stop_pending_count += 1;
        }

        fn persist_current_state_with_trigger<'a>(
            &'a mut self,
            trigger: &'static str,
        ) -> Pin<Box<dyn Future<Output = ()> + Send + 'a>> {
            self.record_persistence_attempt(trigger);
            Box::pin(async {})
        }
    }

    impl StartWorkRuntimeEffects for RecordingSchedulerRuntimeEffects {
        fn emit_timer_tick(&mut self, tick_payload: &TimerTickPayload) {
            self.timer_tick_payloads
                .push(RecordedTickPayload::from_payload(tick_payload));
        }

        fn update_tray(&mut self, tick_payload: &TimerTickPayload) {
            self.tray_update_payloads
                .push(RecordedTickPayload::from_payload(tick_payload));
        }

        fn emit_work_started(&mut self, work_duration_secs: u64) {
            self.work_started_durations.push(work_duration_secs);
        }

        fn spawn_timer_task<'a>(
            &'a mut self,
            _state: &'a state::AppState,
        ) -> Pin<Box<dyn Future<Output = ()> + Send + 'a>> {
            self.timer_task_spawn_count += 1;
            Box::pin(async {})
        }

        fn persist_current_state<'a>(
            &'a mut self,
            _state: &'a state::AppState,
            policy: StartWorkPersistencePolicy,
        ) -> Pin<Box<dyn Future<Output = Result<()>> + Send + 'a>> {
            let StartWorkPersistencePolicy::LogAndContinue { trigger } = policy else {
                return Box::pin(async { Ok(()) });
            };

            self.record_persistence_attempt(trigger);
            Box::pin(async { Ok(()) })
        }
    }

    impl TimerCommandRuntimeEffectShell for RecordingSchedulerRuntimeEffects {
        fn emit_tick(&mut self, tick_payload: &TimerTickPayload) {
            self.timer_tick_payloads
                .push(RecordedTickPayload::from_payload(tick_payload));
        }

        fn update_tray(&mut self, tick_payload: &TimerTickPayload) {
            self.tray_update_payloads
                .push(RecordedTickPayload::from_payload(tick_payload));
        }

        fn spawn_timer_task<'a>(
            &'a mut self,
            _state_handle: &'a state::AppState,
        ) -> Pin<Box<dyn Future<Output = ()> + Send + 'a>> {
            self.timer_task_spawn_count += 1;
            Box::pin(async {})
        }

        fn request_fullscreen_break_exit(&mut self) {}
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

    async fn build_test_state_without_persistence_path() -> state::AppState {
        let clock = fixed_clock(create_utc_datetime(2025, 2, 1, 9, 0, 0));
        let persistence = PersistenceManager::new(as_clock_trait(&clock), None);
        let (app_state, _) =
            state::AppState::build_startup_state_for_tests(as_clock_trait(&clock), persistence)
                .await;
        app_state
    }

    async fn build_test_state_with_persistence_failure() -> state::AppState {
        let clock = fixed_clock(create_utc_datetime(2025, 2, 1, 9, 0, 0));
        let persistence =
            PersistenceManager::new(as_clock_trait(&clock), Some(std::path::PathBuf::from(".")))
                .with_injected_save_error_hook(Arc::new(|| {
                    Some(SaveError::Serialize {
                        source: serde_json::from_str::<serde_json::Value>("{")
                            .expect_err("invalid JSON should fail to parse"),
                    })
                }));
        let (app_state, _) =
            state::AppState::build_startup_state_for_tests(as_clock_trait(&clock), persistence)
                .await;
        app_state
    }

    fn spawn_counted_scheduler_loop(
        cancellation_token: CancellationToken,
        active_loop_count: Arc<AtomicUsize>,
    ) -> tauri::async_runtime::JoinHandle<()> {
        active_loop_count.fetch_add(1, Ordering::SeqCst);

        tauri::async_runtime::spawn(async move {
            cancellation_token.cancelled().await;
            active_loop_count.fetch_sub(1, Ordering::SeqCst);
        })
    }

    #[tokio::test]
    async fn test_scheduler_restart_keeps_only_latest_loop_active() {
        let state = build_test_state_without_persistence_path().await;
        let active_loop_count = Arc::new(AtomicUsize::new(0));

        for _ in 0..6 {
            let active_loop_count_for_spawner = Arc::clone(&active_loop_count);
            start_scheduler_task_with_spawner(
                &state,
                move |cancellation_token, _previous, _blocked| {
                    spawn_counted_scheduler_loop(cancellation_token, active_loop_count_for_spawner)
                },
            )
            .await;
        }

        assert_eq!(
            active_loop_count.load(Ordering::SeqCst),
            1,
            "scheduler restart should leave only the latest loop active"
        );

        state.cancel_and_wait_scheduler_task().await;
    }

    fn scheduler_boundary_id() -> BoundaryOccurrenceId {
        BoundaryOccurrenceId::for_schedule_boundary("mon", "start", 1_738_562_400)
    }

    #[tokio::test]
    async fn test_scheduler_auto_start_side_effects() {
        let state = build_test_state_without_persistence_path().await;
        let timer_settings = TimerSettings::default();
        let start_boundary = scheduler_boundary_id();
        let mut effects = RecordingSchedulerRuntimeEffects::default();

        let did_start = start_work_from_scheduler_when_idle_with_effects(
            &state,
            TimerStatus::Idle,
            timer_settings,
            &start_boundary,
            &mut effects,
        )
        .await;

        assert!(did_start);
        assert_eq!(
            effects.timer_tick_payloads,
            [RecordedTickPayload {
                status: TimerStatus::Working,
                remaining_seconds: 1500,
                session_type: SessionType::Work,
            }]
        );
        assert_eq!(effects.tray_update_payloads, effects.timer_tick_payloads);
        assert_eq!(effects.work_started_durations, [1500]);
        assert_eq!(effects.timer_task_spawn_count, 1);
        assert_eq!(effects.auto_run_stop_pending_count, 0);
        assert_eq!(
            effects.persist_triggers,
            [PERSISTENCE_WRITE_TRIGGER_SCHEDULER_AUTO_START]
        );

        let timer = state.timer_manager.lock().await;
        assert_eq!(timer.status(), TimerStatus::Working);
    }

    #[tokio::test]
    async fn test_scheduler_stop_pending_side_effects() {
        let state = build_test_state_without_persistence_path().await;
        {
            let mut timer = state.timer_manager.lock().await;
            let _ = timer.sync_automation_policies(
                true,
                crate::types::WeeklyAutoRunConfig::default(),
                PostBreakTransitionPolicy::RequireManualContinue,
            );
            timer
                .start_work(
                    TimerSettings::default(),
                    WorkStartOrigin::ManualInsideScheduleWindow,
                )
                .expect("initial work session should start");
        }
        let mut effects = RecordingSchedulerRuntimeEffects::default();

        let did_mark_pending = mark_scheduled_stop_pending_with_effects(&state, &mut effects).await;

        assert!(did_mark_pending);
        assert_eq!(effects.timer_tick_payloads.len(), 0);
        assert_eq!(effects.tray_update_payloads.len(), 0);
        assert_eq!(effects.work_started_durations.len(), 0);
        assert_eq!(effects.timer_task_spawn_count, 0);
        assert_eq!(effects.auto_run_stop_pending_count, 1);
        assert_eq!(
            effects.persist_triggers,
            [PERSISTENCE_WRITE_TRIGGER_SCHEDULER_STOP_PENDING]
        );

        let mut timer = state.timer_manager.lock().await;
        let payload = timer.complete_work_session_transition();
        assert_eq!(
            payload.transition_reason,
            SessionTransitionReason::ScheduledStopExecuted
        );
    }

    #[tokio::test]
    async fn test_scheduler_persistence_failure_policy_is_logged_and_nonfatal() {
        let state = build_test_state_without_persistence_path().await;
        let start_boundary = scheduler_boundary_id();
        let mut effects = RecordingSchedulerRuntimeEffects {
            simulate_persistence_failure: true,
            ..RecordingSchedulerRuntimeEffects::default()
        };

        let did_start = start_work_from_scheduler_when_idle_with_effects(
            &state,
            TimerStatus::Idle,
            TimerSettings::default(),
            &start_boundary,
            &mut effects,
        )
        .await;

        assert!(did_start);
        assert_eq!(effects.logged_persistence_failure_count, 1);
        assert_eq!(
            effects.persist_triggers,
            [PERSISTENCE_WRITE_TRIGGER_SCHEDULER_AUTO_START]
        );
        assert_eq!(
            state.timer_manager.lock().await.status(),
            TimerStatus::Working
        );
    }

    fn local_datetime(
        year: i32,
        month: u32,
        day: u32,
        hour: u32,
        minute: u32,
        second: u32,
    ) -> DateTime<Local> {
        match Local.with_ymd_and_hms(year, month, day, hour, minute, second) {
            LocalResult::Single(datetime) => datetime,
            LocalResult::Ambiguous(first, _) => first,
            LocalResult::None => panic!("test local datetime should exist"),
        }
    }

    fn enabled_daytime_slot() -> DayAutoRunSlot {
        DayAutoRunSlot::try_new(
            true,
            HhMm::try_new(9, 0).expect("start time should be valid"),
            HhMm::try_new(18, 0).expect("end time should be valid"),
        )
        .expect("daytime slot should be valid")
    }

    fn day_slot(enabled: bool, start_hour: u8, end_hour: u8) -> DayAutoRunSlot {
        DayAutoRunSlot::try_new(
            enabled,
            HhMm::try_new(start_hour, 0).expect("start time should be valid"),
            HhMm::try_new(end_hour, 0).expect("end time should be valid"),
        )
        .expect("day slot should be valid")
    }

    fn weekly_config_with_weekday_slot(
        weekday_index: usize,
        slot: DayAutoRunSlot,
    ) -> WeeklyAutoRunConfig {
        let disabled = DayAutoRunSlot::default();

        match weekday_index {
            0 => WeeklyAutoRunConfig::try_new(
                slot, disabled, disabled, disabled, disabled, disabled, disabled,
            ),
            1 => WeeklyAutoRunConfig::try_new(
                disabled, slot, disabled, disabled, disabled, disabled, disabled,
            ),
            2 => WeeklyAutoRunConfig::try_new(
                disabled, disabled, slot, disabled, disabled, disabled, disabled,
            ),
            3 => WeeklyAutoRunConfig::try_new(
                disabled, disabled, disabled, slot, disabled, disabled, disabled,
            ),
            4 => WeeklyAutoRunConfig::try_new(
                disabled, disabled, disabled, disabled, slot, disabled, disabled,
            ),
            5 => WeeklyAutoRunConfig::try_new(
                disabled, disabled, disabled, disabled, disabled, slot, disabled,
            ),
            6 => WeeklyAutoRunConfig::try_new(
                disabled, disabled, disabled, disabled, disabled, disabled, slot,
            ),
            _ => panic!("weekday index should be between zero and six"),
        }
        .expect("weekly configuration should be valid")
    }

    fn weekly_config_with_monday_daytime() -> WeeklyAutoRunConfig {
        WeeklyAutoRunConfig::try_new(
            enabled_daytime_slot(),
            DayAutoRunSlot::default(),
            DayAutoRunSlot::default(),
            DayAutoRunSlot::default(),
            DayAutoRunSlot::default(),
            DayAutoRunSlot::default(),
            DayAutoRunSlot::default(),
        )
        .expect("weekly config should be valid")
    }

    fn weekly_config_with_monday_and_tuesday_daytime() -> WeeklyAutoRunConfig {
        WeeklyAutoRunConfig::try_new(
            enabled_daytime_slot(),
            enabled_daytime_slot(),
            DayAutoRunSlot::default(),
            DayAutoRunSlot::default(),
            DayAutoRunSlot::default(),
            DayAutoRunSlot::default(),
            DayAutoRunSlot::default(),
        )
        .expect("weekly config should be valid")
    }

    fn complete_startup_auto_start_prompt_gate(state: &state::AppState) {
        state.allow_start_boundaries_after_startup_prompt();
    }

    async fn apply_schedule(
        state: &state::AppState,
        config: WeeklyAutoRunConfig,
        post_break_policy: PostBreakTransitionPolicy,
    ) {
        apply_schedule_with_feature_enabled(state, true, config, post_break_policy).await;
    }

    async fn apply_schedule_with_feature_enabled(
        state: &state::AppState,
        auto_run_feature_enabled: bool,
        config: WeeklyAutoRunConfig,
        post_break_policy: PostBreakTransitionPolicy,
    ) {
        let mut timer = state.timer_manager.lock().await;
        let _ = timer.sync_automation_policies(auto_run_feature_enabled, config, post_break_policy);
    }

    async fn collect_config_change_diagnostic_message_batches(
        initial_auto_run_feature_enabled: bool,
        initial_config: WeeklyAutoRunConfig,
        initial_post_break_policy: PostBreakTransitionPolicy,
        updated_auto_run_feature_enabled: bool,
        updated_config: WeeklyAutoRunConfig,
        updated_post_break_policy: PostBreakTransitionPolicy,
        evaluation_count: usize,
    ) -> Vec<Vec<String>> {
        let state = build_test_state_without_persistence_path().await;
        apply_schedule_with_feature_enabled(
            &state,
            initial_auto_run_feature_enabled,
            initial_config,
            initial_post_break_policy,
        )
        .await;

        let mut runtime_state =
            runtime_state_at(&state, local_datetime(2025, 2, 1, 11, 59, 50)).await;
        let mut effects = RecordingSchedulerRuntimeEffects::default();

        apply_schedule_with_feature_enabled(
            &state,
            updated_auto_run_feature_enabled,
            updated_config,
            updated_post_break_policy,
        )
        .await;

        let mut message_batches = Vec::new();
        for now_local in [
            local_datetime(2025, 2, 1, 12, 0, 0),
            local_datetime(2025, 2, 1, 12, 0, 1),
        ]
        .into_iter()
        .take(evaluation_count)
        {
            begin_log_capture();
            run_scheduler_tick_with_effects(&state, &mut runtime_state, now_local, &mut effects)
                .await;
            message_batches.push(finish_log_capture());
        }
        message_batches
    }

    async fn collect_config_change_diagnostic_messages(
        initial_auto_run_feature_enabled: bool,
        initial_config: WeeklyAutoRunConfig,
        initial_post_break_policy: PostBreakTransitionPolicy,
        updated_auto_run_feature_enabled: bool,
        updated_config: WeeklyAutoRunConfig,
        updated_post_break_policy: PostBreakTransitionPolicy,
        evaluation_count: usize,
    ) -> Vec<String> {
        collect_config_change_diagnostic_message_batches(
            initial_auto_run_feature_enabled,
            initial_config,
            initial_post_break_policy,
            updated_auto_run_feature_enabled,
            updated_config,
            updated_post_break_policy,
            evaluation_count,
        )
        .await
        .into_iter()
        .flatten()
        .collect()
    }

    fn config_change_diagnostic_messages(messages: &[String]) -> Vec<&str> {
        const EVENT_TOKEN: &str = "event_id=scheduler.config_revision_changed";

        messages
            .iter()
            .map(String::as_str)
            .filter(|message| message.split_whitespace().next() == Some(EVENT_TOKEN))
            .collect()
    }

    fn assert_config_change_diagnostic_count(
        scenario: &str,
        messages: &[String],
        expected_count: usize,
    ) {
        let diagnostic_messages = config_change_diagnostic_messages(messages);
        assert_eq!(
            diagnostic_messages.len(),
            expected_count,
            "{scenario}; captured messages: {messages:?}"
        );

        for diagnostic_message in diagnostic_messages {
            let (_, fields) = diagnostic_message
                .split_once(" config_revision=")
                .expect("config-change diagnostic should include its configured field");
            assert!(!fields.is_empty(), "{scenario}");
            assert!(!fields.contains(' '), "{scenario}");
        }
    }

    async fn runtime_state_at(
        state_handle: &state::AppState,
        previous_local: DateTime<Local>,
    ) -> SchedulerRuntimeState {
        let timer_snapshot = load_scheduler_timer_snapshot(state_handle).await;

        SchedulerRuntimeState {
            processed_boundaries: ProcessedBoundaryOccurrences::default(),
            last_checked_local: previous_local,
            start_boundaries_blocked_until_idle: false,
            last_observed_auto_run_boundary_change: timer_snapshot
                .auto_run_boundary_change_identity,
        }
    }

    async fn runtime_state_blocking_start_boundaries_at(
        state_handle: &state::AppState,
        previous_local: DateTime<Local>,
    ) -> SchedulerRuntimeState {
        let timer_snapshot = load_scheduler_timer_snapshot(state_handle).await;

        SchedulerRuntimeState {
            processed_boundaries: ProcessedBoundaryOccurrences::default(),
            last_checked_local: previous_local,
            start_boundaries_blocked_until_idle: true,
            last_observed_auto_run_boundary_change: timer_snapshot
                .auto_run_boundary_change_identity,
        }
    }

    async fn start_work_with_schedule_origin(
        state: &state::AppState,
        auto_run_feature_enabled: bool,
        work_start_origin: WorkStartOrigin,
    ) {
        apply_schedule_with_feature_enabled(
            state,
            auto_run_feature_enabled,
            weekly_config_with_monday_daytime(),
            PostBreakTransitionPolicy::RequireManualContinue,
        )
        .await;

        let mut timer = state.timer_manager.lock().await;
        timer
            .start_work(TimerSettings::default(), work_start_origin)
            .expect("work session should start");
    }

    async fn start_work_with_existing_schedule_origin(
        state: &state::AppState,
        work_start_origin: WorkStartOrigin,
    ) {
        let mut timer = state.timer_manager.lock().await;
        timer
            .start_work(TimerSettings::default(), work_start_origin)
            .expect("work session should start");
    }

    fn assert_no_work_start_effects(effects: &RecordingSchedulerRuntimeEffects) {
        assert!(effects.timer_tick_payloads.is_empty());
        assert!(effects.tray_update_payloads.is_empty());
        assert!(effects.work_started_durations.is_empty());
        assert_eq!(effects.timer_task_spawn_count, 0);
    }

    async fn complete_work_session(state: &state::AppState) -> SessionTransitionReason {
        let mut timer = state.timer_manager.lock().await;
        timer.complete_work_session_transition().transition_reason
    }

    async fn complete_break_session(state: &state::AppState) -> SessionTransitionReason {
        let mut timer = state.timer_manager.lock().await;
        timer.complete_break_session_transition().transition_reason
    }

    fn boundary_id(
        slot_key: &str,
        boundary_type: &str,
        epoch_seconds: i64,
    ) -> BoundaryOccurrenceId {
        BoundaryOccurrenceId::for_schedule_boundary(slot_key, boundary_type, epoch_seconds)
    }

    #[test]
    fn stale_or_invalid_boundary_occurrences_become_eligible_again() {
        let now_local = local_datetime(2025, 1, 10, 12, 0, 0);
        let stale_epoch = (now_local - ChronoDuration::days(10)).timestamp();
        let recent_epoch = (now_local - ChronoDuration::days(2)).timestamp();
        let stale_boundary = boundary_id("mon", "start", stale_epoch);
        let recent_boundary = boundary_id("tue", "end", recent_epoch);
        let invalid_boundary = BoundaryOccurrenceId::from_raw_for_tests("invalid");
        let mut processed_boundaries = ProcessedBoundaryOccurrences::default();

        assert!(processed_boundaries.record_if_first_seen(&stale_boundary));
        assert!(processed_boundaries.record_if_first_seen(&recent_boundary));
        assert!(processed_boundaries.record_if_first_seen(&invalid_boundary));

        processed_boundaries.discard_stale_or_invalid(now_local);

        assert!(processed_boundaries.record_if_first_seen(&stale_boundary));
        assert!(processed_boundaries.record_if_first_seen(&invalid_boundary));
        assert!(!processed_boundaries.record_if_first_seen(&recent_boundary));
    }

    #[tokio::test]
    async fn end_boundary_emits_stop_pending_for_auto_run_session() {
        let state = build_test_state_without_persistence_path().await;
        start_work_with_schedule_origin(&state, true, WorkStartOrigin::ManualInsideScheduleWindow)
            .await;

        let mut runtime_state =
            runtime_state_at(&state, local_datetime(2025, 1, 6, 17, 59, 50)).await;
        let mut effects = RecordingSchedulerRuntimeEffects::default();

        run_scheduler_tick_with_effects(
            &state,
            &mut runtime_state,
            local_datetime(2025, 1, 6, 18, 0, 10),
            &mut effects,
        )
        .await;

        assert_eq!(effects.auto_run_stop_pending_count, 1);
        assert_no_work_start_effects(&effects);
        assert_eq!(
            complete_work_session(&state).await,
            SessionTransitionReason::ScheduledStopExecuted
        );
    }

    #[tokio::test]
    async fn end_boundary_does_not_emit_stop_pending_after_boundary_window_has_passed() {
        let state = build_test_state_without_persistence_path().await;
        start_work_with_schedule_origin(&state, true, WorkStartOrigin::ManualInsideScheduleWindow)
            .await;

        let mut runtime_state =
            runtime_state_at(&state, local_datetime(2025, 1, 6, 17, 59, 50)).await;
        let mut effects = RecordingSchedulerRuntimeEffects::default();

        run_scheduler_tick_with_effects(
            &state,
            &mut runtime_state,
            local_datetime(2025, 1, 6, 18, 0, 10),
            &mut effects,
        )
        .await;
        run_scheduler_tick_with_effects(
            &state,
            &mut runtime_state,
            local_datetime(2025, 1, 6, 18, 0, 30),
            &mut effects,
        )
        .await;

        assert_eq!(effects.auto_run_stop_pending_count, 1);
        assert_no_work_start_effects(&effects);
    }

    #[tokio::test]
    async fn replayed_end_boundary_does_not_mark_new_work_session_pending() {
        let state = build_test_state_without_persistence_path().await;
        start_work_with_schedule_origin(&state, true, WorkStartOrigin::ManualInsideScheduleWindow)
            .await;

        let mut runtime_state =
            runtime_state_at(&state, local_datetime(2025, 1, 6, 17, 59, 50)).await;
        let mut effects = RecordingSchedulerRuntimeEffects::default();
        let crossed_end_boundary = local_datetime(2025, 1, 6, 18, 0, 10);

        run_scheduler_tick_with_effects(
            &state,
            &mut runtime_state,
            crossed_end_boundary,
            &mut effects,
        )
        .await;

        assert_eq!(effects.auto_run_stop_pending_count, 1);
        assert_eq!(
            effects.persist_triggers,
            [PERSISTENCE_WRITE_TRIGGER_SCHEDULER_STOP_PENDING]
        );
        assert_eq!(
            complete_work_session(&state).await,
            SessionTransitionReason::ScheduledStopExecuted
        );

        start_work_with_existing_schedule_origin(&state, WorkStartOrigin::ScheduleStartBoundary)
            .await;
        run_scheduler_tick_with_effects(
            &state,
            &mut runtime_state,
            local_datetime(2025, 1, 6, 17, 59, 50),
            &mut effects,
        )
        .await;
        run_scheduler_tick_with_effects(
            &state,
            &mut runtime_state,
            crossed_end_boundary,
            &mut effects,
        )
        .await;

        assert_eq!(effects.auto_run_stop_pending_count, 1);
        assert_eq!(
            effects.persist_triggers,
            [PERSISTENCE_WRITE_TRIGGER_SCHEDULER_STOP_PENDING]
        );
        assert_no_work_start_effects(&effects);
        assert_eq!(
            complete_work_session(&state).await,
            SessionTransitionReason::WorkToBreak
        );
    }

    #[tokio::test]
    async fn end_boundary_does_not_emit_stop_pending_when_tick_time_does_not_advance() {
        let state = build_test_state_without_persistence_path().await;
        start_work_with_schedule_origin(&state, true, WorkStartOrigin::ManualInsideScheduleWindow)
            .await;

        let mut runtime_state =
            runtime_state_at(&state, local_datetime(2025, 1, 6, 18, 0, 0)).await;
        let mut effects = RecordingSchedulerRuntimeEffects::default();

        run_scheduler_tick_with_effects(
            &state,
            &mut runtime_state,
            local_datetime(2025, 1, 6, 18, 0, 0),
            &mut effects,
        )
        .await;

        assert_eq!(effects.auto_run_stop_pending_count, 0);
        assert_no_work_start_effects(&effects);
        assert_eq!(
            complete_work_session(&state).await,
            SessionTransitionReason::WorkToBreak
        );
    }

    #[tokio::test]
    async fn end_boundary_emits_stop_pending_after_large_time_jump_past_scheduled_end() {
        let state = build_test_state_without_persistence_path().await;
        start_work_with_schedule_origin(&state, true, WorkStartOrigin::ManualInsideScheduleWindow)
            .await;

        let mut runtime_state =
            runtime_state_at(&state, local_datetime(2025, 1, 6, 17, 59, 50)).await;
        let mut effects = RecordingSchedulerRuntimeEffects::default();

        run_scheduler_tick_with_effects(
            &state,
            &mut runtime_state,
            local_datetime(2025, 1, 8, 10, 0, 0),
            &mut effects,
        )
        .await;

        assert_eq!(effects.auto_run_stop_pending_count, 1);
        assert_no_work_start_effects(&effects);
        assert_eq!(
            complete_work_session(&state).await,
            SessionTransitionReason::ScheduledStopExecuted
        );
    }

    #[tokio::test]
    async fn end_boundary_does_not_emit_stop_pending_when_auto_run_is_disabled() {
        let state = build_test_state_without_persistence_path().await;
        start_work_with_schedule_origin(&state, false, WorkStartOrigin::ManualInsideScheduleWindow)
            .await;

        let mut runtime_state =
            runtime_state_at(&state, local_datetime(2025, 1, 6, 17, 59, 50)).await;
        let mut effects = RecordingSchedulerRuntimeEffects::default();

        run_scheduler_tick_with_effects(
            &state,
            &mut runtime_state,
            local_datetime(2025, 1, 6, 18, 0, 10),
            &mut effects,
        )
        .await;

        assert_eq!(effects.auto_run_stop_pending_count, 0);
        assert_no_work_start_effects(&effects);
        assert_eq!(
            complete_work_session(&state).await,
            SessionTransitionReason::WorkToBreak
        );
    }

    #[tokio::test]
    async fn end_boundary_does_not_emit_stop_pending_for_outside_window_session() {
        let state = build_test_state_without_persistence_path().await;
        start_work_with_schedule_origin(&state, true, WorkStartOrigin::ManualOutsideScheduleWindow)
            .await;

        let mut runtime_state =
            runtime_state_at(&state, local_datetime(2025, 1, 6, 17, 59, 50)).await;
        let mut effects = RecordingSchedulerRuntimeEffects::default();

        run_scheduler_tick_with_effects(
            &state,
            &mut runtime_state,
            local_datetime(2025, 1, 6, 18, 0, 10),
            &mut effects,
        )
        .await;

        assert_eq!(effects.auto_run_stop_pending_count, 0);
        assert_no_work_start_effects(&effects);
        assert_eq!(
            complete_work_session(&state).await,
            SessionTransitionReason::WorkToBreak
        );
    }

    #[tokio::test]
    async fn arriving_start_boundary_auto_starts_when_gate_is_open() {
        let state = build_test_state_without_persistence_path().await;
        complete_startup_auto_start_prompt_gate(&state);
        apply_schedule(
            &state,
            weekly_config_with_monday_daytime(),
            PostBreakTransitionPolicy::RequireManualContinue,
        )
        .await;

        let mut runtime_state =
            runtime_state_at(&state, local_datetime(2025, 1, 6, 8, 59, 50)).await;
        let mut effects = RecordingSchedulerRuntimeEffects::default();

        run_scheduler_tick_with_effects(
            &state,
            &mut runtime_state,
            local_datetime(2025, 1, 6, 9, 0, 10),
            &mut effects,
        )
        .await;

        assert!(
            !effects.work_started_durations.is_empty(),
            "an arriving start boundary should auto-start work when the gate is open"
        );
    }

    #[tokio::test]
    async fn replayed_start_boundary_does_not_start_work_again_after_returning_idle() {
        let state = build_test_state_without_persistence_path().await;
        complete_startup_auto_start_prompt_gate(&state);
        apply_schedule(
            &state,
            weekly_config_with_monday_daytime(),
            PostBreakTransitionPolicy::RequireManualContinue,
        )
        .await;

        let mut runtime_state =
            runtime_state_at(&state, local_datetime(2025, 1, 6, 8, 59, 50)).await;
        let mut effects = RecordingSchedulerRuntimeEffects::default();
        let crossed_start_boundary = local_datetime(2025, 1, 6, 9, 0, 10);

        run_scheduler_tick_with_effects(
            &state,
            &mut runtime_state,
            crossed_start_boundary,
            &mut effects,
        )
        .await;

        assert_eq!(effects.work_started_durations, [1500]);
        assert_eq!(effects.timer_tick_payloads.len(), 1);
        assert_eq!(effects.tray_update_payloads.len(), 1);
        assert_eq!(effects.timer_task_spawn_count, 1);
        assert_eq!(
            effects.persist_triggers,
            [PERSISTENCE_WRITE_TRIGGER_SCHEDULER_AUTO_START]
        );
        assert_eq!(
            complete_work_session(&state).await,
            SessionTransitionReason::WorkToBreak
        );
        assert_eq!(
            complete_break_session(&state).await,
            SessionTransitionReason::BreakWaitingDecision
        );

        run_scheduler_tick_with_effects(
            &state,
            &mut runtime_state,
            local_datetime(2025, 1, 6, 8, 59, 50),
            &mut effects,
        )
        .await;
        run_scheduler_tick_with_effects(
            &state,
            &mut runtime_state,
            crossed_start_boundary,
            &mut effects,
        )
        .await;

        assert_eq!(effects.work_started_durations, [1500]);
        assert_eq!(effects.timer_tick_payloads.len(), 1);
        assert_eq!(effects.tray_update_payloads.len(), 1);
        assert_eq!(effects.timer_task_spawn_count, 1);
        assert_eq!(
            effects.persist_triggers,
            [PERSISTENCE_WRITE_TRIGGER_SCHEDULER_AUTO_START]
        );
        assert_eq!(state.timer_manager.lock().await.status(), TimerStatus::Idle);
    }

    #[tokio::test]
    async fn restored_running_session_does_not_replay_past_start_boundary_after_returning_idle() {
        let state = build_test_state_without_persistence_path().await;
        complete_startup_auto_start_prompt_gate(&state);
        apply_schedule(
            &state,
            weekly_config_with_monday_and_tuesday_daytime(),
            PostBreakTransitionPolicy::RequireManualContinue,
        )
        .await;

        let mut runtime_state = runtime_state_blocking_start_boundaries_at(
            &state,
            local_datetime(2025, 1, 6, 8, 59, 50),
        )
        .await;
        let mut effects = RecordingSchedulerRuntimeEffects::default();

        run_scheduler_tick_with_effects(
            &state,
            &mut runtime_state,
            local_datetime(2025, 1, 6, 9, 0, 10),
            &mut effects,
        )
        .await;

        assert_no_work_start_effects(&effects);
        {
            let timer = state.timer_manager.lock().await;
            assert_eq!(timer.status(), TimerStatus::Idle);
        }

        run_scheduler_tick_with_effects(
            &state,
            &mut runtime_state,
            local_datetime(2025, 1, 7, 9, 0, 10),
            &mut effects,
        )
        .await;

        assert_eq!(effects.work_started_durations, [1500]);
        let timer = state.timer_manager.lock().await;
        assert_eq!(timer.status(), TimerStatus::Working);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn config_change_diagnostic_tracks_feature_and_weekday_enablement_mutations() {
        let unchanged_config = WeeklyAutoRunConfig::default();

        let messages = collect_config_change_diagnostic_messages(
            false,
            unchanged_config.clone(),
            PostBreakTransitionPolicy::RequireManualContinue,
            true,
            unchanged_config.clone(),
            PostBreakTransitionPolicy::RequireManualContinue,
            1,
        )
        .await;
        assert_config_change_diagnostic_count("feature enable", &messages, 1);

        let messages = collect_config_change_diagnostic_messages(
            true,
            unchanged_config.clone(),
            PostBreakTransitionPolicy::RequireManualContinue,
            false,
            unchanged_config.clone(),
            PostBreakTransitionPolicy::RequireManualContinue,
            1,
        )
        .await;
        assert_config_change_diagnostic_count("feature disable", &messages, 1);

        for (weekday_index, weekday_key) in [
            (0, "Monday"),
            (1, "Tuesday"),
            (2, "Wednesday"),
            (3, "Thursday"),
            (4, "Friday"),
            (5, "Saturday"),
            (6, "Sunday"),
        ] {
            let enabled_config =
                weekly_config_with_weekday_slot(weekday_index, enabled_daytime_slot());
            let enable_scenario = format!("{weekday_key} enable");
            let messages = collect_config_change_diagnostic_messages(
                true,
                unchanged_config.clone(),
                PostBreakTransitionPolicy::RequireManualContinue,
                true,
                enabled_config.clone(),
                PostBreakTransitionPolicy::RequireManualContinue,
                1,
            )
            .await;
            assert_config_change_diagnostic_count(&enable_scenario, &messages, 1);

            let disable_scenario = format!("{weekday_key} disable");
            let messages = collect_config_change_diagnostic_messages(
                true,
                enabled_config,
                PostBreakTransitionPolicy::RequireManualContinue,
                true,
                unchanged_config.clone(),
                PostBreakTransitionPolicy::RequireManualContinue,
                1,
            )
            .await;
            assert_config_change_diagnostic_count(&disable_scenario, &messages, 1);
        }
    }

    #[tokio::test(flavor = "current_thread")]
    async fn config_change_diagnostic_tracks_all_weekday_time_mutations_and_ignores_non_scope_updates(
    ) {
        let unchanged_config = WeeklyAutoRunConfig::default();

        for (weekday_index, weekday_key) in [
            (0, "Monday"),
            (1, "Tuesday"),
            (2, "Wednesday"),
            (3, "Thursday"),
            (4, "Friday"),
            (5, "Saturday"),
            (6, "Sunday"),
        ] {
            let enabled_config =
                weekly_config_with_weekday_slot(weekday_index, enabled_daytime_slot());

            let enabled_start_scenario = format!("{weekday_key} enabled start time");
            let messages = collect_config_change_diagnostic_messages(
                true,
                enabled_config.clone(),
                PostBreakTransitionPolicy::RequireManualContinue,
                true,
                weekly_config_with_weekday_slot(weekday_index, day_slot(true, 10, 18)),
                PostBreakTransitionPolicy::RequireManualContinue,
                1,
            )
            .await;
            assert_config_change_diagnostic_count(&enabled_start_scenario, &messages, 1);

            let enabled_end_scenario = format!("{weekday_key} enabled end time");
            let messages = collect_config_change_diagnostic_messages(
                true,
                enabled_config,
                PostBreakTransitionPolicy::RequireManualContinue,
                true,
                weekly_config_with_weekday_slot(weekday_index, day_slot(true, 9, 17)),
                PostBreakTransitionPolicy::RequireManualContinue,
                1,
            )
            .await;
            assert_config_change_diagnostic_count(&enabled_end_scenario, &messages, 1);

            let disabled_start_scenario = format!("{weekday_key} disabled start time");
            let messages = collect_config_change_diagnostic_messages(
                true,
                unchanged_config.clone(),
                PostBreakTransitionPolicy::RequireManualContinue,
                true,
                weekly_config_with_weekday_slot(weekday_index, day_slot(false, 10, 18)),
                PostBreakTransitionPolicy::RequireManualContinue,
                1,
            )
            .await;
            assert_config_change_diagnostic_count(&disabled_start_scenario, &messages, 1);

            let disabled_end_scenario = format!("{weekday_key} disabled end time");
            let messages = collect_config_change_diagnostic_messages(
                true,
                unchanged_config.clone(),
                PostBreakTransitionPolicy::RequireManualContinue,
                true,
                weekly_config_with_weekday_slot(weekday_index, day_slot(false, 9, 17)),
                PostBreakTransitionPolicy::RequireManualContinue,
                1,
            )
            .await;
            assert_config_change_diagnostic_count(&disabled_end_scenario, &messages, 1);
        }

        let messages = collect_config_change_diagnostic_messages(
            true,
            unchanged_config.clone(),
            PostBreakTransitionPolicy::RequireManualContinue,
            true,
            unchanged_config.clone(),
            PostBreakTransitionPolicy::StartNextWorkAutomatically,
            1,
        )
        .await;
        assert_config_change_diagnostic_count("post-break policy only", &messages, 0);

        let messages = collect_config_change_diagnostic_messages(
            true,
            unchanged_config.clone(),
            PostBreakTransitionPolicy::RequireManualContinue,
            true,
            unchanged_config,
            PostBreakTransitionPolicy::RequireManualContinue,
            1,
        )
        .await;
        assert_config_change_diagnostic_count("unchanged synchronization", &messages, 0);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn config_change_diagnostic_for_single_publication_is_not_reemitted() {
        let message_batches = collect_config_change_diagnostic_message_batches(
            false,
            WeeklyAutoRunConfig::default(),
            PostBreakTransitionPolicy::RequireManualContinue,
            true,
            WeeklyAutoRunConfig::default(),
            PostBreakTransitionPolicy::RequireManualContinue,
            2,
        )
        .await;

        assert_config_change_diagnostic_count(
            "single publication first evaluation",
            &message_batches[0],
            1,
        );
        assert_config_change_diagnostic_count(
            "single publication unchanged evaluation",
            &message_batches[1],
            0,
        );
    }

    #[tokio::test(flavor = "current_thread")]
    async fn config_change_diagnostic_is_aggregated_until_the_next_scheduler_evaluation() {
        let baseline_config = weekly_config_with_monday_daytime();
        let changed_config = weekly_config_with_monday_and_tuesday_daytime();
        let state = build_test_state_without_persistence_path().await;
        apply_schedule(
            &state,
            baseline_config.clone(),
            PostBreakTransitionPolicy::RequireManualContinue,
        )
        .await;
        let mut runtime_state =
            runtime_state_at(&state, local_datetime(2025, 2, 1, 11, 59, 50)).await;
        let mut effects = RecordingSchedulerRuntimeEffects::default();

        apply_schedule(
            &state,
            changed_config,
            PostBreakTransitionPolicy::RequireManualContinue,
        )
        .await;
        apply_schedule(
            &state,
            baseline_config,
            PostBreakTransitionPolicy::RequireManualContinue,
        )
        .await;

        begin_log_capture();
        run_scheduler_tick_with_effects(
            &state,
            &mut runtime_state,
            local_datetime(2025, 2, 1, 12, 0, 0),
            &mut effects,
        )
        .await;
        let first_evaluation_messages = finish_log_capture();

        begin_log_capture();
        run_scheduler_tick_with_effects(
            &state,
            &mut runtime_state,
            local_datetime(2025, 2, 1, 12, 0, 1),
            &mut effects,
        )
        .await;
        let unchanged_evaluation_messages = finish_log_capture();

        assert_config_change_diagnostic_count(
            "return to original schedule first evaluation",
            &first_evaluation_messages,
            1,
        );
        assert_config_change_diagnostic_count(
            "return to original schedule unchanged evaluation",
            &unchanged_evaluation_messages,
            0,
        );
    }

    #[tokio::test(flavor = "current_thread")]
    async fn config_change_diagnostic_survives_configuration_persistence_failure() {
        let state = build_test_state_with_persistence_failure().await;
        let baseline_config = weekly_config_with_monday_daytime();
        let changed_config = weekly_config_with_monday_and_tuesday_daytime();
        apply_schedule(
            &state,
            baseline_config,
            PostBreakTransitionPolicy::RequireManualContinue,
        )
        .await;

        let mut runtime_state =
            runtime_state_at(&state, local_datetime(2025, 2, 1, 11, 59, 50)).await;
        let mut command_effects = RecordingSchedulerRuntimeEffects::default();
        let sync_result = sync_automation_policies_with_runtime_effects(
            &state,
            true,
            changed_config,
            PostBreakTransitionPolicy::RequireManualContinue,
            &mut command_effects,
        )
        .await;

        assert!(
            sync_result.is_err(),
            "configuration synchronization should report its persistence failure"
        );

        let mut scheduler_effects = RecordingSchedulerRuntimeEffects::default();
        begin_log_capture();
        run_scheduler_tick_with_effects(
            &state,
            &mut runtime_state,
            local_datetime(2025, 2, 1, 12, 0, 0),
            &mut scheduler_effects,
        )
        .await;
        let messages = finish_log_capture();

        assert_config_change_diagnostic_count("configuration persistence failure", &messages, 1);
    }

    #[test]
    fn config_change_diagnostic_catalog_remains_stable() {
        let event = BackendLogEvent::SchedulerConfigRevisionChanged;

        assert_eq!(event.event_id(), "scheduler.config_revision_changed");
        assert_eq!(event.severity(), log::Level::Info);
        assert_eq!(
            event.spec().message_template,
            "Weekly scheduler detected config revision change"
        );
        assert_eq!(
            event
                .allowed_fields()
                .iter()
                .map(|field| (field.name, field.redaction_classification))
                .collect::<Vec<_>>(),
            [("config_revision", RedactionClass::Public)]
        );
    }
}
