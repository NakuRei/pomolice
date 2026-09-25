use std::sync::Arc;
use tokio::sync::Mutex;

use crate::external_adapters::clock::Clock;
use crate::external_adapters::input_monitor::InputMonitorManager;
use crate::external_adapters::persistence::{PersistenceLoadOutcome, PersistenceManager};
use crate::external_adapters::window_lock::WindowStateManager;
use crate::logging_event::{log_error_event, log_info_event, BackendLogEvent};
use crate::timer_core::TimerCore;
use crate::timer_runtime::TimerRuntime;
use crate::types::PersistedState;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum AppRestoreStatus {
    NoState,
    Restored,
    UnsupportedSchemaFallback,
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
        persistence: PersistenceManager,
    ) -> (Self, AppRestoreStatus) {
        Self::build_startup_state_with_handlers(clock, persistence, || {}, || {}, || {}).await
    }

    pub(crate) async fn build_startup_state_with_handlers(
        clock: Arc<dyn Clock + Send + Sync>,
        persistence: PersistenceManager,
        on_escape_combo: impl Fn() + Send + Sync + 'static,
        on_input_detected: impl Fn() + Send + Sync + 'static,
        on_input_failure: impl Fn() + Send + Sync + 'static,
    ) -> (Self, AppRestoreStatus) {
        log_info_event(BackendLogEvent::AppStateInitializationStarted, &[]);
        let (persisted_state, restore_status) = load_persisted_state_async(&persistence).await;
        let core = TimerCore::restore(persisted_state, clock.now_local());
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
            timer: TimerRuntime::new(core, clock, persistence),
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

async fn load_persisted_state_async(
    persistence: &PersistenceManager,
) -> (Option<PersistedState>, AppRestoreStatus) {
    let persistence = persistence.clone();
    let outcome = match tokio::task::spawn_blocking(move || persistence.load_state_sync()).await {
        Ok(outcome) => outcome,
        Err(join_error) => {
            log_error_event(
                BackendLogEvent::StoragePersistenceReadFailed,
                &[],
                Some(&join_error),
            );
            PersistenceLoadOutcome::NoState
        }
    };

    match outcome {
        PersistenceLoadOutcome::NoState => {
            log_info_event(BackendLogEvent::AppStateUsingDefaults, &[]);
            (None, AppRestoreStatus::NoState)
        }
        PersistenceLoadOutcome::UnsupportedSchemaFallback => {
            log_info_event(BackendLogEvent::AppStateUsingDefaults, &[]);
            (None, AppRestoreStatus::UnsupportedSchemaFallback)
        }
        PersistenceLoadOutcome::Restored(persisted_state) => {
            log_info_event(
                BackendLogEvent::AppStateRestoredFromPersistence,
                &[
                    ("state_status", format!("{:?}", persisted_state.status)),
                    ("saved_at", persisted_state.saved_at.to_rfc3339()),
                ],
            );
            (Some(persisted_state), AppRestoreStatus::Restored)
        }
    }
}
