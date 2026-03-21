use tauri::{Emitter, Manager};
use tracing::{error, info};

mod commands;
mod error;
mod managers;
mod scheduler;
mod session_pipeline;
mod state;
mod tray;
mod tray_icon_renderer;
mod types;

pub use scheduler::lib_start_scheduler_task;
#[cfg(test)]
pub(crate) use scheduler::{extract_boundary_epoch_seconds, trim_old_processed_boundaries};
pub use session_pipeline::{
    lib_handle_escape_combo, lib_handle_input_detected, lib_handle_session_complete,
    lib_handle_timer_tick,
};

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
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
            let app_state = state::AppState::new(app.handle().clone());

            tauri::async_runtime::block_on(async {
                let tick_payload = app_state
                    .initialize()
                    .await
                    .expect("Failed to initialize application state");

                if let Some(payload) = tick_payload {
                    let _ = app.emit("timer_tick", &payload);
                }
            });

            app.manage(app_state);

            if let Err(e) = tray::setup_tray(app.handle()) {
                error!("Failed to setup tray icon: {:?}", e);
            }

            let scheduler_app_handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                lib_start_scheduler_task(scheduler_app_handle).await;
            });

            Ok(())
        })
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                if window.label() == "main" {
                    api.prevent_close();
                    let _ = window.hide();

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
            commands::timer::start_work,
            commands::timer::resolve_post_break_decision,
            commands::timer::emergency_stop,
            commands::timer::sync_timer_duration_settings,
            commands::timer::sync_automation_policies,
            commands::timer::get_full_state,
            commands::timer::resolve_boot_auto_start_prompt,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

#[cfg(test)]
mod tests;
