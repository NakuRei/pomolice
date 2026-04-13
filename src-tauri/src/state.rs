// Application state management
// Central hub for all state, managed via Tauri's State mechanism

use chrono::{DateTime, Duration as ChronoDuration, Local, Utc};
use std::sync::atomic::{AtomicBool, AtomicU64};
use std::sync::{Arc, Mutex as StdMutex};
use tauri::async_runtime::JoinHandle;
use tauri::{AppHandle, Manager};
use tokio::sync::{Mutex, OnceCell};
use tokio_util::sync::CancellationToken;

use crate::logging_event::{log_info_event, BackendLogEvent};
use crate::managers::clock::{Clock, SystemClock};
use crate::managers::input_monitor::InputMonitorManager;
use crate::managers::persistence::{PersistenceLoadOutcome, PersistenceManager};
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

/// Central application state
///
/// With one exception, never hold multiple locks at the same time.
/// `persist_current_state()` intentionally acquires `persistence_save_lock`
/// and then `timer_manager` to serialize save requests while cloning fresh state.
pub(crate) struct AppState {
    pub timer_manager: Mutex<TimerStateManager>,
    pub input_monitor: Mutex<InputMonitorManager>,
    pub window_manager: Mutex<WindowStateManager>,
    pub persistence: PersistenceManager,
    pub clock: Arc<dyn Clock + Send + Sync>,
    pub scheduler_task_state: StdMutex<SchedulerTaskState>,
    pub boot_prompt_runtime_state: StdMutex<BootPromptRuntimeState>,
    pub boot_prompt_check_pending: AtomicBool,
    pub auto_start_boundary_enabled: AtomicBool,
    restored_running_saved_at_local: StdMutex<Option<DateTime<Local>>>,
    pub config_revision: AtomicU64,
    persistence_save_lock: Mutex<()>,
    initialization: OnceCell<AppInitializationOutcome>,
}

impl AppState {
    /// Create a new `AppState` instance.
    pub(crate) fn new(app_handle: &AppHandle) -> Self {
        Self::new_with_clock(app_handle, Arc::new(SystemClock))
    }

    /// Create a new `AppState` instance with an injected clock implementation.
    pub(crate) fn new_with_clock(
        app_handle: &AppHandle,
        clock: Arc<dyn Clock + Send + Sync>,
    ) -> Self {
        let app_data_dir = app_handle.path().app_data_dir().ok();
        let persistence = PersistenceManager::new(Arc::clone(&clock), app_data_dir);
        let app_for_escape = app_handle.clone();
        let app_for_input = app_handle.clone();
        Self::build_state(
            clock,
            persistence,
            move || {
                let app = app_for_escape.clone();
                tauri::async_runtime::spawn(async move {
                    crate::lib_handle_escape_combo(app).await;
                });
            },
            move || {
                let app = app_for_input.clone();
                tauri::async_runtime::spawn(async move {
                    crate::lib_handle_input_detected(app).await;
                });
            },
        )
    }

    #[cfg(test)]
    pub(crate) fn new_for_tests(
        clock: Arc<dyn Clock + Send + Sync>,
        persistence: PersistenceManager,
    ) -> Self {
        Self::build_state(clock, persistence, || {}, || {})
    }

    fn build_state(
        clock: Arc<dyn Clock + Send + Sync>,
        persistence: PersistenceManager,
        on_escape_combo: impl Fn() + Send + Sync + 'static,
        on_input_detected: impl Fn() + Send + Sync + 'static,
    ) -> Self {
        log_info_event(BackendLogEvent::AppStateCreated, &[]);

        Self {
            timer_manager: Mutex::new(TimerStateManager::new(None)),
            input_monitor: Mutex::new(InputMonitorManager::new(on_escape_combo, on_input_detected)),
            window_manager: Mutex::new(WindowStateManager::new()),
            persistence,
            clock,
            scheduler_task_state: StdMutex::new(SchedulerTaskState::default()),
            boot_prompt_runtime_state: StdMutex::new(BootPromptRuntimeState::NotApplicable),
            boot_prompt_check_pending: AtomicBool::new(true),
            auto_start_boundary_enabled: AtomicBool::new(false),
            restored_running_saved_at_local: StdMutex::new(None),
            config_revision: AtomicU64::new(0),
            persistence_save_lock: Mutex::new(()),
            initialization: OnceCell::new(),
        }
    }

    /// Stops the scheduler task by canceling the token and taking ownership
    /// of the stored `JoinHandle`.
    pub(crate) fn stop_scheduler_task(&self) {
        if let Ok(mut scheduler_task_state) = self.scheduler_task_state.lock() {
            if let Some(cancellation_token) = scheduler_task_state.cancellation_token.take() {
                cancellation_token.cancel();
            }
            if let Some(join_handle) = scheduler_task_state.join_handle.take() {
                join_handle.abort();
            }
        }
    }

    /// Persist the latest timer state without holding `timer_manager` across `.await`.
    pub(crate) async fn persist_current_state(&self) {
        let _save_order_guard = self.persistence_save_lock.lock().await;
        let timer_state = {
            let timer_manager = self.timer_manager.lock().await;
            timer_manager.get_state().clone()
        };
        self.persistence.save_state(timer_state).await;
    }

    /// Initialize the application state once, restoring persisted state before startup continues.
    pub(crate) async fn initialize(&self) -> AppInitializationOutcome {
        self.initialization
            .get_or_init(|| async { self.run_initialize().await })
            .await
            .clone()
    }

    pub(crate) fn restored_running_saved_at_local(&self) -> Option<DateTime<Local>> {
        match self.restored_running_saved_at_local.lock() {
            Ok(saved_at) => *saved_at,
            Err(_) => None,
        }
    }

    async fn run_initialize(&self) -> AppInitializationOutcome {
        log_info_event(BackendLogEvent::AppStateInitializationStarted, &[]);

        let outcome = match self.persistence.load_state().await {
            PersistenceLoadOutcome::NoState => {
                self.set_restored_running_saved_at_local(None);
                log_info_event(BackendLogEvent::AppStateUsingDefaults, &[]);
                AppInitializationOutcome {
                    initial_tick_payload: None,
                    restore_status: AppRestoreStatus::NoState,
                    startup_action: StartupRestoreAction::None,
                }
            }
            PersistenceLoadOutcome::UnsupportedSchemaFallback => {
                self.set_restored_running_saved_at_local(None);
                let initial_tick_payload = {
                    let mut timer_manager = self.timer_manager.lock().await;
                    timer_manager.apply_persisted_state(TimerState::default())
                };
                log_info_event(BackendLogEvent::AppStateUsingDefaults, &[]);
                AppInitializationOutcome {
                    initial_tick_payload,
                    restore_status: AppRestoreStatus::UnsupportedSchemaFallback,
                    startup_action: StartupRestoreAction::None,
                }
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
                self.restore_persisted_state(timer_state, saved_at_utc)
                    .await
            }
        };

        let is_running = {
            let timer_manager = self.timer_manager.lock().await;
            timer_manager.status() != TimerStatus::Idle
        };

        log_info_event(
            BackendLogEvent::AppStateInitializationCompleted,
            &[
                ("restore_status", format!("{:?}", outcome.restore_status)),
                ("is_running", is_running.to_string()),
            ],
        );

        outcome
    }

    fn set_restored_running_saved_at_local(&self, saved_at_local: Option<DateTime<Local>>) {
        if let Ok(mut restored_saved_at_local) = self.restored_running_saved_at_local.lock() {
            *restored_saved_at_local = saved_at_local;
        }
    }

    async fn restore_persisted_state(
        &self,
        timer_state: TimerState,
        saved_at_utc: DateTime<Utc>,
    ) -> AppInitializationOutcome {
        if timer_state.status == TimerStatus::Working && timer_state.remaining_seconds == 0 {
            return self
                .restore_expired_work_session(timer_state, saved_at_utc)
                .await;
        }

        let restored_running_saved_at_local =
            restored_running_saved_at_local(&*self.clock, &timer_state, saved_at_utc);
        let initial_tick_payload = {
            let mut timer_manager = self.timer_manager.lock().await;
            timer_manager.apply_persisted_state(timer_state)
        };

        self.set_restored_running_saved_at_local(restored_running_saved_at_local);
        AppInitializationOutcome {
            initial_tick_payload: initial_tick_payload.clone(),
            restore_status: AppRestoreStatus::Restored,
            startup_action: match initial_tick_payload {
                Some(_) => StartupRestoreAction::RestartRestoredTimerTask,
                None => StartupRestoreAction::None,
            },
        }
    }

    async fn restore_expired_work_session(
        &self,
        timer_state: TimerState,
        saved_at_utc: DateTime<Utc>,
    ) -> AppInitializationOutcome {
        let transition_outcome = {
            let mut timer_manager = self.timer_manager.lock().await;
            let _ = timer_manager.apply_persisted_state(timer_state);
            let completed_payload = timer_manager.complete_work_session_transition();
            SessionTransitionOutcome::from_completed_payload(&completed_payload)
        };

        let finalized_timer_state = {
            let timer_manager = self.timer_manager.lock().await;
            timer_manager.get_state().clone()
        };

        self.set_restored_running_saved_at_local(restored_running_saved_at_local(
            &*self.clock,
            &finalized_timer_state,
            saved_at_utc,
        ));

        AppInitializationOutcome {
            initial_tick_payload: None,
            restore_status: AppRestoreStatus::Restored,
            startup_action: StartupRestoreAction::FinalizeExpiredRestoredWork(transition_outcome),
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
    use std::time::{SystemTime, UNIX_EPOCH};

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
        let clock_trait: Arc<dyn Clock + Send + Sync> = <Arc<FixedClock>>::clone(clock);
        clock_trait
    }

    async fn save_state_to_dir(clock: Arc<FixedClock>, dir: &Path, timer_state: TimerState) {
        let persistence = PersistenceManager::new(clock, Some(dir.to_path_buf()));
        persistence.save_state(timer_state).await;
    }

    #[tokio::test]
    async fn test_initialize_restores_running_state_before_returning_initial_tick_payload() {
        let temp_dir = create_temp_dir("initialize-restores-running");
        let clock = fixed_clock(create_utc_datetime(2025, 2, 1, 9, 0, 0));
        let running_state = TimerState {
            status: TimerStatus::Working,
            remaining_seconds: 900,
            ..TimerState::default()
        };

        save_state_to_dir(Arc::clone(&clock), &temp_dir, running_state).await;

        let app_state = AppState::new_for_tests(
            as_clock_trait(&clock),
            PersistenceManager::new(as_clock_trait(&clock), Some(temp_dir.clone())),
        );
        let outcome = app_state.initialize().await;

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
    async fn test_initialize_restored_running_work_with_remaining_time_marks_startup_resume() {
        let temp_dir = create_temp_dir("initialize-restored-running-startup-resume");
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
        )
        .await;

        let app_state = AppState::new_for_tests(
            as_clock_trait(&restore_clock),
            PersistenceManager::new(as_clock_trait(&restore_clock), Some(temp_dir.clone())),
        );
        let outcome = app_state.initialize().await;

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
    async fn test_initialize_expired_restored_work_transitions_to_break_before_first_emit() {
        let temp_dir = create_temp_dir("initialize-expired-restored-work-to-break");
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
        )
        .await;

        let app_state = AppState::new_for_tests(
            as_clock_trait(&restore_clock),
            PersistenceManager::new(as_clock_trait(&restore_clock), Some(temp_dir.clone())),
        );
        let outcome = app_state.initialize().await;

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
    async fn test_initialize_expired_restored_work_preserves_scheduled_stop_priority() {
        let temp_dir = create_temp_dir("initialize-expired-restored-work-scheduled-stop");
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
        )
        .await;

        let app_state = AppState::new_for_tests(
            as_clock_trait(&restore_clock),
            PersistenceManager::new(as_clock_trait(&restore_clock), Some(temp_dir.clone())),
        );
        let outcome = app_state.initialize().await;

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
    async fn test_initialize_sets_restored_running_timestamp_only_for_running_sessions() {
        let temp_dir = create_temp_dir("initialize-idle-restored");
        let clock = fixed_clock(create_utc_datetime(2025, 2, 1, 9, 0, 0));
        save_state_to_dir(Arc::clone(&clock), &temp_dir, TimerState::default()).await;

        let app_state = AppState::new_for_tests(
            as_clock_trait(&clock),
            PersistenceManager::new(as_clock_trait(&clock), Some(temp_dir.clone())),
        );
        let outcome = app_state.initialize().await;

        assert_eq!(outcome.restore_status, AppRestoreStatus::Restored);
        assert!(outcome.initial_tick_payload.is_none());
        assert!(app_state.restored_running_saved_at_local().is_none());

        cleanup_temp_dir(&temp_dir);
    }

    #[tokio::test]
    async fn test_initialize_returns_cached_outcome_without_rereading_persistence() {
        let temp_dir = create_temp_dir("initialize-cached");
        let clock = fixed_clock(create_utc_datetime(2025, 2, 1, 9, 0, 0));
        save_state_to_dir(
            Arc::clone(&clock),
            &temp_dir,
            TimerState {
                status: TimerStatus::Working,
                remaining_seconds: 900,
                ..TimerState::default()
            },
        )
        .await;

        let app_state = AppState::new_for_tests(
            as_clock_trait(&clock),
            PersistenceManager::new(as_clock_trait(&clock), Some(temp_dir.clone())),
        );
        let first = app_state.initialize().await;

        save_state_to_dir(
            Arc::clone(&clock),
            &temp_dir,
            TimerState {
                status: TimerStatus::Working,
                remaining_seconds: 100,
                ..TimerState::default()
            },
        )
        .await;

        let second = app_state.initialize().await;

        assert_eq!(first.restore_status, AppRestoreStatus::Restored);
        assert_eq!(second.restore_status, AppRestoreStatus::Restored);
        assert_eq!(
            first
                .initial_tick_payload
                .as_ref()
                .expect("first payload should exist")
                .remaining_seconds,
            900
        );
        assert_eq!(
            second
                .initial_tick_payload
                .as_ref()
                .expect("second payload should exist")
                .remaining_seconds,
            900
        );

        cleanup_temp_dir(&temp_dir);
    }

    #[tokio::test]
    async fn test_concurrent_initialize_calls_share_one_shot_result() {
        let temp_dir = create_temp_dir("initialize-concurrent");
        let clock = fixed_clock(create_utc_datetime(2025, 2, 1, 9, 0, 0));
        save_state_to_dir(
            Arc::clone(&clock),
            &temp_dir,
            TimerState {
                status: TimerStatus::Working,
                remaining_seconds: 321,
                ..TimerState::default()
            },
        )
        .await;

        let app_state = AppState::new_for_tests(
            as_clock_trait(&clock),
            PersistenceManager::new(as_clock_trait(&clock), Some(temp_dir.clone())),
        );

        let (left, right) = tokio::join!(app_state.initialize(), app_state.initialize());

        assert_eq!(left.restore_status, AppRestoreStatus::Restored);
        assert_eq!(right.restore_status, AppRestoreStatus::Restored);
        assert_eq!(
            left.initial_tick_payload
                .as_ref()
                .expect("left payload should exist")
                .remaining_seconds,
            321
        );
        assert_eq!(
            right
                .initial_tick_payload
                .as_ref()
                .expect("right payload should exist")
                .remaining_seconds,
            321
        );

        cleanup_temp_dir(&temp_dir);
    }

    #[tokio::test]
    async fn test_initialize_unsupported_schema_fallback_does_not_report_restored_running_session()
    {
        let temp_dir = create_temp_dir("initialize-v3-fallback");
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

        let app_state = AppState::new_for_tests(
            as_clock_trait(&clock),
            PersistenceManager::new(as_clock_trait(&clock), Some(temp_dir.clone())),
        );
        let outcome = app_state.initialize().await;

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
    async fn test_persist_current_state_serializes_latest_completed_save() {
        let temp_dir = create_temp_dir("persist-latest");
        let clock = fixed_clock(create_utc_datetime(2025, 2, 1, 9, 0, 0));
        let app_state = AppState::new_for_tests(
            as_clock_trait(&clock),
            PersistenceManager::new(as_clock_trait(&clock), Some(temp_dir.clone())),
        );

        let save_lock = app_state.persistence_save_lock.lock().await;
        {
            let mut timer_manager = app_state.timer_manager.lock().await;
            let mut state = timer_manager.get_state().clone();
            state.status = TimerStatus::Working;
            state.remaining_seconds = 111;
            let _ = timer_manager.apply_persisted_state(state);
        }
        let first_save = app_state.persist_current_state();
        tokio::pin!(first_save);
        tokio::task::yield_now().await;

        {
            let mut timer_manager = app_state.timer_manager.lock().await;
            let mut state = timer_manager.get_state().clone();
            state.remaining_seconds = 222;
            let _ = timer_manager.apply_persisted_state(state);
        }
        let second_save = app_state.persist_current_state();
        tokio::pin!(second_save);
        tokio::task::yield_now().await;

        drop(save_lock);
        first_save.await;
        second_save.await;

        match app_state.persistence.load_state().await {
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
    async fn test_queued_persist_current_state_clones_after_acquiring_save_lock() {
        let temp_dir = create_temp_dir("persist-after-lock");
        let clock = fixed_clock(create_utc_datetime(2025, 2, 1, 9, 0, 0));
        let app_state = AppState::new_for_tests(
            as_clock_trait(&clock),
            PersistenceManager::new(as_clock_trait(&clock), Some(temp_dir.clone())),
        );

        let save_lock = app_state.persistence_save_lock.lock().await;
        {
            let mut timer_manager = app_state.timer_manager.lock().await;
            let mut state = timer_manager.get_state().clone();
            state.status = TimerStatus::Working;
            state.remaining_seconds = 111;
            let _ = timer_manager.apply_persisted_state(state);
        }

        let pending_save = app_state.persist_current_state();
        tokio::pin!(pending_save);
        tokio::task::yield_now().await;

        {
            let mut timer_manager = app_state.timer_manager.lock().await;
            let mut state = timer_manager.get_state().clone();
            state.remaining_seconds = 333;
            let _ = timer_manager.apply_persisted_state(state);
        }

        drop(save_lock);
        pending_save.await;

        match app_state.persistence.load_state().await {
            PersistenceLoadOutcome::Restored { timer_state, .. } => {
                assert_eq!(timer_state.remaining_seconds, 333);
            }
            PersistenceLoadOutcome::NoState => panic!("expected restored state"),
            PersistenceLoadOutcome::UnsupportedSchemaFallback => {
                panic!("expected restored state")
            }
        }

        cleanup_temp_dir(&temp_dir);
    }
}
