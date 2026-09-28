use std::sync::Arc;
use tauri::{async_runtime, Emitter, Manager};
use tauri_plugin_notification::NotificationExt;

type RunResult = std::result::Result<(), Box<dyn std::error::Error>>;

mod app_effects;
mod break_lock;
mod commands;
mod error;
mod external_adapters;
mod log_redaction;
mod logging_event;
mod schedule_boundary;
mod settings;
mod state;
mod timer_core;
mod timer_effects;
mod timer_runtime;
mod tray_icon_renderer;
mod types;

pub use crate::logging_event::{log_error_event, log_info_event, log_warn_event, BackendLogEvent};
use crate::logging_event::{
    log_persistence_write_failure, PERSISTENCE_WRITE_TRIGGER_RUN_EVENT_EXIT,
};
use crate::timer_core::Event;

fn handle_window_event(window: &tauri::Window, event: &tauri::WindowEvent) {
    if let tauri::WindowEvent::CloseRequested { api, .. } = event {
        if window.label() == "main" {
            api.prevent_close();
            if !external_adapters::tray_icon::is_tray_available(window.app_handle()) {
                return;
            }
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

fn setup_app(app: &tauri::App) {
    let app_for_escape = app.handle().clone();
    let app_for_input = app.handle().clone();
    let app_for_input_failure = app.handle().clone();
    let on_escape_combo = move || {
        app_effects::dispatch_from_callback(app_for_escape.clone(), Event::EscapeComboPressed);
    };
    let on_input_detected = move || {
        app_effects::dispatch_from_callback(app_for_input.clone(), Event::InputDetected);
    };
    let clock: Arc<dyn external_adapters::clock::Clock + Send + Sync> =
        Arc::new(external_adapters::clock::SystemClock);
    let app_data_dir = app.handle().path().app_data_dir().ok();
    let timer_state_persistence =
        external_adapters::persistence::PersistenceManager::new(app_data_dir.clone());
    let settings_persistence =
        external_adapters::persistence::PersistenceManager::new(app_data_dir);
    let (app_state, restore_status) =
        async_runtime::block_on(state::AppState::build_startup_state_with_handlers(
            clock,
            timer_state_persistence,
            settings_persistence,
            on_escape_combo,
            on_input_detected,
            move || {
                let _ = app_for_input_failure.emit_to("main", "input_monitor_failed", ());
            },
        ));
    let has_persisted_state = restore_status == state::AppRestoreStatus::Restored;

    app.manage(app_state);
    external_adapters::emergency_shortcut::setup(app.handle());

    if let Err(e) = external_adapters::tray_icon::setup_tray(app.handle()) {
        log_error_event(
            BackendLogEvent::AppTraySetupFailed,
            &[("error_message", e.to_string())],
            None,
        );
    }

    let app_handle = app.handle().clone();
    async_runtime::block_on(async {
        let app_state = app_handle.state::<state::AppState>();
        // Startup can only fail on persistence, which is logged where it fails.
        let _ = app_state.timer.dispatch(&app_handle, Event::Started).await;
    });
    app_effects::spawn_ticker(app.handle().clone());

    log_info_event(
        BackendLogEvent::AppStartupCompleted,
        &[
            ("startup_phase", "setup".to_string()),
            ("has_persisted_state", has_persisted_state.to_string()),
            ("restored_state_status", format!("{restore_status:?}")),
        ],
    );
}

fn app_context<R: tauri::Runtime>() -> tauri::Context<R> {
    #[expect(
        clippy::exit,
        reason = "Tauri's generate_context!() codegen calls process::exit() if generated context creation panics"
    )]
    let context = tauri::generate_context!();
    context
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
/// Start the Tauri application and register all runtime state, handlers, and background tasks.
///
/// # Errors
/// Returns an error if Tauri application setup or runtime startup fails.
pub fn run() -> RunResult {
    let tauri_context = app_context();

    let app = tauri::Builder::default()
        // Raw Input registration interferes with rdev keyboard hooks on Windows while focused.
        .device_event_filter(tauri::DeviceEventFilter::Always)
        .plugin(external_adapters::logging::build_log_plugin())
        .plugin(tauri_plugin_notification::init())
        .setup(|app| {
            setup_app(app);
            Ok(())
        })
        .on_window_event(handle_window_event)
        .invoke_handler(commands::invoke_handler())
        .build(tauri_context)?;

    app.run(|app_handle, event| {
        if matches!(event, tauri::RunEvent::Exit) {
            let state = app_handle.state::<state::AppState>();
            if let Err(error) = state.timer.flush_sync_on_exit() {
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
    use crate::settings::{
        PersistedSettings, SettingsFileStatus, UserSettings, SETTINGS_SCHEMA_VERSION,
    };
    use crate::state;
    use crate::types::{
        PersistedState, ScheduleStopState, SessionDurationSecs, SessionType, TimerStatus,
        WorkDurationSecs, PERSISTED_STATE_SCHEMA_VERSION,
    };
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
        PersistenceManager::new(Some(dir.to_path_buf()))
            .save_sync(&PersistedState {
                schema_version: PERSISTED_STATE_SCHEMA_VERSION,
                status: TimerStatus::Working,
                remaining_seconds: 321,
                session_duration_secs: SessionDurationSecs::try_new(25 * 60)
                    .expect("valid session duration"),
                session_type: SessionType::Work,
                current_cycle: 1,
                completed_pomodoros: 0,
                post_break_decision_pending: false,
                saved_at: clock.now_utc(),
                schedule_stop_state: ScheduleStopState::NotControlledBySchedule,
            })
            .expect("seed state should be saved");

        start_app_state(dir)
    }

    fn start_app_state(dir: &Path) -> state::AppState {
        let clock = fixed_clock();
        let runtime = tokio::runtime::Runtime::new().expect("test runtime should be created");
        let (app_state, _) = runtime.block_on(state::AppState::build_startup_state_for_tests(
            as_clock_trait(&clock),
            PersistenceManager::new(Some(dir.to_path_buf())),
            PersistenceManager::new(Some(dir.to_path_buf())),
        ));
        drop(runtime);
        app_state
    }

    fn full_state_at_startup(dir: &Path) -> crate::types::FullStatePayload {
        let app_state = start_app_state(dir);
        tauri::async_runtime::block_on(app_state.timer.read_full_state()).timer
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
        std::fs::remove_file(temp_dir.join("timer_state.json"))
            .expect("seed file should be removable");

        app_state
            .timer
            .flush_sync_on_exit()
            .expect("exit event flush should succeed");

        let persistence: PersistenceManager<PersistedState> =
            PersistenceManager::new(Some(temp_dir.clone()));
        match persistence.load_sync() {
            PersistenceLoadOutcome::Restored(persisted_state) => {
                assert_eq!(persisted_state.status, TimerStatus::Working);
                assert_eq!(persisted_state.remaining_seconds, 321);
            }
            PersistenceLoadOutcome::NoState | PersistenceLoadOutcome::Discarded => {
                panic!("expected restored state")
            }
        }

        cleanup_temp_dir(&temp_dir);
    }

    #[test]
    fn test_startup_uses_saved_settings() {
        let temp_dir = create_temp_dir("startup-saved-settings");
        let settings = UserSettings {
            work_duration_secs: WorkDurationSecs::try_new(50 * 60).expect("valid work duration"),
            ..UserSettings::default()
        };
        PersistenceManager::new(Some(temp_dir.clone()))
            .save_sync(&PersistedSettings {
                schema_version: SETTINGS_SCHEMA_VERSION,
                settings: settings.clone(),
            })
            .expect("seed settings should be saved");

        let full_state = full_state_at_startup(&temp_dir);

        assert_eq!(full_state.settings, settings);
        assert_eq!(full_state.remaining_seconds, 50 * 60);
        assert_eq!(full_state.settings_file_status, SettingsFileStatus::Ok);

        cleanup_temp_dir(&temp_dir);
    }

    #[test]
    fn test_first_startup_uses_defaults_without_reporting_a_failure() {
        let temp_dir = create_temp_dir("startup-no-settings");

        let full_state = full_state_at_startup(&temp_dir);

        assert_eq!(full_state.settings, UserSettings::default());
        assert_eq!(full_state.settings_file_status, SettingsFileStatus::Ok);

        cleanup_temp_dir(&temp_dir);
    }

    #[test]
    fn test_unusable_settings_file_falls_back_to_defaults_and_is_reported() {
        let temp_dir = create_temp_dir("startup-broken-settings");
        std::fs::write(temp_dir.join("settings.json"), "{ not json")
            .expect("broken settings should be writable");

        let full_state = full_state_at_startup(&temp_dir);

        assert_eq!(full_state.settings, UserSettings::default());
        assert_eq!(
            full_state.settings_file_status,
            SettingsFileStatus::Unusable
        );
        assert_eq!(
            std::fs::read_to_string(temp_dir.join("settings.json"))
                .expect("broken settings should still exist"),
            "{ not json"
        );

        cleanup_temp_dir(&temp_dir);
    }
}
