use std::sync::Arc;
use tauri::{async_runtime, Manager};
use tauri_plugin_notification::NotificationExt;

type RunResult = std::result::Result<(), Box<dyn std::error::Error>>;

mod app_handle_timer_effects;
mod commands;
mod error;
mod external_adapters;
mod log_redaction;
mod logging_event;
mod schedule_boundary;
mod scheduler;
mod session_pipeline;
mod state;
mod timer_state_transition;
mod tray_icon_renderer;
mod types;
mod work_start;

pub use crate::logging_event::{log_error_event, log_info_event, log_warn_event, BackendLogEvent};
use crate::logging_event::{
    log_persistence_write_failure, PERSISTENCE_WRITE_TRIGGER_RUN_EVENT_EXIT,
};
pub use scheduler::lib_start_scheduler_task;
fn handle_window_event(window: &tauri::Window, event: &tauri::WindowEvent) {
    if let tauri::WindowEvent::CloseRequested { api, .. } = event {
        if window.label() == "main" {
            api.prevent_close();
            let _ = window.hide();

            if external_adapters::tray_icon::should_show_tray_notification() {
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
}

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
    let tauri_context = tauri::generate_context!();

    let app = tauri::Builder::default()
        .plugin(external_adapters::logging::build_log_plugin())
        .plugin(tauri_plugin_notification::init())
        .setup(|app| {
            let app_for_escape = app.handle().clone();
            let app_for_input = app.handle().clone();
            let on_escape_combo = move || {
                let app = app_for_escape.clone();
                async_runtime::spawn(async move {
                    session_pipeline::lib_handle_escape_combo(app).await;
                });
            };
            let on_input_detected = move || {
                let app = app_for_input.clone();
                async_runtime::spawn(async move {
                    session_pipeline::lib_handle_input_detected(app).await;
                });
            };
            let clock: Arc<dyn external_adapters::clock::Clock + Send + Sync> =
                Arc::new(external_adapters::clock::SystemClock);
            let app_data_dir = app.handle().path().app_data_dir().ok();
            let persistence = external_adapters::persistence::PersistenceManager::new(
                Arc::clone(&clock),
                app_data_dir,
            );
            let (app_state, initialization_outcome) =
                async_runtime::block_on(state::AppState::build_startup_state_with_handlers(
                    clock,
                    persistence,
                    on_escape_combo,
                    on_input_detected,
                ));
            let has_persisted_state =
                initialization_outcome.restore_status == state::AppRestoreStatus::Restored;

            app.manage(app_state);

            if let Err(e) = external_adapters::tray_icon::setup_tray(app.handle()) {
                log_error_event(
                    BackendLogEvent::AppTraySetupFailed,
                    &[("error_message", e.to_string())],
                    None,
                );
            }

            async_runtime::block_on(async {
                session_pipeline::lib_handle_startup_restore(
                    app.handle().clone(),
                    initialization_outcome.clone(),
                )
                .await;
            });

            let scheduler_app_handle = app.handle().clone();
            async_runtime::spawn(async move {
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
        .on_window_event(handle_window_event)
        .invoke_handler(tauri::generate_handler![
            commands::timer::start_work,
            commands::timer::resolve_post_break_decision,
            commands::timer::emergency_stop,
            commands::timer::sync_timer_duration_settings,
            commands::timer::sync_automation_policies,
            commands::timer::get_full_state,
            commands::timer::resolve_boot_auto_start_prompt,
        ])
        .build(tauri_context)?;

    app.run(|app_handle, event| {
        if matches!(event, tauri::RunEvent::Exit) {
            let state = app_handle.state::<state::AppState>();
            if let Err(error) = state.flush_sync_on_exit() {
                log_persistence_write_failure(PERSISTENCE_WRITE_TRIGGER_RUN_EVENT_EXIT, &error);
            }
        }
    });

    Ok(())
}

#[cfg(test)]
#[path = "tests/mod.rs"]
mod shared_tests;

#[cfg(test)]
mod run_event_tests {
    use crate::external_adapters::clock::{testing::FixedClock, Clock};
    use crate::external_adapters::persistence::{PersistenceLoadOutcome, PersistenceManager};
    use crate::logging_event::{
        render_persistence_write_failure_message, PERSISTENCE_WRITE_TRIGGER_RUN_EVENT_EXIT,
    };
    use crate::state;
    use crate::types::{TimerState, TimerStatus};
    use chrono::{Local, TimeZone, Utc};
    use std::io;
    use std::path::{Path, PathBuf};
    use std::sync::Arc;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn create_temp_dir(test_name: &str) -> PathBuf {
        let timestamp_nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time should be after unix epoch")
            .as_nanos();

        let path = std::env::temp_dir().join(format!(
            "pomolice-lib-{test_name}-{}-{timestamp_nanos}",
            std::process::id(),
        ));
        std::fs::create_dir_all(&path).expect("temp directory should be created");
        path
    }

    fn cleanup_temp_dir(path: &Path) {
        let _ = std::fs::remove_dir_all(path);
    }

    fn fixed_clock() -> Arc<FixedClock> {
        let now_utc = Utc
            .with_ymd_and_hms(2025, 2, 1, 9, 0, 0)
            .single()
            .expect("valid UTC datetime");
        Arc::new(FixedClock::new(now_utc.with_timezone(&Local)))
    }

    fn as_clock_trait(clock: &Arc<FixedClock>) -> Arc<dyn Clock + Send + Sync> {
        Arc::<FixedClock>::clone(clock) as Arc<dyn Clock + Send + Sync>
    }

    fn build_test_app_state(dir: &Path) -> state::AppState {
        let clock = fixed_clock();
        let runtime = tokio::runtime::Runtime::new().expect("test runtime should be created");
        let (app_state, _) = runtime.block_on(state::AppState::build_startup_state_for_tests(
            as_clock_trait(&clock),
            PersistenceManager::new(as_clock_trait(&clock), Some(dir.to_path_buf())),
        ));
        drop(runtime);
        app_state
    }

    #[test]
    fn test_run_event_exit_failure_logs_persistence_trigger_context() {
        let rendered = render_persistence_write_failure_message(
            PERSISTENCE_WRITE_TRIGGER_RUN_EVENT_EXIT,
            &io::Error::other("disk full while exiting"),
        );

        assert_eq!(
            rendered,
            "event_id=storage.persistence.write_failed trigger=run_event_exit error_message=disk\\ full\\ while\\ exiting"
        );
    }

    #[test]
    fn test_run_event_exit_triggers_final_flush() {
        let temp_dir = create_temp_dir("run-event-exit-final-flush");
        let app_state = build_test_app_state(&temp_dir);

        {
            let mut timer_manager = app_state.timer_manager.blocking_lock();
            let state = TimerState {
                status: TimerStatus::Working,
                remaining_seconds: 321,
                ..TimerState::default()
            };
            let _ = timer_manager.apply_persisted_state(state);
        }

        app_state
            .flush_sync_on_exit()
            .expect("exit event flush should succeed");

        match app_state.persistence.load_state_sync() {
            PersistenceLoadOutcome::Restored { timer_state, .. } => {
                assert_eq!(timer_state.status, TimerStatus::Working);
                assert_eq!(timer_state.remaining_seconds, 321);
            }
            PersistenceLoadOutcome::NoState => panic!("expected restored state"),
            PersistenceLoadOutcome::UnsupportedSchemaFallback => {
                panic!("expected restored state")
            }
        }

        cleanup_temp_dir(&temp_dir);
    }
}
