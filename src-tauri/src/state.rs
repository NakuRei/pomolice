// Application state management
// Central hub for all state, managed via Tauri's State mechanism

use chrono::{DateTime, Duration as ChronoDuration, Local};
use std::sync::atomic::{AtomicBool, AtomicU64};
use std::sync::{Arc, Mutex as StdMutex};
use tauri::async_runtime::JoinHandle;
use tauri::AppHandle;
use tokio::sync::Mutex;
use tokio_util::sync::CancellationToken;
use tracing::info;

use crate::error::Result;
use crate::managers::clock::{Clock, SystemClock};
use crate::managers::input_monitor::InputMonitorManager;
use crate::managers::persistence::PersistenceManager;
use crate::managers::timer::TimerStateManager;
use crate::managers::window::WindowStateManager;
use crate::types::{BootPromptRuntimeState, TimerStatus, TimerTickPayload};

/// Lifecycle handles for the weekly scheduler background task.
#[derive(Default)]
pub struct SchedulerTaskState {
    pub cancellation_token: Option<CancellationToken>,
    pub join_handle: Option<JoinHandle<()>>,
}

/// Central application state
/// This is registered as Tauri State and accessible from all commands
///
/// # Lock Ordering
///
/// To prevent deadlocks, always acquire multiple Mutex locks in this order:
///
/// ```text
/// 1. input_monitor
/// 2. window_manager
/// 3. boot_prompt_runtime_state
/// 4. timer_manager
/// ```
///
/// Each lock must be released (by dropping the guard or closing the `{}` block)
/// before acquiring the next one. Never hold two locks simultaneously.
///
/// `scheduler_task_state` must not be acquired together with other locks.
pub struct AppState {
    pub timer_manager: Arc<Mutex<TimerStateManager>>,
    pub input_monitor: Arc<Mutex<InputMonitorManager>>,
    pub window_manager: Arc<Mutex<WindowStateManager>>,
    pub persistence: Arc<PersistenceManager>,
    pub clock: Arc<dyn Clock + Send + Sync>,
    pub scheduler_task_state: StdMutex<SchedulerTaskState>,
    pub boot_prompt_runtime_state: StdMutex<BootPromptRuntimeState>,
    pub boot_prompt_check_pending: AtomicBool,
    pub auto_start_boundary_enabled: AtomicBool,
    pub restored_running_saved_at_local: Option<DateTime<Local>>,
    pub config_revision: AtomicU64,
}

impl AppState {
    /// Create a new AppState instance
    pub fn new(app_handle: AppHandle) -> Self {
        Self::new_with_clock(app_handle, Arc::new(SystemClock))
    }

    /// Create a new AppState instance with an injected clock implementation.
    pub fn new_with_clock(
        app_handle: AppHandle,
        clock: Arc<dyn Clock + Send + Sync>,
    ) -> Self {
        info!("Creating new AppState");
        let persistence = Arc::new(PersistenceManager::new(clock.clone()));

        // Load persisted state if available
        let (initial_timer_state, restored_saved_at_utc) =
            match persistence.load_state_with_saved_at_utc() {
                Some((timer_state, saved_at_utc)) => (Some(timer_state), Some(saved_at_utc)),
                None => (None, None),
            };

        if initial_timer_state.is_some() {
            info!("Restored timer state from persistence");
        } else {
            info!("No persisted state found, using defaults");
        }

        let restored_running_saved_at_local = match (
            initial_timer_state.as_ref(),
            restored_saved_at_utc,
        ) {
            (Some(timer_state), Some(saved_at_utc))
                if matches!(timer_state.status, TimerStatus::Working | TimerStatus::OnBreak) =>
            {
                let saved_at_local = saved_at_utc.with_timezone(&Local);
                let now_local = clock.now_local();
                let lookback_floor_local = now_local - ChronoDuration::days(7);
                Some(saved_at_local.max(lookback_floor_local))
            }
            _ => None,
        };

        let app_for_escape = app_handle.clone();
        let app_for_input = app_handle.clone();

        Self {
            timer_manager: Arc::new(Mutex::new(TimerStateManager::new(initial_timer_state))),
            input_monitor: Arc::new(Mutex::new(InputMonitorManager::new(
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
            ))),
            window_manager: Arc::new(Mutex::new(WindowStateManager::new())),
            persistence,
            clock,
            scheduler_task_state: StdMutex::new(SchedulerTaskState::default()),
            boot_prompt_runtime_state: StdMutex::new(BootPromptRuntimeState::NotApplicable),
            boot_prompt_check_pending: AtomicBool::new(true),
            auto_start_boundary_enabled: AtomicBool::new(false),
            restored_running_saved_at_local,
            config_revision: AtomicU64::new(0),
        }
    }

    /// Stops the scheduler task by canceling the token and taking ownership
    /// of the stored JoinHandle.
    pub fn stop_scheduler_task(&self) {
        if let Ok(mut scheduler_task_state) = self.scheduler_task_state.lock() {
            if let Some(cancellation_token) = scheduler_task_state.cancellation_token.take() {
                cancellation_token.cancel();
            }
            if let Some(join_handle) = scheduler_task_state.join_handle.take() {
                join_handle.abort();
            }
        }
    }

    /// Initialize the application state
    ///
    /// Returns a tick payload to emit if the timer was running when the app closed,
    /// or `None` if the timer was idle. The caller is responsible for emitting the event.
    pub async fn initialize(&self) -> Result<Option<TimerTickPayload>> {
        info!("Initializing application state");
        let mut timer = self.timer_manager.lock().await;
        let result = timer.initialize_from_persisted_state();
        info!("Application state initialized successfully");
        result
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
