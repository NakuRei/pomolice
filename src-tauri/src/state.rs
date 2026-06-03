// Application state management
// Central hub for all state, managed via Tauri's State mechanism

use chrono::{DateTime, Duration as ChronoDuration, Local, Utc};
use std::collections::BTreeSet;
use std::future::Future;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Condvar, Mutex as StdMutex};
use tauri::async_runtime::JoinHandle;
use tokio::sync::{Mutex, Notify, Semaphore};
use tokio_util::sync::CancellationToken;

use crate::error::{PomoliceError, Result as AppResult};
use crate::logging_event::{log_error_event, log_info_event, BackendLogEvent};
use crate::managers::clock::Clock;
use crate::managers::input_monitor::InputMonitorManager;
use crate::managers::persistence::{PersistenceLoadOutcome, PersistenceManager, SaveError};
use crate::managers::timer::TimerStateManager;
use crate::managers::window::WindowStateManager;
use crate::types::{
    BootPromptRuntimeState, SessionTransitionOutcome, TimerState, TimerStatus, TimerTickPayload,
};

/// Lifecycle handles for the weekly scheduler background task.
#[derive(Default)]
pub(crate) struct SchedulerTaskState {
    pub cancellation_token: Option<CancellationToken>,
    pub join_handle: Option<JoinHandle<()>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum AppRestoreStatus {
    NoState,
    Restored,
    UnsupportedSchemaFallback,
}

#[derive(Debug, Clone)]
pub(crate) struct AppInitializationOutcome {
    pub(crate) initial_tick_payload: Option<TimerTickPayload>,
    pub(crate) restore_status: AppRestoreStatus,
    pub(crate) startup_action: StartupRestoreAction,
}

#[derive(Debug, Clone)]
pub(crate) enum StartupRestoreAction {
    None,
    RestartRestoredTimerTask,
    FinalizeExpiredRestoredWork(SessionTransitionOutcome),
}

struct StartupPreparation {
    initial_timer_state: TimerState,
    restored_running_saved_at_local: Option<DateTime<Local>>,
    initialization_outcome: AppInitializationOutcome,
}

#[cfg(test)]
type RuntimeSaveSnapshotReadyHook = Arc<dyn Fn() + Send + Sync>;

#[derive(Default)]
struct OrderedSaveLaneState {
    next_to_run: u64,
    cancelled_tickets: BTreeSet<u64>,
}

struct OrderedSaveLaneInner {
    next_ticket: AtomicU64,
    // Ticket bookkeeping is synchronous and never held across await; async and blocking callers
    // only wait after releasing this guard.
    state: StdMutex<OrderedSaveLaneState>,
    // Blocking exit flush and async runtime saves wait on the same lane, so we need both a
    // Condvar for sync callers and a Notify for async callers to observe head advancement.
    condvar: Condvar,
    notify: Notify,
}

impl Default for OrderedSaveLaneInner {
    fn default() -> Self {
        Self {
            next_ticket: AtomicU64::new(0),
            state: StdMutex::new(OrderedSaveLaneState::default()),
            condvar: Condvar::new(),
            notify: Notify::new(),
        }
    }
}

#[derive(Clone, Default)]
struct OrderedSaveLane {
    inner: Arc<OrderedSaveLaneInner>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum OrderedSaveTicketState {
    Pending,
    Executing,
}

struct OrderedSaveTicket {
    lane: OrderedSaveLane,
    ticket: u64,
    state: OrderedSaveTicketState,
}

impl OrderedSaveLane {
    fn issue_ticket(&self) -> OrderedSaveTicket {
        OrderedSaveTicket {
            lane: self.clone(),
            ticket: self.inner.next_ticket.fetch_add(1, Ordering::AcqRel),
            state: OrderedSaveTicketState::Pending,
        }
    }

    async fn wait_turn_async(&self, ticket: u64) {
        loop {
            let notified = self.inner.notify.notified();

            if self.is_turn(ticket) {
                return;
            }

            notified.await;
        }
    }

    fn wait_turn_blocking(&self, ticket: u64) {
        let mut state = self
            .inner
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);

        while state.next_to_run != ticket {
            state = self
                .inner
                .condvar
                .wait(state)
                .unwrap_or_else(std::sync::PoisonError::into_inner);
        }
    }

    fn is_turn(&self, ticket: u64) -> bool {
        self.inner
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .next_to_run
            == ticket
    }

    fn cancel_ticket(&self, ticket: u64) {
        let mut state = self
            .inner
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);

        if ticket < state.next_to_run {
            return;
        }

        let previous_head = state.next_to_run;
        state.cancelled_tickets.insert(ticket);
        Self::advance_past_cancelled_tickets(&mut state);

        let should_notify = state.next_to_run != previous_head;
        drop(state);

        if should_notify {
            self.notify_waiters();
        }
    }

    fn advance_past_cancelled_tickets(state: &mut OrderedSaveLaneState) {
        while state.cancelled_tickets.remove(&state.next_to_run) {
            state.next_to_run = state.next_to_run.saturating_add(1);
        }
    }

    fn advance(&self) {
        let mut state = self
            .inner
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        state.next_to_run = state.next_to_run.saturating_add(1);
        Self::advance_past_cancelled_tickets(&mut state);
        drop(state);

        self.notify_waiters();
    }

    fn notify_waiters(&self) {
        self.inner.condvar.notify_all();
        self.inner.notify.notify_waiters();
    }
}

impl OrderedSaveTicket {
    async fn wait_turn_async(&self) {
        self.lane.wait_turn_async(self.ticket).await;
    }

    fn wait_turn_blocking(&self) {
        self.lane.wait_turn_blocking(self.ticket);
    }

    fn mark_executing(&mut self) {
        self.state = OrderedSaveTicketState::Executing;
    }
}

impl Drop for OrderedSaveTicket {
    // Pending tickets release the lane by marking themselves cancelled so a later head advance can
    // skip them. Executing tickets release the lane immediately on completion or unwind.
    fn drop(&mut self) {
        match self.state {
            OrderedSaveTicketState::Pending => self.lane.cancel_ticket(self.ticket),
            OrderedSaveTicketState::Executing => self.lane.advance(),
        }
    }
}

/// Central application state.
pub(crate) struct AppState {
    // Async command/session paths mutate these managers from Tokio tasks. Clippy rejects held
    // Tokio guards across await so manager critical sections stay locally scoped.
    pub timer_manager: Mutex<TimerStateManager>,
    pub input_monitor: Mutex<InputMonitorManager>,
    pub window_manager: Mutex<WindowStateManager>,
    pub persistence: PersistenceManager,
    pub clock: Arc<dyn Clock + Send + Sync>,
    // These runtime fields are synchronous, short-lived bookkeeping and are not held with await.
    pub scheduler_task_state: StdMutex<SchedulerTaskState>,
    pub boot_prompt_runtime_state: StdMutex<BootPromptRuntimeState>,
    pub boot_prompt_check_pending: AtomicBool,
    pub auto_start_boundary_enabled: AtomicBool,
    restored_running_saved_at_local: StdMutex<Option<DateTime<Local>>>,
    pub config_revision: AtomicU64,
    ordered_save_lane: OrderedSaveLane,
    #[cfg(test)]
    runtime_save_snapshot_ready_hook: Option<RuntimeSaveSnapshotReadyHook>,
    break_mode_transition_permit: Arc<Semaphore>,
}

impl AppState {
    #[cfg(test)]
    pub(crate) async fn build_startup_state_for_tests(
        clock: Arc<dyn Clock + Send + Sync>,
        persistence: PersistenceManager,
    ) -> (Self, AppInitializationOutcome) {
        Self::build_startup_state_with_handlers(clock, persistence, || {}, || {}).await
    }

    pub(crate) async fn build_startup_state_with_handlers(
        clock: Arc<dyn Clock + Send + Sync>,
        persistence: PersistenceManager,
        on_escape_combo: impl Fn() + Send + Sync + 'static,
        on_input_detected: impl Fn() + Send + Sync + 'static,
    ) -> (Self, AppInitializationOutcome) {
        let preparation = Self::prepare_startup(clock.as_ref(), &persistence).await;
        let app_state = Self::build_state(
            clock,
            persistence,
            preparation.initial_timer_state,
            preparation.restored_running_saved_at_local,
            on_escape_combo,
            on_input_detected,
        );
        (app_state, preparation.initialization_outcome)
    }

    fn build_state(
        clock: Arc<dyn Clock + Send + Sync>,
        persistence: PersistenceManager,
        initial_timer_state: TimerState,
        restored_running_saved_at_local: Option<DateTime<Local>>,
        on_escape_combo: impl Fn() + Send + Sync + 'static,
        on_input_detected: impl Fn() + Send + Sync + 'static,
    ) -> Self {
        log_info_event(BackendLogEvent::AppStateCreated, &[]);

        Self {
            timer_manager: Mutex::new(TimerStateManager::new(Some(initial_timer_state))),
            input_monitor: Mutex::new(InputMonitorManager::new(on_escape_combo, on_input_detected)),
            window_manager: Mutex::new(WindowStateManager::new()),
            persistence,
            clock,
            scheduler_task_state: StdMutex::new(SchedulerTaskState::default()),
            boot_prompt_runtime_state: StdMutex::new(BootPromptRuntimeState::NotApplicable),
            boot_prompt_check_pending: AtomicBool::new(true),
            auto_start_boundary_enabled: AtomicBool::new(false),
            restored_running_saved_at_local: StdMutex::new(restored_running_saved_at_local),
            config_revision: AtomicU64::new(0),
            ordered_save_lane: OrderedSaveLane::default(),
            #[cfg(test)]
            runtime_save_snapshot_ready_hook: None,
            break_mode_transition_permit: Arc::new(Semaphore::new(1)),
        }
    }

    #[cfg(test)]
    fn with_runtime_save_snapshot_ready_hook_for_tests(
        mut self,
        hook: RuntimeSaveSnapshotReadyHook,
    ) -> Self {
        self.runtime_save_snapshot_ready_hook = Some(hook);
        self
    }

    pub(crate) fn cancel_and_take_scheduler_task(&self) -> Option<JoinHandle<()>> {
        if let Ok(mut scheduler_task_state) = self.scheduler_task_state.lock() {
            if let Some(cancellation_token) = scheduler_task_state.cancellation_token.take() {
                cancellation_token.cancel();
            }
            return scheduler_task_state.join_handle.take();
        }

        None
    }

    pub(crate) async fn cancel_and_wait_scheduler_task(&self) {
        if let Some(join_handle) = self.cancel_and_take_scheduler_task() {
            let _ = join_handle.await;
        }
    }

    pub(crate) fn store_scheduler_task(
        &self,
        cancellation_token: CancellationToken,
        join_handle: JoinHandle<()>,
    ) {
        if let Ok(mut scheduler_task_state) = self.scheduler_task_state.lock() {
            scheduler_task_state.cancellation_token = Some(cancellation_token);
            scheduler_task_state.join_handle = Some(join_handle);
        }
    }

    pub(crate) async fn persist_current_state_async(&self) -> AppResult<()> {
        if !self.persistence.has_state_path() {
            let timer_state = self.snapshot_timer_state().await;
            return self
                .persistence
                .save_state_sync(timer_state)
                .map_err(PomoliceError::from);
        }

        let ticket = self.ordered_save_lane.issue_ticket();
        let timer_state = self.snapshot_timer_state().await;
        let persistence = self.persistence.clone();

        #[cfg(test)]
        if let Some(hook) = &self.runtime_save_snapshot_ready_hook {
            hook();
        }

        ticket.wait_turn_async().await;

        let save_result = tokio::task::spawn_blocking(move || {
            let mut ticket = ticket;
            ticket.mark_executing();
            persistence.save_state_sync(timer_state)
        })
        .await;

        match save_result {
            Ok(save_result) => save_result.map_err(PomoliceError::from),
            Err(join_error) => {
                log_error_event(
                    BackendLogEvent::StoragePersistenceWriteFailed,
                    &[],
                    Some(&join_error),
                );
                Err(PomoliceError::PersistenceTaskJoin { source: join_error })
            }
        }
    }

    /// Synchronously persists the current timer state during `RunEvent::Exit`.
    ///
    /// This helper is intentionally confined to the synchronous Tauri exit hook.
    /// Calling `blocking_lock()` from async commands or runtime tasks can panic.
    pub(crate) fn flush_sync_on_exit(&self) -> Result<(), SaveError> {
        if !self.persistence.has_state_path() {
            let timer_state = {
                let timer_manager = self.timer_manager.blocking_lock();
                timer_manager.get_state().clone()
            };

            return self.persistence.save_state_sync(timer_state);
        }

        let mut ticket = self.ordered_save_lane.issue_ticket();
        let timer_state = {
            let timer_manager = self.timer_manager.blocking_lock();
            timer_manager.get_state().clone()
        };

        ticket.wait_turn_blocking();
        ticket.mark_executing();

        self.persistence.save_state_sync(timer_state)
    }

    pub(crate) async fn run_break_mode_transition<T, Fut>(&self, transition: Fut) -> T
    where
        Fut: Future<Output = T>,
    {
        let Ok(_permit) = Arc::clone(&self.break_mode_transition_permit)
            .acquire_owned()
            .await
        else {
            panic!("break-mode transition permit should remain open");
        };

        transition.await
    }

    pub(crate) fn restored_running_saved_at_local(&self) -> Option<DateTime<Local>> {
        match self.restored_running_saved_at_local.lock() {
            Ok(saved_at) => *saved_at,
            Err(_) => None,
        }
    }

    async fn prepare_startup(
        clock: &dyn Clock,
        persistence: &PersistenceManager,
    ) -> StartupPreparation {
        log_info_event(BackendLogEvent::AppStateInitializationStarted, &[]);

        let preparation = match load_persisted_state_async(persistence).await {
            PersistenceLoadOutcome::NoState => {
                log_info_event(BackendLogEvent::AppStateUsingDefaults, &[]);
                StartupPreparation::idle(AppRestoreStatus::NoState)
            }
            PersistenceLoadOutcome::UnsupportedSchemaFallback => {
                log_info_event(BackendLogEvent::AppStateUsingDefaults, &[]);
                StartupPreparation::idle(AppRestoreStatus::UnsupportedSchemaFallback)
            }
            PersistenceLoadOutcome::Restored {
                timer_state,
                saved_at_utc,
            } => {
                log_info_event(
                    BackendLogEvent::AppStateRestoredFromPersistence,
                    &[
                        ("state_status", format!("{:?}", timer_state.status)),
                        ("saved_at", saved_at_utc.to_rfc3339()),
                    ],
                );
                Self::prepare_restored_state(clock, timer_state, saved_at_utc)
            }
        };

        let is_running = preparation.initial_timer_state.status != TimerStatus::Idle;

        log_info_event(
            BackendLogEvent::AppStateInitializationCompleted,
            &[
                (
                    "restore_status",
                    format!("{:?}", preparation.initialization_outcome.restore_status),
                ),
                ("is_running", is_running.to_string()),
            ],
        );

        preparation
    }

    fn prepare_restored_state(
        clock: &dyn Clock,
        timer_state: TimerState,
        saved_at_utc: DateTime<Utc>,
    ) -> StartupPreparation {
        if timer_state.status == TimerStatus::Working && timer_state.remaining_seconds == 0 {
            return Self::prepare_expired_work_restore(clock, timer_state, saved_at_utc);
        }

        let mut timer_manager = TimerStateManager::new(None);
        let initial_tick_payload = timer_manager.apply_persisted_state(timer_state);
        let restored_timer_state = timer_manager.get_state().clone();
        let restored_running_saved_at_local =
            restored_running_saved_at_local(clock, &restored_timer_state, saved_at_utc);
        let startup_action = if initial_tick_payload.is_some() {
            StartupRestoreAction::RestartRestoredTimerTask
        } else {
            StartupRestoreAction::None
        };

        StartupPreparation {
            initial_timer_state: restored_timer_state,
            restored_running_saved_at_local,
            initialization_outcome: AppInitializationOutcome {
                initial_tick_payload,
                restore_status: AppRestoreStatus::Restored,
                startup_action,
            },
        }
    }

    fn prepare_expired_work_restore(
        clock: &dyn Clock,
        timer_state: TimerState,
        saved_at_utc: DateTime<Utc>,
    ) -> StartupPreparation {
        let mut timer_manager = TimerStateManager::new(Some(timer_state));
        let completed_payload = timer_manager.complete_work_session_transition();
        let transition_outcome =
            SessionTransitionOutcome::from_completed_payload(&completed_payload);
        let finalized_timer_state = timer_manager.get_state().clone();

        StartupPreparation {
            initial_timer_state: finalized_timer_state.clone(),
            restored_running_saved_at_local: restored_running_saved_at_local(
                clock,
                &finalized_timer_state,
                saved_at_utc,
            ),
            initialization_outcome: AppInitializationOutcome {
                initial_tick_payload: None,
                restore_status: AppRestoreStatus::Restored,
                startup_action: StartupRestoreAction::FinalizeExpiredRestoredWork(
                    transition_outcome,
                ),
            },
        }
    }

    async fn snapshot_timer_state(&self) -> TimerState {
        let timer_manager = self.timer_manager.lock().await;
        timer_manager.get_state().clone()
    }
}

impl StartupPreparation {
    fn idle(restore_status: AppRestoreStatus) -> Self {
        Self {
            initial_timer_state: TimerState::default(),
            restored_running_saved_at_local: None,
            initialization_outcome: AppInitializationOutcome {
                initial_tick_payload: None,
                restore_status,
                startup_action: StartupRestoreAction::None,
            },
        }
    }
}

fn restored_running_saved_at_local(
    clock: &dyn Clock,
    timer_state: &TimerState,
    saved_at_utc: DateTime<Utc>,
) -> Option<DateTime<Local>> {
    if !matches!(
        timer_state.status,
        TimerStatus::Working | TimerStatus::OnBreak
    ) {
        return None;
    }

    let saved_at_local = saved_at_utc.with_timezone(&Local);
    let lookback_floor_local = clock.now_local() - ChronoDuration::days(7);
    Some(saved_at_local.max(lookback_floor_local))
}

async fn load_persisted_state_async(persistence: &PersistenceManager) -> PersistenceLoadOutcome {
    let persistence = persistence.clone();

    match tokio::task::spawn_blocking(move || persistence.load_state_sync()).await {
        Ok(outcome) => outcome,
        Err(join_error) => {
            log_error_event(
                BackendLogEvent::StoragePersistenceReadFailed,
                &[],
                Some(&join_error),
            );
            PersistenceLoadOutcome::NoState
        }
    }
}

impl Drop for AppState {
    fn drop(&mut self) {
        if let Ok(mut scheduler_task_state) = self.scheduler_task_state.lock() {
            if let Some(cancellation_token) = scheduler_task_state.cancellation_token.take() {
                cancellation_token.cancel();
            }
            if let Some(join_handle) = scheduler_task_state.join_handle.take() {
                join_handle.abort();
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::managers::clock::testing::FixedClock;
    use crate::types::{
        ScheduledStopState, SessionTransitionReason, SessionType, TimerStartSource,
        PERSISTED_STATE_SCHEMA_VERSION_V3,
    };
    use chrono::TimeZone;
    use serde_json::json;
    use std::path::{Path, PathBuf};
    use std::sync::atomic::{AtomicBool as StdAtomicBool, AtomicUsize};
    use std::time::{SystemTime, UNIX_EPOCH};
    use std::{io, time::Duration as StdDuration};

    type SaveReleaseGate = Arc<(StdMutex<bool>, Condvar)>;

    fn create_temp_dir(test_name: &str) -> PathBuf {
        let timestamp_nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time should be after unix epoch")
            .as_nanos();

        let path = std::env::temp_dir().join(format!(
            "pomolice-state-{}-{}-{}",
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
    ) -> DateTime<Utc> {
        Utc.with_ymd_and_hms(year, month, day, hour, minute, second)
            .single()
            .expect("valid UTC datetime")
    }

    fn fixed_clock(now_utc: DateTime<Utc>) -> Arc<FixedClock> {
        Arc::new(FixedClock::new(now_utc.with_timezone(&Local)))
    }

    fn as_clock_trait(clock: &Arc<FixedClock>) -> Arc<dyn Clock + Send + Sync> {
        Arc::<FixedClock>::clone(clock) as Arc<dyn Clock + Send + Sync>
    }

    fn save_state_to_dir(clock: Arc<FixedClock>, dir: &Path, timer_state: TimerState) {
        let persistence = PersistenceManager::new(clock, Some(dir.to_path_buf()));
        persistence
            .save_state_sync(timer_state)
            .expect("sync save should succeed");
    }

    fn build_blocked_first_save_hook() -> (
        Arc<dyn Fn() + Send + Sync>,
        Arc<StdAtomicBool>,
        SaveReleaseGate,
    ) {
        let first_save_blocked = Arc::new(StdAtomicBool::new(false));
        let release_first_save = Arc::new((StdMutex::new(false), Condvar::new()));
        let save_call_count = Arc::new(AtomicUsize::new(0));
        let before_save_hook = {
            let first_save_blocked = Arc::clone(&first_save_blocked);
            let release_first_save = Arc::clone(&release_first_save);
            let save_call_count = Arc::clone(&save_call_count);

            Arc::new(move || {
                let call_index = save_call_count.fetch_add(1, Ordering::AcqRel);
                if call_index != 0 {
                    return;
                }

                first_save_blocked.store(true, Ordering::Release);
                let (lock, condvar) = &*release_first_save;
                let mut released = lock
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner);
                while !*released {
                    released = condvar
                        .wait(released)
                        .unwrap_or_else(std::sync::PoisonError::into_inner);
                }
            }) as Arc<dyn Fn() + Send + Sync>
        };

        (before_save_hook, first_save_blocked, release_first_save)
    }

    fn build_second_runtime_save_snapshot_ready_hook(
    ) -> (RuntimeSaveSnapshotReadyHook, Arc<StdAtomicBool>) {
        let second_runtime_save_snapshot_ready = Arc::new(StdAtomicBool::new(false));
        let runtime_save_snapshot_count = Arc::new(AtomicUsize::new(0));
        let snapshot_ready_hook = {
            let second_runtime_save_snapshot_ready =
                Arc::clone(&second_runtime_save_snapshot_ready);
            let runtime_save_snapshot_count = Arc::clone(&runtime_save_snapshot_count);

            Arc::new(move || {
                let call_index = runtime_save_snapshot_count.fetch_add(1, Ordering::AcqRel);
                if call_index == 1 {
                    second_runtime_save_snapshot_ready.store(true, Ordering::Release);
                }
            }) as RuntimeSaveSnapshotReadyHook
        };

        (snapshot_ready_hook, second_runtime_save_snapshot_ready)
    }

    async fn build_app_state_with_blocked_first_save(
        test_name: &str,
    ) -> (Arc<AppState>, PathBuf, Arc<StdAtomicBool>, SaveReleaseGate) {
        build_blocked_first_save_app_state(test_name, None).await
    }

    async fn build_app_state_with_blocked_first_save_and_snapshot_ready_hook(
        test_name: &str,
        snapshot_ready_hook: RuntimeSaveSnapshotReadyHook,
    ) -> (Arc<AppState>, PathBuf, Arc<StdAtomicBool>, SaveReleaseGate) {
        build_blocked_first_save_app_state(test_name, Some(snapshot_ready_hook)).await
    }

    async fn build_blocked_first_save_app_state(
        test_name: &str,
        snapshot_ready_hook: Option<RuntimeSaveSnapshotReadyHook>,
    ) -> (Arc<AppState>, PathBuf, Arc<StdAtomicBool>, SaveReleaseGate) {
        let temp_dir = create_temp_dir(test_name);
        let clock = fixed_clock(create_utc_datetime(2025, 2, 1, 9, 0, 0));
        let (before_save_hook, first_save_blocked, release_first_save) =
            build_blocked_first_save_hook();
        let persistence = PersistenceManager::new(as_clock_trait(&clock), Some(temp_dir.clone()))
            .with_before_save_hook(before_save_hook);
        let (mut app_state, _) =
            build_startup_state_for_tests_with_persistence(Arc::clone(&clock), persistence).await;
        if let Some(snapshot_ready_hook) = snapshot_ready_hook {
            app_state =
                app_state.with_runtime_save_snapshot_ready_hook_for_tests(snapshot_ready_hook);
        }

        (
            Arc::new(app_state),
            temp_dir,
            first_save_blocked,
            release_first_save,
        )
    }

    async fn apply_working_timer_state(app_state: &AppState, remaining_seconds: u64) {
        let mut timer_manager = app_state.timer_manager.lock().await;
        let mut state = timer_manager.get_state().clone();
        state.status = TimerStatus::Working;
        state.remaining_seconds = remaining_seconds;
        let _ = timer_manager.apply_persisted_state(state);
    }

    fn release_blocked_save(release_first_save: &SaveReleaseGate) {
        let (lock, condvar) = &**release_first_save;
        let mut released = lock
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        *released = true;
        condvar.notify_all();
    }

    fn assert_persisted_remaining_seconds(persistence: &PersistenceManager, expected: u64) {
        match persistence.load_state_sync() {
            PersistenceLoadOutcome::Restored { timer_state, .. } => {
                assert_eq!(timer_state.remaining_seconds, expected);
            }
            PersistenceLoadOutcome::NoState => panic!("expected restored state"),
            PersistenceLoadOutcome::UnsupportedSchemaFallback => {
                panic!("expected restored state")
            }
        }
    }

    async fn build_startup_state_for_tests(
        clock: Arc<FixedClock>,
        dir: &Path,
    ) -> (AppState, AppInitializationOutcome) {
        build_startup_state_for_tests_with_persistence(
            Arc::clone(&clock),
            PersistenceManager::new(as_clock_trait(&clock), Some(dir.to_path_buf())),
        )
        .await
    }

    async fn build_startup_state_for_tests_with_persistence(
        clock: Arc<FixedClock>,
        persistence: PersistenceManager,
    ) -> (AppState, AppInitializationOutcome) {
        AppState::build_startup_state_for_tests(as_clock_trait(&clock), persistence).await
    }

    fn build_startup_state_for_tests_sync(
        clock: Arc<FixedClock>,
        dir: &Path,
    ) -> (AppState, AppInitializationOutcome) {
        let runtime = tokio::runtime::Runtime::new().expect("test runtime should be created");
        let outcome = runtime.block_on(build_startup_state_for_tests(clock, dir));
        drop(runtime);
        outcome
    }

    async fn wait_until_flag_is_set(flag: &StdAtomicBool, context: &str) {
        for _ in 0..100 {
            if flag.load(Ordering::Acquire) {
                return;
            }
            tokio::time::sleep(StdDuration::from_millis(10)).await;
        }

        panic!("timed out waiting for {context}");
    }

    #[tokio::test]
    async fn test_build_startup_state_restores_running_state_before_manage() {
        let temp_dir = create_temp_dir("build-startup-state-restores-running");
        let clock = fixed_clock(create_utc_datetime(2025, 2, 1, 9, 0, 0));
        let running_state = TimerState {
            status: TimerStatus::Working,
            remaining_seconds: 900,
            ..TimerState::default()
        };

        save_state_to_dir(Arc::clone(&clock), &temp_dir, running_state);

        let (app_state, outcome) =
            build_startup_state_for_tests(Arc::clone(&clock), &temp_dir).await;

        assert_eq!(outcome.restore_status, AppRestoreStatus::Restored);
        assert_eq!(
            outcome
                .initial_tick_payload
                .expect("running restore should return a tick payload")
                .remaining_seconds,
            900
        );
        assert!(app_state.restored_running_saved_at_local().is_some());
        assert_eq!(
            app_state.timer_manager.lock().await.status(),
            TimerStatus::Working
        );

        cleanup_temp_dir(&temp_dir);
    }

    #[tokio::test]
    async fn test_build_startup_state_restored_running_work_marks_startup_resume() {
        let temp_dir = create_temp_dir("build-startup-state-running-startup-resume");
        let saved_at_utc = create_utc_datetime(2025, 2, 1, 9, 0, 0);
        let restore_at_utc = create_utc_datetime(2025, 2, 1, 9, 2, 0);
        let save_clock = fixed_clock(saved_at_utc);
        let restore_clock = fixed_clock(restore_at_utc);

        save_state_to_dir(
            Arc::clone(&save_clock),
            &temp_dir,
            TimerState {
                status: TimerStatus::Working,
                remaining_seconds: 900,
                timer_start_source: Some(TimerStartSource::ManualInsideWindow),
                ..TimerState::default()
            },
        );

        let (app_state, outcome) =
            build_startup_state_for_tests(Arc::clone(&restore_clock), &temp_dir).await;

        assert_eq!(outcome.restore_status, AppRestoreStatus::Restored);
        assert!(matches!(
            outcome.startup_action,
            StartupRestoreAction::RestartRestoredTimerTask
        ));

        let initial_tick_payload = outcome
            .initial_tick_payload
            .expect("restored running work should provide an initial tick payload");
        assert_eq!(initial_tick_payload.status, TimerStatus::Working);
        assert_eq!(initial_tick_payload.remaining_seconds, 780);
        assert_eq!(
            app_state.timer_manager.lock().await.status(),
            TimerStatus::Working
        );
        assert_eq!(
            app_state.restored_running_saved_at_local(),
            Some(saved_at_utc.with_timezone(&Local))
        );

        cleanup_temp_dir(&temp_dir);
    }

    #[tokio::test]
    async fn test_build_startup_state_finalizes_expired_work_before_manage() {
        let temp_dir = create_temp_dir("build-startup-state-expired-work-to-break");
        let saved_at_utc = create_utc_datetime(2025, 2, 1, 9, 0, 0);
        let restore_at_utc = create_utc_datetime(2025, 2, 1, 9, 2, 0);
        let save_clock = fixed_clock(saved_at_utc);
        let restore_clock = fixed_clock(restore_at_utc);

        save_state_to_dir(
            Arc::clone(&save_clock),
            &temp_dir,
            TimerState {
                status: TimerStatus::Working,
                remaining_seconds: 60,
                current_cycle: 1,
                timer_start_source: Some(TimerStartSource::ManualInsideWindow),
                ..TimerState::default()
            },
        );

        let (app_state, outcome) =
            build_startup_state_for_tests(Arc::clone(&restore_clock), &temp_dir).await;

        assert_eq!(outcome.restore_status, AppRestoreStatus::Restored);
        assert!(outcome.initial_tick_payload.is_none());
        assert!(matches!(
            outcome.startup_action,
            StartupRestoreAction::FinalizeExpiredRestoredWork(SessionTransitionOutcome {
                transition_reason: SessionTransitionReason::WorkToBreak,
                ..
            })
        ));

        let timer_state = app_state.timer_manager.lock().await.get_state().clone();
        assert_ne!(
            (timer_state.status, timer_state.remaining_seconds),
            (TimerStatus::Working, 0)
        );
        assert_eq!(timer_state.status, TimerStatus::OnBreak);
        assert_eq!(timer_state.session_type, SessionType::ShortBreak);
        assert_eq!(timer_state.remaining_seconds, 300);
        assert_eq!(
            app_state.restored_running_saved_at_local(),
            Some(saved_at_utc.with_timezone(&Local))
        );

        cleanup_temp_dir(&temp_dir);
    }

    #[tokio::test]
    async fn test_build_startup_state_expired_work_preserves_scheduled_stop_priority() {
        let temp_dir = create_temp_dir("build-startup-state-expired-work-scheduled-stop");
        let saved_at_utc = create_utc_datetime(2025, 2, 1, 9, 0, 0);
        let restore_at_utc = create_utc_datetime(2025, 2, 1, 9, 2, 0);
        let save_clock = fixed_clock(saved_at_utc);
        let restore_clock = fixed_clock(restore_at_utc);

        save_state_to_dir(
            Arc::clone(&save_clock),
            &temp_dir,
            TimerState {
                status: TimerStatus::Working,
                remaining_seconds: 60,
                scheduled_stop_state: ScheduledStopState::Pending,
                auto_run_feature_enabled: true,
                timer_start_source: Some(TimerStartSource::ManualInsideWindow),
                ..TimerState::default()
            },
        );

        let (app_state, outcome) =
            build_startup_state_for_tests(Arc::clone(&restore_clock), &temp_dir).await;

        assert_eq!(outcome.restore_status, AppRestoreStatus::Restored);
        assert!(outcome.initial_tick_payload.is_none());
        assert!(matches!(
            outcome.startup_action,
            StartupRestoreAction::FinalizeExpiredRestoredWork(SessionTransitionOutcome {
                transition_reason: SessionTransitionReason::ScheduledStopExecuted,
                ..
            })
        ));

        let timer_state = app_state.timer_manager.lock().await.get_state().clone();
        assert_eq!(timer_state.status, TimerStatus::Idle);
        assert_eq!(timer_state.session_type, SessionType::Work);
        assert_eq!(
            timer_state.remaining_seconds,
            timer_state.settings.work_duration_secs.get()
        );
        assert!(app_state.restored_running_saved_at_local().is_none());

        cleanup_temp_dir(&temp_dir);
    }

    #[tokio::test]
    async fn test_build_startup_state_sets_restored_running_timestamp_only_for_running_sessions() {
        let temp_dir = create_temp_dir("build-startup-state-idle-restored");
        let clock = fixed_clock(create_utc_datetime(2025, 2, 1, 9, 0, 0));
        save_state_to_dir(Arc::clone(&clock), &temp_dir, TimerState::default());

        let (app_state, outcome) =
            build_startup_state_for_tests(Arc::clone(&clock), &temp_dir).await;

        assert_eq!(outcome.restore_status, AppRestoreStatus::Restored);
        assert!(outcome.initial_tick_payload.is_none());
        assert!(app_state.restored_running_saved_at_local().is_none());

        cleanup_temp_dir(&temp_dir);
    }

    #[tokio::test]
    async fn test_build_startup_state_unsupported_schema_uses_default_state() {
        let temp_dir = create_temp_dir("build-startup-state-v3-fallback");
        let clock = fixed_clock(create_utc_datetime(2025, 2, 1, 9, 0, 0));
        let saved_at_utc = create_utc_datetime(2025, 2, 1, 9, 0, 0);

        std::fs::write(
            temp_dir.join("timer_state.json"),
            serde_json::to_string_pretty(&json!({
                "schema_version": PERSISTED_STATE_SCHEMA_VERSION_V3,
                "status": "working",
                "remaining_seconds": 1200,
                "session_type": "work",
                "current_cycle": 2,
                "completed_pomodoros": 1,
                "post_break_decision_pending": false,
                "settings": {
                    "work_duration_secs": 1500,
                    "short_break_duration_secs": 300,
                    "long_break_duration_secs": 900,
                    "long_break_interval": 4,
                    "post_break_transition_policy": "require_manual_continue"
                },
                "saved_at": saved_at_utc,
                "auto_run_feature_enabled": false,
                "weekly_auto_run_config": {
                    "mon": { "enabled": false, "start_time": "09:00", "end_time": "18:00" },
                    "tue": { "enabled": false, "start_time": "09:00", "end_time": "18:00" },
                    "wed": { "enabled": false, "start_time": "09:00", "end_time": "18:00" },
                    "thu": { "enabled": false, "start_time": "09:00", "end_time": "18:00" },
                    "fri": { "enabled": false, "start_time": "09:00", "end_time": "18:00" },
                    "sat": { "enabled": false, "start_time": "09:00", "end_time": "18:00" },
                    "sun": { "enabled": false, "start_time": "09:00", "end_time": "18:00" }
                },
                "timer_start_source": "manual_inside_window",
                "scheduled_stop_state": "pending"
            }))
            .expect("legacy json should serialize"),
        )
        .expect("legacy json should be writable");

        let (app_state, outcome) =
            build_startup_state_for_tests(Arc::clone(&clock), &temp_dir).await;

        assert_eq!(
            outcome.restore_status,
            AppRestoreStatus::UnsupportedSchemaFallback
        );
        assert!(outcome.initial_tick_payload.is_none());
        assert!(app_state.restored_running_saved_at_local().is_none());
        assert_eq!(
            app_state.timer_manager.lock().await.status(),
            TimerStatus::Idle
        );

        cleanup_temp_dir(&temp_dir);
    }

    #[tokio::test]
    async fn test_persist_current_state_async_persists_current_timer_state() {
        let temp_dir = create_temp_dir("persist-current-state");
        let clock = fixed_clock(create_utc_datetime(2025, 2, 1, 9, 0, 0));
        let (app_state, _) = build_startup_state_for_tests(Arc::clone(&clock), &temp_dir).await;

        {
            let mut timer_manager = app_state.timer_manager.lock().await;
            let mut state = timer_manager.get_state().clone();
            state.status = TimerStatus::Working;
            state.remaining_seconds = 111;
            let _ = timer_manager.apply_persisted_state(state);
        }

        app_state
            .persist_current_state_async()
            .await
            .expect("runtime save should succeed");

        match app_state.persistence.load_state_sync() {
            PersistenceLoadOutcome::Restored { timer_state, .. } => {
                assert_eq!(timer_state.status, TimerStatus::Working);
                assert_eq!(timer_state.remaining_seconds, 111);
            }
            PersistenceLoadOutcome::NoState => panic!("expected restored state"),
            PersistenceLoadOutcome::UnsupportedSchemaFallback => {
                panic!("expected restored state")
            }
        }

        cleanup_temp_dir(&temp_dir);
    }

    #[tokio::test]
    async fn test_persist_current_state_async_preserves_latest_completed_disk_state() {
        let temp_dir = create_temp_dir("persist-latest-completed-state");
        let clock = fixed_clock(create_utc_datetime(2025, 2, 1, 9, 0, 0));
        let (app_state, _) = build_startup_state_for_tests(Arc::clone(&clock), &temp_dir).await;

        {
            let mut timer_manager = app_state.timer_manager.lock().await;
            let mut state = timer_manager.get_state().clone();
            state.status = TimerStatus::Working;
            state.remaining_seconds = 111;
            let _ = timer_manager.apply_persisted_state(state);
        }

        app_state
            .persist_current_state_async()
            .await
            .expect("first runtime save should succeed");

        {
            let mut timer_manager = app_state.timer_manager.lock().await;
            let mut state = timer_manager.get_state().clone();
            state.remaining_seconds = 222;
            let _ = timer_manager.apply_persisted_state(state);
        }

        app_state
            .persist_current_state_async()
            .await
            .expect("second runtime save should succeed");

        {
            let mut timer_manager = app_state.timer_manager.lock().await;
            let mut state = timer_manager.get_state().clone();
            state.remaining_seconds = 333;
            let _ = timer_manager.apply_persisted_state(state);
        }

        assert_eq!(
            app_state.timer_manager.lock().await.get_state().remaining_seconds,
            333,
            "mutating in-memory state without another save must not rewrite the last committed snapshot"
        );

        match app_state.persistence.load_state_sync() {
            PersistenceLoadOutcome::Restored { timer_state, .. } => {
                assert_eq!(timer_state.remaining_seconds, 222);
            }
            PersistenceLoadOutcome::NoState => panic!("expected restored state"),
            PersistenceLoadOutcome::UnsupportedSchemaFallback => {
                panic!("expected restored state")
            }
        }

        cleanup_temp_dir(&temp_dir);
    }

    #[tokio::test]
    async fn test_persist_current_state_async_serializes_overlapping_runtime_saves_in_request_order(
    ) {
        let (app_state, temp_dir, first_save_blocked, release_first_save) =
            build_app_state_with_blocked_first_save("persist-runtime-save-request-order").await;
        apply_working_timer_state(&app_state, 111).await;

        let first_save = {
            let app_state = Arc::clone(&app_state);
            tokio::spawn(async move { app_state.persist_current_state_async().await })
        };

        wait_until_flag_is_set(&first_save_blocked, "the first runtime save to block").await;
        apply_working_timer_state(&app_state, 222).await;

        let second_save = {
            let app_state = Arc::clone(&app_state);
            tokio::spawn(async move { app_state.persist_current_state_async().await })
        };

        release_blocked_save(&release_first_save);

        first_save
            .await
            .expect("first runtime save task should complete")
            .expect("first runtime save should succeed");
        second_save
            .await
            .expect("second runtime save task should complete")
            .expect("second runtime save should succeed");

        assert_persisted_remaining_seconds(&app_state.persistence, 222);

        cleanup_temp_dir(&temp_dir);
    }

    #[tokio::test]
    async fn test_cancelled_pre_execution_save_does_not_commit_or_block_later_save() {
        let (snapshot_ready_hook, cancelled_save_snapshot_ready) =
            build_second_runtime_save_snapshot_ready_hook();
        let (app_state, temp_dir, first_save_blocked, release_first_save) =
            build_app_state_with_blocked_first_save_and_snapshot_ready_hook(
                "cancelled-runtime-save-before-execution",
                snapshot_ready_hook,
            )
            .await;
        apply_working_timer_state(&app_state, 111).await;

        let first_save = {
            let app_state = Arc::clone(&app_state);
            tokio::spawn(async move { app_state.persist_current_state_async().await })
        };

        wait_until_flag_is_set(&first_save_blocked, "the first runtime save to block").await;
        apply_working_timer_state(&app_state, 222).await;

        let cancelled_save = {
            let app_state = Arc::clone(&app_state);
            tokio::spawn(async move { app_state.persist_current_state_async().await })
        };

        wait_until_flag_is_set(
            &cancelled_save_snapshot_ready,
            "the cancelled runtime save to reach the pre-execution checkpoint",
        )
        .await;
        cancelled_save.abort();
        let cancelled_join_error = cancelled_save
            .await
            .expect_err("the second runtime save task should be cancelled");
        assert!(
            cancelled_join_error.is_cancelled(),
            "the second runtime save task should report cancellation"
        );

        apply_working_timer_state(&app_state, 333).await;

        let third_save = {
            let app_state = Arc::clone(&app_state);
            tokio::spawn(async move { app_state.persist_current_state_async().await })
        };

        release_blocked_save(&release_first_save);

        first_save
            .await
            .expect("first runtime save task should complete")
            .expect("first runtime save should succeed");
        tokio::time::timeout(StdDuration::from_secs(1), third_save)
            .await
            .expect("a cancelled pre-execution save must not block later runtime saves")
            .expect("third runtime save task should complete")
            .expect("third runtime save should succeed");

        assert_persisted_remaining_seconds(&app_state.persistence, 333);

        cleanup_temp_dir(&temp_dir);
    }

    #[tokio::test]
    async fn test_ordered_save_lane_advances_after_failed_runtime_save() {
        let temp_dir = create_temp_dir("persist-runtime-save-failure-advance");
        let clock = fixed_clock(create_utc_datetime(2025, 2, 1, 9, 0, 0));
        let error_call_count = Arc::new(AtomicUsize::new(0));
        let failing_path = temp_dir.join("timer_state.json");
        let injected_save_error_hook = {
            let error_call_count = Arc::clone(&error_call_count);
            let failing_path = failing_path.clone();

            Arc::new(move || {
                let call_index = error_call_count.fetch_add(1, Ordering::AcqRel);
                if call_index == 0 {
                    Some(SaveError::Write {
                        path: failing_path.clone(),
                        source: io::Error::other("injected ordered-save failure"),
                    })
                } else {
                    None
                }
            }) as Arc<dyn Fn() -> Option<SaveError> + Send + Sync>
        };
        let persistence = PersistenceManager::new(as_clock_trait(&clock), Some(temp_dir.clone()))
            .with_injected_save_error_hook(injected_save_error_hook);
        let (app_state, _) =
            build_startup_state_for_tests_with_persistence(Arc::clone(&clock), persistence).await;

        {
            let mut timer_manager = app_state.timer_manager.lock().await;
            let mut state = timer_manager.get_state().clone();
            state.status = TimerStatus::Working;
            state.remaining_seconds = 111;
            let _ = timer_manager.apply_persisted_state(state);
        }

        let first_result = app_state.persist_current_state_async().await;
        assert!(matches!(
            first_result,
            Err(PomoliceError::PersistenceSave(SaveError::Write { .. }))
        ));

        {
            let mut timer_manager = app_state.timer_manager.lock().await;
            let mut state = timer_manager.get_state().clone();
            state.remaining_seconds = 222;
            let _ = timer_manager.apply_persisted_state(state);
        }

        tokio::time::timeout(
            StdDuration::from_secs(1),
            app_state.persist_current_state_async(),
        )
        .await
        .expect("failed runtime save must not stall the ordered save lane")
        .expect("second runtime save should succeed");

        match app_state.persistence.load_state_sync() {
            PersistenceLoadOutcome::Restored { timer_state, .. } => {
                assert_eq!(timer_state.remaining_seconds, 222);
            }
            PersistenceLoadOutcome::NoState => panic!("expected restored state"),
            PersistenceLoadOutcome::UnsupportedSchemaFallback => {
                panic!("expected restored state")
            }
        }

        cleanup_temp_dir(&temp_dir);
    }

    #[tokio::test]
    async fn test_ordered_save_lane_advances_after_panic_in_save_execution() {
        let temp_dir = create_temp_dir("persist-runtime-save-panic-advance");
        let clock = fixed_clock(create_utc_datetime(2025, 2, 1, 9, 0, 0));
        let panic_call_count = Arc::new(AtomicUsize::new(0));
        let before_save_hook = {
            let panic_call_count = Arc::clone(&panic_call_count);

            Arc::new(move || {
                let call_index = panic_call_count.fetch_add(1, Ordering::AcqRel);
                assert_ne!(call_index, 0, "injected ordered-save panic");
            }) as Arc<dyn Fn() + Send + Sync>
        };
        let persistence = PersistenceManager::new(as_clock_trait(&clock), Some(temp_dir.clone()))
            .with_before_save_hook(before_save_hook);
        let (app_state, _) =
            build_startup_state_for_tests_with_persistence(Arc::clone(&clock), persistence).await;

        apply_working_timer_state(&app_state, 111).await;

        let first_result = app_state.persist_current_state_async().await;
        assert!(matches!(
            first_result,
            Err(PomoliceError::PersistenceTaskJoin { .. })
        ));

        apply_working_timer_state(&app_state, 222).await;

        tokio::time::timeout(
            StdDuration::from_secs(1),
            app_state.persist_current_state_async(),
        )
        .await
        .expect("panic during save execution must not stall the ordered save lane")
        .expect("second runtime save should succeed");

        assert_persisted_remaining_seconds(&app_state.persistence, 222);

        cleanup_temp_dir(&temp_dir);
    }

    #[tokio::test]
    async fn test_persist_current_state_async_preserves_path_unavailable_noop_success() {
        let clock = fixed_clock(create_utc_datetime(2025, 2, 1, 9, 0, 0));
        let persistence = PersistenceManager::new(as_clock_trait(&clock), None);
        let (app_state, _) =
            build_startup_state_for_tests_with_persistence(Arc::clone(&clock), persistence).await;

        {
            let mut timer_manager = app_state.timer_manager.lock().await;
            let mut state = timer_manager.get_state().clone();
            state.status = TimerStatus::Working;
            state.remaining_seconds = 555;
            let _ = timer_manager.apply_persisted_state(state);
        }

        app_state
            .persist_current_state_async()
            .await
            .expect("path-unavailable runtime save should remain a no-op success");

        assert!(matches!(
            app_state.persistence.load_state_sync(),
            PersistenceLoadOutcome::NoState
        ));
    }

    #[tokio::test]
    async fn test_flush_sync_on_exit_commits_after_older_in_flight_runtime_save() {
        let (app_state, temp_dir, first_save_blocked, release_first_save) =
            build_app_state_with_blocked_first_save("flush-sync-after-in-flight-runtime-save")
                .await;
        apply_working_timer_state(&app_state, 111).await;

        let runtime_save = {
            let app_state = Arc::clone(&app_state);
            tokio::spawn(async move { app_state.persist_current_state_async().await })
        };

        wait_until_flag_is_set(&first_save_blocked, "the in-flight runtime save to block").await;
        apply_working_timer_state(&app_state, 222).await;

        let exit_flush = {
            let app_state = Arc::clone(&app_state);
            std::thread::spawn(move || app_state.flush_sync_on_exit())
        };

        release_blocked_save(&release_first_save);

        runtime_save
            .await
            .expect("runtime save task should complete")
            .expect("runtime save should succeed");
        exit_flush
            .join()
            .expect("exit flush thread should not panic")
            .expect("exit flush should succeed");

        assert_persisted_remaining_seconds(&app_state.persistence, 222);

        cleanup_temp_dir(&temp_dir);
    }

    #[tokio::test]
    async fn test_cancelled_runtime_save_after_save_execution_starts_does_not_block_flush_sync_on_exit(
    ) {
        let (app_state, temp_dir, first_save_blocked, release_first_save) =
            build_app_state_with_blocked_first_save(
                "cancelled-runtime-save-after-execution-starts",
            )
            .await;
        apply_working_timer_state(&app_state, 111).await;

        let runtime_save = {
            let app_state = Arc::clone(&app_state);
            tokio::spawn(async move { app_state.persist_current_state_async().await })
        };

        wait_until_flag_is_set(
            &first_save_blocked,
            "the first runtime save to begin save execution",
        )
        .await;

        runtime_save.abort();
        let cancelled_join_error = runtime_save
            .await
            .expect_err("the runtime save task should be cancelled after dispatch");
        assert!(
            cancelled_join_error.is_cancelled(),
            "the runtime save task should report cancellation"
        );

        apply_working_timer_state(&app_state, 222).await;

        let exit_flush = {
            let app_state = Arc::clone(&app_state);
            std::thread::spawn(move || app_state.flush_sync_on_exit())
        };

        release_blocked_save(&release_first_save);

        exit_flush
            .join()
            .expect("exit flush thread should not panic")
            .expect("exit flush should succeed after the cancelled dispatched save finishes");

        assert_persisted_remaining_seconds(&app_state.persistence, 222);

        cleanup_temp_dir(&temp_dir);
    }

    #[test]
    fn test_flush_sync_on_exit_persists_current_timer_state() {
        let temp_dir = create_temp_dir("flush-sync-on-exit");
        let clock = fixed_clock(create_utc_datetime(2025, 2, 1, 9, 0, 0));
        let (app_state, _) = build_startup_state_for_tests_sync(Arc::clone(&clock), &temp_dir);

        {
            let mut timer_manager = app_state.timer_manager.blocking_lock();
            let mut state = timer_manager.get_state().clone();
            state.status = TimerStatus::Working;
            state.remaining_seconds = 444;
            let _ = timer_manager.apply_persisted_state(state);
        }

        app_state
            .flush_sync_on_exit()
            .expect("exit flush should succeed");

        match app_state.persistence.load_state_sync() {
            PersistenceLoadOutcome::Restored { timer_state, .. } => {
                assert_eq!(timer_state.status, TimerStatus::Working);
                assert_eq!(timer_state.remaining_seconds, 444);
            }
            PersistenceLoadOutcome::NoState => panic!("expected restored state"),
            PersistenceLoadOutcome::UnsupportedSchemaFallback => {
                panic!("expected restored state")
            }
        }

        cleanup_temp_dir(&temp_dir);
    }
}
