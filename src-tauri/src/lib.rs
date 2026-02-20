// Pomolice - Pomodoro Timer Application
// Backend is the single source of truth for all timer state
// Security: Window control is managed through timer_state module

use tauri::{AppHandle, Emitter, Manager};
use tracing::{error, info};

mod commands;
mod error;
mod managers;
mod state;
mod types;

/// Setup event listeners for internal events
fn setup_event_listeners(_app: AppHandle) {
    // Note: For now, we'll handle events directly in the timer thread
    // and input monitor rather than using Tauri's event system
    // This is simpler and avoids the complexity of cross-thread event handling
}

/// Handle timer tick event
pub async fn lib_handle_timer_tick(app: AppHandle) {
    let state_handle = app.state::<state::AppState>();

    let (should_complete_session, tick_payload) = {
        let mut timer = state_handle.timer_manager.lock().await;

        // Only tick if running
        if timer.status() == types::TimerStatus::Idle {
            return;
        }

        let (completed, payload) = timer.tick();

        // Save state
        state_handle.persistence.save_state(timer.get_state());

        (completed, payload)
    };

    // Emit tick event to frontend
    let _ = app.emit("timer_tick", &tick_payload);

    if should_complete_session {
        lib_handle_session_complete(app).await;
    }
}

/// Handle session completion
pub async fn lib_handle_session_complete(app: AppHandle) {
    use types::SessionType;

    let state_handle = app.state::<state::AppState>();

    let (session_type, _next_action) = {
        let timer = state_handle.timer_manager.lock().await;
        (timer.get_state().session_type, timer.status())
    };

    match session_type {
        SessionType::Work => {
            // Work session completed - transition to break
            let (_break_type, payload) = {
                let mut timer = state_handle.timer_manager.lock().await;
                timer.transition_to_break()
            };

            let _ = app.emit("session_completed", &payload);

            // Save state
            {
                let timer = state_handle.timer_manager.lock().await;
                state_handle.persistence.save_state(timer.get_state());
            }

            // Enter fullscreen break mode
            enter_break_mode(app.clone()).await;
        }
        SessionType::ShortBreak | SessionType::LongBreak => {
            // Break session completed - transition to idle
            let payload = {
                let mut timer = state_handle.timer_manager.lock().await;
                timer.transition_to_idle_after_break()
            };

            let _ = app.emit("session_completed", &payload);

            // Save state
            {
                let timer = state_handle.timer_manager.lock().await;
                state_handle.persistence.save_state(timer.get_state());
            }

            // Exit fullscreen break mode
            exit_break_mode(app.clone()).await;
        }
    }
}

/// Enter fullscreen break mode
async fn enter_break_mode(app: AppHandle) {
    info!("Entering fullscreen break mode");
    let state_handle = app.state::<state::AppState>();

    // Start input monitoring
    {
        let mut input_monitor = state_handle.input_monitor.lock().await;
        if let Err(e) = input_monitor.start() {
            error!("Failed to start input monitoring: {:?}", e);
        } else {
            info!("Input monitoring started successfully");
        }
    }

    // Enter fullscreen on all monitors
    {
        let mut window_manager = state_handle.window_manager.lock().await;
        if let Err(e) = window_manager
            .enter_fullscreen_all_monitors(app.clone())
            .await
        {
            error!("Failed to enter fullscreen: {:?}", e);
            return;
        }
        info!("Entered fullscreen mode on all monitors");
    }

    // Update state
    {
        let mut timer = state_handle.timer_manager.lock().await;
        timer.set_fullscreen_break(true);
    }

    let _ = app.emit("break_started", serde_json::json!({}));
}

/// Exit fullscreen break mode
async fn exit_break_mode(app: AppHandle) {
    info!("Exiting fullscreen break mode");
    let state_handle = app.state::<state::AppState>();

    // Stop input monitoring
    {
        let mut input_monitor = state_handle.input_monitor.lock().await;
        if let Err(e) = input_monitor.stop() {
            error!("Failed to stop input monitoring: {:?}", e);
        } else {
            info!("Input monitoring stopped successfully");
        }
    }

    // Exit fullscreen on all monitors
    {
        let mut window_manager = state_handle.window_manager.lock().await;
        if let Err(e) = window_manager
            .exit_fullscreen_all_monitors(app.clone())
            .await
        {
            error!("Failed to exit fullscreen: {:?}", e);
            return;
        }
        info!("Exited fullscreen mode on all monitors");
    }

    // Update state
    {
        let mut timer = state_handle.timer_manager.lock().await;
        timer.set_fullscreen_break(false);
    }

    let _ = app.emit("break_completed", serde_json::json!({}));
}

/// Handle input detected during break
pub async fn lib_handle_input_detected(app: AppHandle) {
    let state_handle = app.state::<state::AppState>();

    let mut timer = state_handle.timer_manager.lock().await;
    let _ = timer.apply_input_penalty();

    // Save state
    state_handle.persistence.save_state(timer.get_state());
}

/// Handle escape combo detection
pub async fn lib_handle_escape_combo(app: AppHandle) {
    let state_handle = app.state::<state::AppState>();

    let mut window_manager = state_handle.window_manager.lock().await;
    let _ = window_manager
        .exit_fullscreen_all_monitors(app.clone())
        .await;

    let mut input_monitor = state_handle.input_monitor.lock().await;
    let _ = input_monitor.stop();
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // Initialize tracing subscriber for structured logging
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .with_target(false)
        .with_thread_ids(false)
        .with_file(true)
        .with_line_number(true)
        .init();

    info!("Starting Pomolice application");

    tauri::Builder::default()
        .plugin(tauri_plugin_notification::init())
        .setup(|app| {
            info!("Initializing application state");
            // Create and register application state
            let app_state = state::AppState::new(app.handle().clone());

            // Initialize state (handles persisted state restoration)
            tauri::async_runtime::block_on(async {
                app_state
                    .initialize()
                    .await
                    .expect("Failed to initialize application state");
            });

            // Register state with Tauri
            app.manage(app_state);

            // Setup event listeners
            setup_event_listeners(app.handle().clone());

            Ok(())
        })
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { .. } = event {
                if window.label() == "main" {
                    let app = window.app_handle();
                    for (label, win) in app.webview_windows() {
                        if label.starts_with("break-overlay-") {
                            let _ = win.close();
                        }
                    }
                    app.exit(0);
                }
            }
        })
        .invoke_handler(tauri::generate_handler![
            // New commands using dependency injection
            commands::timer::start_work,
            commands::timer::continue_to_next_session,
            commands::timer::end_cycle,
            commands::timer::emergency_stop,
            commands::timer::sync_settings,
            commands::timer::get_full_state,
            commands::timer::get_timer_status,
            commands::timer::record_skip_attempt,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

#[cfg(test)]
mod tests;
