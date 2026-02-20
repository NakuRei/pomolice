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

/// Central application state
/// This is registered as Tauri State and accessible from all commands
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

        Self {
            timer_manager: Arc::new(Mutex::new(TimerStateManager::new(
                app_handle.clone(),
                initial_timer_state,
            ))),
            input_monitor: Arc::new(Mutex::new(InputMonitorManager::new(app_handle.clone()))),
            window_manager: Arc::new(Mutex::new(WindowStateManager::new())),
            persistence,
        }
    }

    /// Initialize the application state
    /// This should be called during app setup
    pub async fn initialize(&self) -> Result<()> {
        info!("Initializing application state");
        // If there was persisted state, the timer manager will handle restoration
        let mut timer = self.timer_manager.lock().await;
        let result = timer.initialize_from_persisted_state();
        info!("Application state initialized successfully");
        result
    }
}
