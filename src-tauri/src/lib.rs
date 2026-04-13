use tauri::Manager;
use tauri_plugin_notification::NotificationExt;

type RunResult = std::result::Result<(), Box<dyn std::error::Error>>;

mod commands;
mod error;
mod log_redaction;
mod logging;
mod logging_event;
mod managers;
mod scheduler;
mod session_pipeline;
mod state;
mod tray;
mod tray_icon_renderer;
mod types;

pub use crate::logging_event::{log_error_event, log_info_event, log_warn_event, BackendLogEvent};
pub use scheduler::lib_start_scheduler_task;
#[cfg(test)]
pub(crate) use scheduler::{extract_boundary_epoch_seconds, trim_old_processed_boundaries};
pub use session_pipeline::{
    lib_handle_escape_combo, lib_handle_input_detected, lib_handle_session_complete,
    lib_handle_timer_tick,
};

#[cfg_attr(mobile, tauri::mobile_entry_point)]
/// Start the Tauri application and register all runtime state, handlers, and background tasks.
///
/// # Errors
/// Returns an error if Tauri application setup or runtime startup fails.
pub fn run() -> RunResult {
    #[expect(
        clippy::exit,
        reason = "Tauri's generate_context!() codegen calls process::exit() if generated context creation panics"
    )]
    tauri::Builder::default()
        .plugin(logging::build_log_plugin())
        .plugin(tauri_plugin_notification::init())
        .setup(|app| {
            let app_state = state::AppState::new(app.handle());

            let initialization_outcome =
                tauri::async_runtime::block_on(async { app_state.initialize().await });
            let has_persisted_state =
                initialization_outcome.restore_status == state::AppRestoreStatus::Restored;

            app.manage(app_state);

            if let Err(e) = tray::setup_tray(app.handle()) {
                log_error_event(
                    BackendLogEvent::AppTraySetupFailed,
                    &[("error_message", e.to_string())],
                    None,
                );
            }

            tauri::async_runtime::block_on(async {
                session_pipeline::lib_handle_startup_restore(
                    app.handle().clone(),
                    initialization_outcome.clone(),
                )
                .await;
            });

            let scheduler_app_handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                lib_start_scheduler_task(scheduler_app_handle).await;
            });

            log_info_event(
                BackendLogEvent::AppStartupCompleted,
                &[
                    ("startup_phase", "setup".to_string()),
                    ("has_persisted_state", has_persisted_state.to_string()),
                    (
                        "restored_state_status",
                        format!("{:?}", initialization_outcome.restore_status),
                    ),
                ],
            );

            Ok(())
        })
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                if window.label() == "main" {
                    api.prevent_close();
                    let _ = window.hide();

                    if tray::should_show_tray_notification() {
                        log_info_event(
                            BackendLogEvent::AppWindowHiddenToTray,
                            &[("first_notification", "true".to_string())],
                        );
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
        .run(tauri::generate_context!())?;

    Ok(())
}

#[cfg(test)]
mod tests;
