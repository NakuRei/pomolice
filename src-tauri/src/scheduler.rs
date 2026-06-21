use chrono::{DateTime, Duration as ChronoDuration, Local};
use std::collections::HashSet;
use std::future::Future;
use std::pin::Pin;
use std::sync::atomic::Ordering;
use std::time::Duration;
use tauri::{AppHandle, Emitter, Manager};
use tokio::time::MissedTickBehavior;
use tokio_util::sync::CancellationToken;

use crate::logging_event::{
    log_info_event, log_persistence_write_failure, log_warn_event, BackendLogEvent,
    PERSISTENCE_WRITE_TRIGGER_SCHEDULER_AUTO_START,
    PERSISTENCE_WRITE_TRIGGER_SCHEDULER_STOP_PENDING,
};
use crate::managers::weekly_schedule::{
    collect_crossed_end_boundaries, collect_crossed_start_boundaries,
};
use crate::state;
use crate::tray;
use crate::types::{
    BootPromptRuntimeState, BoundaryOccurrenceId, TimerSettings, TimerStartSource, TimerStatus,
    WeeklyAutoRunConfig,
};

const SCHEDULER_INTERVAL_SECONDS: u64 = 1;
const PROCESSED_BOUNDARY_RETENTION_DAYS: i64 = 8;

struct SchedulerRuntimeState {
    processed_boundary_occurrences: HashSet<BoundaryOccurrenceId>,
    last_checked_local: DateTime<Local>,
    start_boundaries_blocked_until_idle: bool,
    last_seen_config_revision: u64,
}

struct SchedulerTimerSnapshot {
    status: TimerStatus,
    auto_run_feature_enabled: bool,
    weekly_auto_run_config: WeeklyAutoRunConfig,
    timer_settings: TimerSettings,
}

struct SchedulerTickPlan {
    end_boundary_previous_local: DateTime<Local>,
    should_evaluate_start_boundaries: bool,
}

trait SchedulerRuntimeEffects {
    fn emit_timer_tick(&mut self, tick_payload: &crate::types::TimerTickPayload);
    fn update_tray(&mut self, tick_payload: &crate::types::TimerTickPayload);
    fn emit_work_started(&mut self, work_duration_secs: u64);
    fn emit_auto_run_stop_pending(&mut self);
    fn spawn_timer_task<'a>(&'a mut self) -> Pin<Box<dyn Future<Output = ()> + Send + 'a>>;
    fn persist_current_state_with_trigger<'a>(
        &'a mut self,
        trigger: &'static str,
    ) -> Pin<Box<dyn Future<Output = ()> + Send + 'a>>;
}

struct AppHandleSchedulerRuntimeEffects<'a> {
    app: &'a AppHandle,
}

impl SchedulerRuntimeEffects for AppHandleSchedulerRuntimeEffects<'_> {
    fn emit_timer_tick(&mut self, tick_payload: &crate::types::TimerTickPayload) {
        let _ = self.app.emit("timer_tick", tick_payload);
    }

    fn update_tray(&mut self, tick_payload: &crate::types::TimerTickPayload) {
        tray::update_tray_icon(self.app, tick_payload);
    }

    fn emit_work_started(&mut self, work_duration_secs: u64) {
        let _ = self.app.emit(
            "work_started",
            serde_json::json!({ "duration": work_duration_secs }),
        );
    }

    fn emit_auto_run_stop_pending(&mut self) {
        let _ = self
            .app
            .emit("auto_run_stop_pending", serde_json::json!({}));
    }

    fn spawn_timer_task<'a>(&'a mut self) -> Pin<Box<dyn Future<Output = ()> + Send + 'a>> {
        Box::pin(async move {
            crate::session_pipeline::spawn_timer_task(self.app).await;
        })
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

#[cfg(test)]
pub(crate) struct SchedulerTickTestRuntime {
    runtime_state: SchedulerRuntimeState,
}

#[cfg(test)]
impl SchedulerTickTestRuntime {
    pub(crate) fn from_state(
        state_handle: &state::AppState,
        previous_local: DateTime<Local>,
    ) -> Self {
        Self::new(
            previous_local,
            state_handle.config_revision.load(Ordering::Acquire),
        )
    }

    fn new(previous_local: DateTime<Local>, last_seen_config_revision: u64) -> Self {
        Self {
            runtime_state: SchedulerRuntimeState {
                processed_boundary_occurrences: HashSet::new(),
                last_checked_local: previous_local,
                start_boundaries_blocked_until_idle: false,
                last_seen_config_revision,
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
    fn emit_timer_tick(&mut self, _tick_payload: &crate::types::TimerTickPayload) {}

    fn update_tray(&mut self, _tick_payload: &crate::types::TimerTickPayload) {}

    fn emit_work_started(&mut self, _work_duration_secs: u64) {}

    fn emit_auto_run_stop_pending(&mut self) {}

    fn spawn_timer_task<'a>(&'a mut self) -> Pin<Box<dyn Future<Output = ()> + Send + 'a>> {
        Box::pin(async {})
    }

    fn persist_current_state_with_trigger<'a>(
        &'a mut self,
        _trigger: &'static str,
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
    let mut runtime_state = SchedulerRuntimeState {
        processed_boundary_occurrences: HashSet::new(),
        last_checked_local: initial_previous_local,
        start_boundaries_blocked_until_idle,
        last_seen_config_revision: state_handle.config_revision.load(Ordering::Acquire),
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

async fn run_scheduler_tick_with_effects(
    state_handle: &state::AppState,
    runtime_state: &mut SchedulerRuntimeState,
    now_local: DateTime<Local>,
    effects: &mut impl SchedulerRuntimeEffects,
) {
    let previous_local = runtime_state.last_checked_local;

    let timer_snapshot = load_scheduler_timer_snapshot(state_handle).await;
    let resumed_start_boundary_evaluation =
        resume_start_boundary_evaluation_when_idle(runtime_state, timer_snapshot.status);

    let tick_plan = build_scheduler_tick_plan(
        state_handle,
        runtime_state,
        now_local,
        timer_snapshot.auto_run_feature_enabled,
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

    trim_old_processed_boundaries(&mut runtime_state.processed_boundary_occurrences, now_local);
    runtime_state.last_checked_local = now_local;
}

async fn load_scheduler_timer_snapshot(state_handle: &state::AppState) -> SchedulerTimerSnapshot {
    let timer_manager = state_handle.timer_manager.lock().await;
    let timer_state = timer_manager.get_state();

    SchedulerTimerSnapshot {
        status: timer_manager.status(),
        auto_run_feature_enabled: timer_state.auto_run_feature_enabled,
        weekly_auto_run_config: timer_state.weekly_auto_run_config.clone(),
        timer_settings: timer_state.settings.clone(),
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
    auto_run_feature_enabled: bool,
    resumed_start_boundary_evaluation: bool,
) -> SchedulerTickPlan {
    let config_revision = state_handle.config_revision.load(Ordering::Acquire);
    let config_changed = config_revision != runtime_state.last_seen_config_revision;
    let mut skip_start_boundaries_this_tick = false;

    let end_boundary_previous_local = if config_changed {
        runtime_state.processed_boundary_occurrences.clear();
        runtime_state.last_checked_local = now_local;
        runtime_state.last_seen_config_revision = config_revision;
        skip_start_boundaries_this_tick = true;
        log_info_event(
            BackendLogEvent::SchedulerConfigRevisionChanged,
            &[("config_revision", config_revision.to_string())],
        );
        now_local - ChronoDuration::seconds(1)
    } else {
        runtime_state.last_checked_local
    };

    let should_evaluate_start_boundaries = auto_run_feature_enabled
        && boot_prompt_gate_allows_start_boundary_evaluation(
            state_handle
                .auto_start_boundary_enabled
                .load(Ordering::Acquire),
            state_handle
                .boot_prompt_check_pending
                .load(Ordering::Acquire),
            current_boot_prompt_runtime_state(state_handle),
        )
        && !runtime_state.start_boundaries_blocked_until_idle
        && !resumed_start_boundary_evaluation
        && !skip_start_boundaries_this_tick;

    SchedulerTickPlan {
        end_boundary_previous_local,
        should_evaluate_start_boundaries,
    }
}

fn current_boot_prompt_runtime_state(state_handle: &state::AppState) -> BootPromptRuntimeState {
    match state_handle.boot_prompt_runtime_state.lock() {
        Ok(guard) => *guard,
        Err(_) => BootPromptRuntimeState::NotApplicable,
    }
}

fn boot_prompt_gate_allows_start_boundary_evaluation(
    auto_start_boundary_enabled: bool,
    boot_prompt_check_pending: bool,
    boot_prompt_runtime_state: BootPromptRuntimeState,
) -> bool {
    auto_start_boundary_enabled
        && !boot_prompt_check_pending
        && boot_prompt_runtime_state != BootPromptRuntimeState::AwaitingDecision
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
        if !record_processed_boundary_occurrence(runtime_state, &end_boundary) {
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

async fn process_crossed_start_boundaries_with_effects(
    state_handle: &state::AppState,
    runtime_state: &mut SchedulerRuntimeState,
    timer_snapshot: &SchedulerTimerSnapshot,
    previous_local: DateTime<Local>,
    now_local: DateTime<Local>,
    effects: &mut impl SchedulerRuntimeEffects,
) {
    let crossed_start_boundaries = collect_crossed_start_boundaries(
        previous_local,
        now_local,
        &timer_snapshot.weekly_auto_run_config,
    );

    for start_boundary in crossed_start_boundaries {
        if !record_processed_boundary_occurrence(runtime_state, &start_boundary) {
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

fn record_processed_boundary_occurrence(
    runtime_state: &mut SchedulerRuntimeState,
    boundary: &BoundaryOccurrenceId,
) -> bool {
    runtime_state
        .processed_boundary_occurrences
        .insert(boundary.clone())
}

pub(crate) fn trim_old_processed_boundaries(
    processed_boundary_occurrences: &mut HashSet<BoundaryOccurrenceId>,
    now_local: DateTime<Local>,
) {
    let oldest_epoch_to_keep =
        (now_local - ChronoDuration::days(PROCESSED_BOUNDARY_RETENTION_DAYS)).timestamp();

    processed_boundary_occurrences.retain(|boundary_id| {
        boundary_id
            .epoch_seconds()
            .is_some_and(|epoch_seconds| epoch_seconds >= oldest_epoch_to_keep)
    });
}

async fn mark_scheduled_stop_pending_with_effects(
    state_handle: &state::AppState,
    effects: &mut impl SchedulerRuntimeEffects,
) -> bool {
    let should_persist = {
        let mut timer_manager = state_handle.timer_manager.lock().await;
        timer_manager.mark_scheduled_stop_pending_if_session_is_eligible()
    };

    if should_persist {
        effects.emit_auto_run_stop_pending();
        effects
            .persist_current_state_with_trigger(PERSISTENCE_WRITE_TRIGGER_SCHEDULER_STOP_PENDING)
            .await;
    }

    should_persist
}

async fn start_work_from_scheduler_when_idle_with_effects(
    state_handle: &state::AppState,
    timer_status_at_tick_start: TimerStatus,
    timer_settings: TimerSettings,
    start_boundary: &BoundaryOccurrenceId,
    effects: &mut impl SchedulerRuntimeEffects,
) -> bool {
    if timer_status_at_tick_start != TimerStatus::Idle {
        return false;
    }

    let (old_timer_task, tick_payload, work_duration_secs) = {
        let mut timer_manager = state_handle.timer_manager.lock().await;
        if timer_manager.status() != TimerStatus::Idle {
            return false;
        }

        let old_task = timer_manager.cancel_and_take_task();
        let Ok((payload, duration_secs)) = timer_manager
            .start_work_with_source(timer_settings, TimerStartSource::ScheduledAutoStart)
        else {
            log_warn_event(
                BackendLogEvent::SchedulerSkippedAutoStartDueToRejection,
                &[("boundary_id", start_boundary.as_str().to_string())],
            );
            return false;
        };
        (old_task, payload, duration_secs)
    };

    effects.emit_timer_tick(&tick_payload);
    effects.update_tray(&tick_payload);
    effects.emit_work_started(work_duration_secs);

    if let Some(join_handle) = old_timer_task {
        let _ = join_handle.await;
    }

    effects.spawn_timer_task().await;

    effects
        .persist_current_state_with_trigger(PERSISTENCE_WRITE_TRIGGER_SCHEDULER_AUTO_START)
        .await;

    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::logging_event::{
        log_persistence_write_failure, PERSISTENCE_WRITE_TRIGGER_SCHEDULER_AUTO_START,
        PERSISTENCE_WRITE_TRIGGER_SCHEDULER_STOP_PENDING,
    };
    use crate::managers::clock::testing::FixedClock;
    use crate::managers::clock::Clock;
    use crate::managers::persistence::PersistenceManager;
    use crate::types::{
        DayAutoRunSlot, HhMm, PostBreakTransitionPolicy, ScheduledStopState,
        SessionTransitionReason, SessionType, TimerStartSource, TimerTickPayload,
        WeeklyAutoRunConfig,
    };
    use chrono::{LocalResult, TimeZone, Utc};
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;

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

    impl SchedulerRuntimeEffects for RecordingSchedulerRuntimeEffects {
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

        fn emit_auto_run_stop_pending(&mut self) {
            self.auto_run_stop_pending_count += 1;
        }

        fn spawn_timer_task<'a>(&'a mut self) -> Pin<Box<dyn Future<Output = ()> + Send + 'a>> {
            self.timer_task_spawn_count += 1;
            Box::pin(async {})
        }

        fn persist_current_state_with_trigger<'a>(
            &'a mut self,
            trigger: &'static str,
        ) -> Pin<Box<dyn Future<Output = ()> + Send + 'a>> {
            self.persist_triggers.push(trigger);
            if self.simulate_persistence_failure {
                self.logged_persistence_failure_count += 1;
                let error = std::io::Error::other("simulated scheduler persistence failure");
                log_persistence_write_failure(trigger, &error);
            }
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

    async fn build_test_state_without_persistence_path() -> state::AppState {
        let clock = fixed_clock(create_utc_datetime(2025, 2, 1, 9, 0, 0));
        let persistence = PersistenceManager::new(as_clock_trait(&clock), None);
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

    #[test]
    fn test_boot_prompt_gate_allows_resolved_start_after_prompt_check_completed() {
        assert!(boot_prompt_gate_allows_start_boundary_evaluation(
            true,
            false,
            BootPromptRuntimeState::ResolvedStart,
        ));
    }

    #[test]
    fn test_boot_prompt_gate_allows_resolved_later_after_prompt_check_completed() {
        assert!(boot_prompt_gate_allows_start_boundary_evaluation(
            true,
            false,
            BootPromptRuntimeState::ResolvedLater,
        ));
    }

    #[test]
    fn test_boot_prompt_gate_allows_not_applicable_after_prompt_check_completed() {
        assert!(boot_prompt_gate_allows_start_boundary_evaluation(
            true,
            false,
            BootPromptRuntimeState::NotApplicable,
        ));
    }

    #[test]
    fn test_boot_prompt_gate_blocks_when_auto_start_boundary_disabled() {
        assert!(!boot_prompt_gate_allows_start_boundary_evaluation(
            false,
            false,
            BootPromptRuntimeState::ResolvedStart,
        ));
    }

    #[test]
    fn test_boot_prompt_gate_blocks_while_prompt_check_pending() {
        assert!(!boot_prompt_gate_allows_start_boundary_evaluation(
            true,
            true,
            BootPromptRuntimeState::ResolvedStart,
        ));
    }

    #[test]
    fn test_boot_prompt_gate_blocks_awaiting_decision() {
        assert!(!boot_prompt_gate_allows_start_boundary_evaluation(
            true,
            false,
            BootPromptRuntimeState::AwaitingDecision,
        ));
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
        let timer_state = timer.get_state();
        assert_eq!(timer.status(), TimerStatus::Working);
        assert_eq!(
            timer_state.timer_start_source,
            Some(TimerStartSource::ScheduledAutoStart)
        );
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
                .start_work_with_source(
                    TimerSettings::default(),
                    TimerStartSource::ManualInsideWindow,
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

        let timer = state.timer_manager.lock().await;
        assert_eq!(
            timer.get_state().scheduled_stop_state,
            ScheduledStopState::Pending
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

    /// Opens the boot-prompt and auto-start gates so that start boundaries are
    /// actually evaluated during a tick.
    fn open_start_boundary_gate(state: &state::AppState) {
        state
            .auto_start_boundary_enabled
            .store(true, Ordering::Release);
        state
            .boot_prompt_check_pending
            .store(false, Ordering::Release);
        *state
            .boot_prompt_runtime_state
            .lock()
            .expect("boot prompt runtime state mutex should not be poisoned") =
            BootPromptRuntimeState::NotApplicable;
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

    fn runtime_state_at(
        previous_local: DateTime<Local>,
        last_seen_config_revision: u64,
    ) -> SchedulerRuntimeState {
        SchedulerRuntimeState {
            processed_boundary_occurrences: HashSet::new(),
            last_checked_local: previous_local,
            start_boundaries_blocked_until_idle: false,
            last_seen_config_revision,
        }
    }

    fn runtime_state_blocking_start_boundaries_at(
        previous_local: DateTime<Local>,
        last_seen_config_revision: u64,
    ) -> SchedulerRuntimeState {
        SchedulerRuntimeState {
            processed_boundary_occurrences: HashSet::new(),
            last_checked_local: previous_local,
            start_boundaries_blocked_until_idle: true,
            last_seen_config_revision,
        }
    }

    async fn start_work_with_schedule_source(
        state: &state::AppState,
        auto_run_feature_enabled: bool,
        timer_start_source: TimerStartSource,
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
            .start_work_with_source(TimerSettings::default(), timer_start_source)
            .expect("work session should start");
    }

    fn close_start_boundary_gate(state: &state::AppState) {
        state
            .auto_start_boundary_enabled
            .store(false, Ordering::Release);
        state
            .boot_prompt_check_pending
            .store(true, Ordering::Release);
        *state
            .boot_prompt_runtime_state
            .lock()
            .expect("boot prompt runtime state mutex should not be poisoned") =
            BootPromptRuntimeState::AwaitingDecision;
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

    #[tokio::test]
    async fn end_boundary_emits_stop_pending_for_auto_run_session() {
        let state = build_test_state_without_persistence_path().await;
        close_start_boundary_gate(&state);
        start_work_with_schedule_source(&state, true, TimerStartSource::ManualInsideWindow).await;

        let mut runtime_state = runtime_state_at(
            local_datetime(2025, 1, 6, 17, 59, 50),
            state.config_revision.load(Ordering::Acquire),
        );
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
        start_work_with_schedule_source(&state, true, TimerStartSource::ManualInsideWindow).await;

        let mut runtime_state = runtime_state_at(
            local_datetime(2025, 1, 6, 17, 59, 50),
            state.config_revision.load(Ordering::Acquire),
        );
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
    async fn end_boundary_does_not_emit_stop_pending_when_tick_time_does_not_advance() {
        let state = build_test_state_without_persistence_path().await;
        start_work_with_schedule_source(&state, true, TimerStartSource::ManualInsideWindow).await;

        let mut runtime_state = runtime_state_at(
            local_datetime(2025, 1, 6, 18, 0, 0),
            state.config_revision.load(Ordering::Acquire),
        );
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
        start_work_with_schedule_source(&state, true, TimerStartSource::ManualInsideWindow).await;

        let mut runtime_state = runtime_state_at(
            local_datetime(2025, 1, 6, 17, 59, 50),
            state.config_revision.load(Ordering::Acquire),
        );
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
        start_work_with_schedule_source(&state, false, TimerStartSource::ManualInsideWindow).await;

        let mut runtime_state = runtime_state_at(
            local_datetime(2025, 1, 6, 17, 59, 50),
            state.config_revision.load(Ordering::Acquire),
        );
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
        start_work_with_schedule_source(&state, true, TimerStartSource::ManualOutsideWindow).await;

        let mut runtime_state = runtime_state_at(
            local_datetime(2025, 1, 6, 17, 59, 50),
            state.config_revision.load(Ordering::Acquire),
        );
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
        open_start_boundary_gate(&state);
        apply_schedule(
            &state,
            weekly_config_with_monday_daytime(),
            PostBreakTransitionPolicy::RequireManualContinue,
        )
        .await;

        let mut runtime_state = runtime_state_at(
            local_datetime(2025, 1, 6, 8, 59, 50),
            state.config_revision.load(Ordering::Acquire),
        );
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
    async fn restored_running_session_does_not_replay_past_start_boundary_after_returning_idle() {
        let state = build_test_state_without_persistence_path().await;
        open_start_boundary_gate(&state);
        apply_schedule(
            &state,
            weekly_config_with_monday_and_tuesday_daytime(),
            PostBreakTransitionPolicy::RequireManualContinue,
        )
        .await;

        let mut runtime_state = runtime_state_blocking_start_boundaries_at(
            local_datetime(2025, 1, 6, 8, 59, 50),
            state.config_revision.load(Ordering::Acquire),
        );
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
            assert_ne!(
                timer.get_state().timer_start_source,
                Some(TimerStartSource::ScheduledAutoStart)
            );
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
        assert_eq!(
            timer.get_state().timer_start_source,
            Some(TimerStartSource::ScheduledAutoStart)
        );
    }

    #[tokio::test]
    async fn policy_only_change_keeps_scheduled_auto_start_firing() {
        let state = build_test_state_without_persistence_path().await;
        open_start_boundary_gate(&state);
        apply_schedule(
            &state,
            weekly_config_with_monday_daytime(),
            PostBreakTransitionPolicy::RequireManualContinue,
        )
        .await;

        // A post-break policy change leaves the schedule topology untouched,
        // so it does not invalidate the scheduler baseline.
        apply_schedule(
            &state,
            weekly_config_with_monday_daytime(),
            PostBreakTransitionPolicy::StartNextWorkAutomatically,
        )
        .await;

        let mut runtime_state = runtime_state_at(
            local_datetime(2025, 1, 6, 8, 59, 50),
            state.config_revision.load(Ordering::Acquire),
        );
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
            "a policy-only change must not suppress an arriving scheduled auto-start"
        );
    }

    #[tokio::test]
    async fn topology_change_suppresses_retroactive_start_then_fires_on_next_boundary() {
        let state = build_test_state_without_persistence_path().await;
        open_start_boundary_gate(&state);
        apply_schedule(
            &state,
            weekly_config_with_monday_and_tuesday_daytime(),
            PostBreakTransitionPolicy::RequireManualContinue,
        )
        .await;

        // A schedule (topology) edit bumps the scheduler config revision, so the
        // runtime state lags one revision behind at the first post-edit tick.
        let revision_before_edit = state.config_revision.load(Ordering::Acquire);
        state.config_revision.fetch_add(1, Ordering::AcqRel);

        // At edit time it is already past Monday's 09:00 boundary.
        let mut runtime_state =
            runtime_state_at(local_datetime(2025, 1, 6, 9, 59, 50), revision_before_edit);
        let mut effects = RecordingSchedulerRuntimeEffects::default();

        // First post-edit tick: the invalidation rebaselines the scheduler.
        run_scheduler_tick_with_effects(
            &state,
            &mut runtime_state,
            local_datetime(2025, 1, 6, 10, 0, 0),
            &mut effects,
        )
        .await;

        // Subsequent tick that does not re-cross any boundary: Monday's already
        // past 09:00 boundary must not be replayed retroactively.
        run_scheduler_tick_with_effects(
            &state,
            &mut runtime_state,
            local_datetime(2025, 1, 6, 10, 0, 30),
            &mut effects,
        )
        .await;

        assert!(
            effects.work_started_durations.is_empty(),
            "a boundary already in the past at edit time must not fire retroactively"
        );

        // A legitimate boundary that arrives after the edit is honored.
        run_scheduler_tick_with_effects(
            &state,
            &mut runtime_state,
            local_datetime(2025, 1, 7, 9, 0, 10),
            &mut effects,
        )
        .await;

        assert!(
            !effects.work_started_durations.is_empty(),
            "a boundary arriving after the edit must auto-start under the new schedule"
        );
    }
}
