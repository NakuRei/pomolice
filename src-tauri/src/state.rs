// Application state management
// Central hub for all state, managed via Tauri's State mechanism

use std::sync::Arc;
use tauri::AppHandle;
use tokio::sync::Mutex;
use tracing::info;

use crate::error::Result;
use crate::managers::input_monitor::InputMonitorManager;
use crate::managers::persistence::PersistenceManager;
use crate::managers::timer::TimerStateManager;
use crate::managers::window::WindowStateManager;
use crate::types::TimerTickPayload;

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
/// 3. timer_manager
/// ```
///
/// Each lock must be released (by dropping the guard or closing the `{}` block)
/// before acquiring the next one. Never hold two locks simultaneously.
pub struct AppState {
    pub timer_manager: Arc<Mutex<TimerStateManager>>,
    pub input_monitor: Arc<Mutex<InputMonitorManager>>,
    pub window_manager: Arc<Mutex<WindowStateManager>>,
    pub persistence: Arc<PersistenceManager>,
}

impl AppState {
    /// Create a new AppState instance
    pub fn new(app_handle: AppHandle) -> Self {
        info!("Creating new AppState");
        let persistence = Arc::new(PersistenceManager::new());

        // Load persisted state if available
        let initial_timer_state = persistence.load_state();
        if initial_timer_state.is_some() {
            info!("Restored timer state from persistence");
        } else {
            info!("No persisted state found, using defaults");
        }

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
