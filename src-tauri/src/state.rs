use std::sync::Arc;
use tokio::sync::Mutex;

use crate::external_adapters::clock::Clock;
use crate::external_adapters::input_monitor::InputMonitorManager;
use crate::external_adapters::persistence::{
    PersistedDocument, PersistenceLoadOutcome, PersistenceManager,
};
use crate::external_adapters::window_lock::WindowStateManager;
use crate::logging_event::{log_error_event, log_info_event, log_warn_event, BackendLogEvent};
use crate::settings::{PersistedSettings, SettingsFileStatus, UserSettings};
use crate::timer_core::TimerCore;
use crate::timer_runtime::TimerRuntime;
use crate::types::PersistedState;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum AppRestoreStatus {
    NoState,
    Restored,
    Discarded,
}

pub(crate) struct AppState {
    pub timer: TimerRuntime,
    pub input_monitor: Mutex<InputMonitorManager>,
    pub window_manager: Mutex<WindowStateManager>,
}

impl AppState {
    #[cfg(test)]
    pub(crate) async fn build_startup_state_for_tests(
        clock: Arc<dyn Clock + Send + Sync>,
        timer_state_persistence: PersistenceManager<PersistedState>,
        settings_persistence: PersistenceManager<PersistedSettings>,
    ) -> (Self, AppRestoreStatus) {
        Self::build_startup_state_with_handlers(
            clock,
            timer_state_persistence,
            settings_persistence,
            || {},
            || {},
            || {},
        )
        .await
    }

    pub(crate) async fn build_startup_state_with_handlers(
        clock: Arc<dyn Clock + Send + Sync>,
        timer_state_persistence: PersistenceManager<PersistedState>,
        settings_persistence: PersistenceManager<PersistedSettings>,
        on_escape_combo: impl Fn() + Send + Sync + 'static,
        on_input_detected: impl Fn() + Send + Sync + 'static,
        on_input_failure: impl Fn() + Send + Sync + 'static,
    ) -> (Self, AppRestoreStatus) {
        log_info_event(BackendLogEvent::AppStateInitializationStarted, &[]);
        let (persisted_state, restore_status) = load_document_async(&timer_state_persistence).await;
        let (persisted_settings, settings_restore_status) =
            load_document_async(&settings_persistence).await;
        let settings =
            persisted_settings.map_or_else(UserSettings::default, |persisted| persisted.settings);
        let core = TimerCore::restore(
            persisted_state,
            settings,
            if settings_restore_status == AppRestoreStatus::Discarded {
                SettingsFileStatus::Unusable
            } else {
                SettingsFileStatus::Ok
            },
            clock.now_local(),
        );
        log_info_event(
            BackendLogEvent::AppStateInitializationCompleted,
            &[
                ("restore_status", format!("{restore_status:?}")),
                (
                    "is_running",
                    (core.status() != crate::types::TimerStatus::Idle).to_string(),
                ),
            ],
        );
        log_info_event(BackendLogEvent::AppStateCreated, &[]);

        let app_state = Self {
            timer: TimerRuntime::new(core, clock, timer_state_persistence, settings_persistence),
            input_monitor: Mutex::new(InputMonitorManager::new(
                on_escape_combo,
                on_input_detected,
                on_input_failure,
            )),
            window_manager: Mutex::new(WindowStateManager::new()),
        };
        (app_state, restore_status)
    }
}

async fn load_document_async<T: PersistedDocument + Send + 'static>(
    persistence: &PersistenceManager<T>,
) -> (Option<T>, AppRestoreStatus) {
    let persistence = persistence.clone();
    let outcome = match tokio::task::spawn_blocking(move || persistence.load_sync()).await {
        Ok(outcome) => outcome,
        Err(join_error) => {
            log_error_event(
                BackendLogEvent::StoragePersistenceReadFailed,
                &[],
                Some(&join_error),
            );
            PersistenceLoadOutcome::Discarded
        }
    };

    match outcome {
        PersistenceLoadOutcome::NoState => {
            log_info_event(
                BackendLogEvent::AppStateUsingDefaults,
                &[("file_name", T::FILE_NAME.to_string())],
            );
            (None, AppRestoreStatus::NoState)
        }
        PersistenceLoadOutcome::Discarded => {
            log_warn_event(
                BackendLogEvent::AppStateDiscardedUnusableFile,
                &[("file_name", T::FILE_NAME.to_string())],
            );
            (None, AppRestoreStatus::Discarded)
        }
        PersistenceLoadOutcome::Restored(document) => {
            log_info_event(
                BackendLogEvent::AppStateRestoredFromPersistence,
                &[("file_name", T::FILE_NAME.to_string())],
            );
            (Some(document), AppRestoreStatus::Restored)
        }
    }
}
