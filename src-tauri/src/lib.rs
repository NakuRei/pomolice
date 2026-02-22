// Pomolice - Pomodoro Timer Application
// Backend is the single source of truth for all timer state
// Security: Window control is managed through timer_state module

use tauri::{AppHandle, Emitter, Manager};
use tracing::{error, info};

mod commands;
mod error;
mod managers;
mod state;
mod tray;
mod tray_icon_renderer;
mod types;

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

        (completed, payload)
    };

    // Emit tick event to frontend
    let _ = app.emit("timer_tick", &tick_payload);
    tray::update_tray_icon(&app, &tick_payload);

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
            // Work session completed - transition to break.
            // The break timer is driven by the same task that just fired this
            // completion, so we reuse it (no cancel/respawn needed here).
            let (_break_type, payload) = {
                let mut timer = state_handle.timer_manager.lock().await;
                timer.transition_to_break()
            };

            let _ = app.emit("session_completed", &payload);

            // Update tray icon for break state
            {
                let timer = state_handle.timer_manager.lock().await;
                tray::update_tray_icon(&app, &timer.get_state().to_tick_payload());
            }

            // Save state
            {
                let timer = state_handle.timer_manager.lock().await;
                state_handle.persistence.save_state(timer.get_state());
            }

            // Enter fullscreen break mode
            enter_break_mode(app.clone()).await;
        }
        SessionType::ShortBreak | SessionType::LongBreak => {
            // Break session completed - transition to idle and stop the task.
            //
            // IMPORTANT: This code runs INSIDE the timer task spawned by
            // spawn_timer_task() (via lib_handle_timer_tick → here).
            // cancel_and_take_task() cancels the token and extracts the
            // JoinHandle, but we must NOT await that handle because it is
            // our own task — awaiting it would deadlock (self-join).
            // The task will exit naturally after this function returns:
            // the select! loop checks token.is_cancelled() and breaks.
            let (completed_payload, tick_payload) = {
                let mut timer = state_handle.timer_manager.lock().await;
                // Cancel token and drop the JoinHandle (do not await)
                let _ = timer.cancel_and_take_task();
                let payload = timer.transition_to_idle_after_break();
                let tick = timer.get_state().to_tick_payload();
                (payload, tick)
            };

            let _ = app.emit("session_completed", &completed_payload);
            let _ = app.emit("timer_tick", &tick_payload);
            tray::update_tray_icon(&app, &tick_payload);

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

    // Update state and notify frontend that fullscreen break ended
    let tick_payload = {
        let mut timer = state_handle.timer_manager.lock().await;
        timer.set_fullscreen_break(false);
        timer.get_state().to_tick_payload()
    };

    // Emit timer_tick with is_fullscreen_break: false so the frontend
    // switches from FullscreenBreak back to the normal timer view.
    let _ = app.emit("timer_tick", &tick_payload);
    tray::update_tray_icon(&app, &tick_payload);

    let _ = app.emit("break_completed", serde_json::json!({}));
}

/// Handle input detected during break
pub async fn lib_handle_input_detected(app: AppHandle) {
    let state_handle = app.state::<state::AppState>();

    let tick_payload = {
        let mut timer = state_handle.timer_manager.lock().await;
        let payload = timer.apply_input_penalty().ok().flatten();
        // Save state
        state_handle.persistence.save_state(timer.get_state());
        payload
    };

    if let Some(ref payload) = tick_payload {
        let _ = app.emit("timer_tick", payload);
        tray::update_tray_icon(&app, payload);
    }
}

/// Handle escape combo detection
pub async fn lib_handle_escape_combo(app: AppHandle) {
    let state_handle = app.state::<state::AppState>();

    // Lock ordering: input_monitor first, then window_manager (matches exit_break_mode)
    {
        let mut input_monitor = state_handle.input_monitor.lock().await;
        let _ = input_monitor.stop();
    }

    {
        let mut window_manager = state_handle.window_manager.lock().await;
        let _ = window_manager
            .exit_fullscreen_all_monitors(app.clone())
            .await;
    }
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
                let tick_payload = app_state
                    .initialize()
                    .await
                    .expect("Failed to initialize application state");

                // Emit restored state to frontend if the timer was running when app closed
                if let Some(payload) = tick_payload {
                    let _ = app.emit("timer_tick", &payload);
                }
            });

            // Register state with Tauri
            app.manage(app_state);

            // Initialize system tray icon
            if let Err(e) = tray::setup_tray(app.handle()) {
                error!("Failed to setup tray icon: {:?}", e);
            }

            Ok(())
        })
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                if window.label() == "main" {
                    // Hide to tray instead of closing
                    api.prevent_close();
                    let _ = window.hide();

                    // Show a notification the first time the window is minimized to tray
                    if tray::should_show_tray_notification() {
                        info!("Window minimized to tray (first time notification)");
                        use tauri_plugin_notification::NotificationExt;
                        let _ = window
                            .app_handle()
                            .notification()
                            .builder()
                            .title("Pomolice")
                            .body("Pomolice is still running in the system tray.")
                            .show();
                    }
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
