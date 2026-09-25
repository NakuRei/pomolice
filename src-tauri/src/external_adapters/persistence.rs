// Persistence Manager
// Saves and loads timer state without global statics

use atomic_write_file::AtomicWriteFile;
use serde::Deserialize;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use thiserror::Error;

use crate::logging_event::{log_error_event, log_info_event, log_warn_event, BackendLogEvent};
use crate::types::{PersistedState, PERSISTED_STATE_SCHEMA_VERSION};

#[cfg(test)]
type InjectedSaveErrorHook = Arc<dyn Fn() -> Option<SaveError> + Send + Sync>;

#[cfg(test)]
type BeforePublishSaveErrorHook = Arc<dyn Fn(&Path) -> Option<SaveError> + Send + Sync>;

#[derive(Debug, Clone, Copy, Deserialize)]
struct PersistedSchemaEnvelope {
    schema_version: u32,
}

pub(crate) enum PersistenceLoadOutcome {
    NoState,
    Restored(PersistedState),
    UnsupportedSchemaFallback,
}

#[derive(Debug, Error)]
pub(crate) enum SaveError {
    #[error("failed to create persistence directory for {path}: {source}")]
    CreateDir {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("failed to serialize persisted timer state: {source}")]
    Serialize {
        #[source]
        source: serde_json::Error,
    },

    #[error("failed to open persistence file {path}: {source}")]
    Open {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("failed to write persistence file {path}: {source}")]
    Write {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("failed to commit persistence file {path}: {source}")]
    Commit {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
}

/// Manager for state persistence.
#[derive(Clone)]
pub(crate) struct PersistenceManager {
    state_file_path: Option<PathBuf>,
    save_lock: Arc<Mutex<()>>,
    #[cfg(test)]
    injected_save_error_hook: Option<InjectedSaveErrorHook>,
    #[cfg(test)]
    before_publish_save_error_hook: Option<BeforePublishSaveErrorHook>,
}

impl PersistenceManager {
    pub(crate) fn new(app_data_dir: Option<PathBuf>) -> Self {
        let state_file_path = app_data_dir.map(|path| path.join("timer_state.json"));
        Self {
            state_file_path,
            save_lock: Arc::new(Mutex::new(())),
            #[cfg(test)]
            injected_save_error_hook: None,
            #[cfg(test)]
            before_publish_save_error_hook: None,
        }
    }

    pub(crate) fn save_state_sync(
        &self,
        persisted_state: &PersistedState,
    ) -> Result<(), SaveError> {
        Self::save_state_sync_with_context(
            self.state_file_path.as_deref(),
            self.save_lock.as_ref(),
            #[cfg(test)]
            self.injected_save_error_hook.as_ref(),
            #[cfg(test)]
            self.before_publish_save_error_hook.as_ref(),
            persisted_state,
        )
    }

    pub(crate) fn load_state_sync(&self) -> PersistenceLoadOutcome {
        Self::load_state_sync_with_context(self.state_file_path.as_deref())
    }

    fn save_state_sync_with_context(
        state_file_path: Option<&Path>,
        save_lock: &Mutex<()>,
        #[cfg(test)] injected_save_error_hook: Option<&InjectedSaveErrorHook>,
        #[cfg(test)] before_publish_save_error_hook: Option<&BeforePublishSaveErrorHook>,
        persisted_state: &PersistedState,
    ) -> Result<(), SaveError> {
        let Some(path) = state_file_path else {
            log_warn_event(BackendLogEvent::StoragePersistencePathUnavailable, &[]);
            return Ok(());
        };

        log_info_event(
            BackendLogEvent::StoragePersistenceWriteStarted,
            &[("path", path.display().to_string())],
        );

        #[cfg(test)]
        if let Some(hook) = injected_save_error_hook {
            if let Some(error) = hook() {
                log_error_event(
                    BackendLogEvent::StoragePersistenceWriteFailed,
                    &[
                        ("path", path.display().to_string()),
                        ("error_message", error.to_string()),
                    ],
                    None,
                );
                return Err(error);
            }
        }

        // The exit-time save runs outside the dispatch permit and may overlap a dispatch's
        // save; this lock keeps the two file commits from interleaving.
        let _save_guard = save_lock
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let save_result = Self::save_persisted_state_to_path(
            persisted_state,
            path,
            #[cfg(test)]
            before_publish_save_error_hook,
        );

        match save_result {
            Ok(()) => {
                log_info_event(
                    BackendLogEvent::StoragePersistenceWriteCompleted,
                    &[("path", path.display().to_string())],
                );
                Ok(())
            }
            Err(error) => {
                log_error_event(
                    BackendLogEvent::StoragePersistenceWriteFailed,
                    &[
                        ("path", path.display().to_string()),
                        ("error_message", error.to_string()),
                    ],
                    None,
                );
                Err(error)
            }
        }
    }

    fn load_state_sync_with_context(state_file_path: Option<&Path>) -> PersistenceLoadOutcome {
        let Some(path) = state_file_path else {
            log_warn_event(BackendLogEvent::StoragePersistencePathUnavailable, &[]);
            return PersistenceLoadOutcome::NoState;
        };

        Self::load_state_from_path(path)
    }

    fn save_persisted_state_to_path(
        persisted_state: &PersistedState,
        path: &Path,
        #[cfg(test)] before_publish_save_error_hook: Option<&BeforePublishSaveErrorHook>,
    ) -> Result<(), SaveError> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|source| SaveError::CreateDir {
                path: parent.to_path_buf(),
                source,
            })?;
        }

        let serialized_json = serde_json::to_string_pretty(persisted_state)
            .map_err(|source| SaveError::Serialize { source })?;

        let mut temp_file =
            AtomicWriteFile::options()
                .open(path)
                .map_err(|source| SaveError::Open {
                    path: path.to_path_buf(),
                    source,
                })?;

        temp_file
            .write_all(serialized_json.as_bytes())
            .map_err(|source| SaveError::Write {
                path: path.to_path_buf(),
                source,
            })?;

        #[cfg(test)]
        if let Some(hook) = before_publish_save_error_hook {
            if let Some(error) = hook(path) {
                return Err(error);
            }
        }

        temp_file.commit().map_err(|source| SaveError::Commit {
            path: path.to_path_buf(),
            source,
        })?;

        Ok(())
    }

    fn load_state_from_path(path: &Path) -> PersistenceLoadOutcome {
        if !path.exists() {
            log_info_event(
                BackendLogEvent::StoragePersistenceNoStateFile,
                &[("path", path.display().to_string())],
            );
            return PersistenceLoadOutcome::NoState;
        }

        log_info_event(
            BackendLogEvent::StoragePersistenceReadStarted,
            &[("path", path.display().to_string())],
        );

        let raw_json = match fs::read_to_string(path) {
            Ok(content) => content,
            Err(read_error) => {
                log_error_event(
                    BackendLogEvent::StoragePersistenceReadFailed,
                    &[("path", path.display().to_string())],
                    Some(&read_error),
                );
                return PersistenceLoadOutcome::NoState;
            }
        };

        let envelope = match serde_json::from_str::<PersistedSchemaEnvelope>(&raw_json) {
            Ok(envelope) => envelope,
            Err(deserialize_error) => {
                log_warn_event(
                    BackendLogEvent::StoragePersistenceParseFailed,
                    &[
                        ("path", path.display().to_string()),
                        ("error_message", deserialize_error.to_string()),
                    ],
                );
                return PersistenceLoadOutcome::NoState;
            }
        };

        if envelope.schema_version != PERSISTED_STATE_SCHEMA_VERSION {
            log_warn_event(
                BackendLogEvent::StoragePersistenceSchemaVersionUnsupported,
                &[
                    ("schema_version", envelope.schema_version.to_string()),
                    (
                        "expected_schema_version",
                        PERSISTED_STATE_SCHEMA_VERSION.to_string(),
                    ),
                ],
            );
            return PersistenceLoadOutcome::UnsupportedSchemaFallback;
        }

        let persisted_state = match serde_json::from_str::<PersistedState>(&raw_json) {
            Ok(parsed) => parsed,
            Err(deserialize_error) => {
                log_warn_event(
                    BackendLogEvent::StoragePersistenceParseFailed,
                    &[
                        ("path", path.display().to_string()),
                        ("error_message", deserialize_error.to_string()),
                    ],
                );
                return PersistenceLoadOutcome::NoState;
            }
        };

        log_info_event(BackendLogEvent::StoragePersistenceLoaded, &[]);
        PersistenceLoadOutcome::Restored(persisted_state)
    }

    #[cfg(test)]
    pub(crate) fn with_injected_save_error_hook(mut self, hook: InjectedSaveErrorHook) -> Self {
        self.injected_save_error_hook = Some(hook);
        self
    }

    #[cfg(test)]
    pub(crate) fn with_before_publish_save_error_hook(
        mut self,
        hook: BeforePublishSaveErrorHook,
    ) -> Self {
        self.before_publish_save_error_hook = Some(hook);
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{
        DayAutoRunSlot, HhMm, PostBreakTransitionPolicy, ScheduleStopState, SessionType,
        TimerSettings, TimerStatus, WeeklyAutoRunConfig,
    };
    use chrono::{DateTime, TimeZone, Utc};
    use serde_json::json;
    use std::sync::{
        atomic::{AtomicUsize, Ordering},
        Barrier,
    };
    use std::thread;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn create_temp_dir(test_name: &str) -> PathBuf {
        let timestamp_nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time should be after unix epoch")
            .as_nanos();

        let path = std::env::temp_dir().join(format!(
            "pomolice-persistence-{}-{}-{}",
            test_name,
            std::process::id(),
            timestamp_nanos
        ));
        fs::create_dir_all(&path).expect("temp directory should be created");
        path
    }

    fn cleanup_temp_dir(path: &Path) {
        let _ = fs::remove_dir_all(path);
    }

    fn create_utc_datetime(
        year: i32,
        month: u32,
        day: u32,
        hour: u32,
        minute: u32,
        second: u32,
    ) -> DateTime<Utc> {
        Utc.with_ymd_and_hms(year, month, day, hour, minute, second)
            .single()
            .expect("valid UTC datetime")
    }

    fn default_weekly_config_json() -> serde_json::Value {
        json!({
            "mon": { "enabled": false, "start_time": "09:00", "end_time": "18:00" },
            "tue": { "enabled": false, "start_time": "09:00", "end_time": "18:00" },
            "wed": { "enabled": false, "start_time": "09:00", "end_time": "18:00" },
            "thu": { "enabled": false, "start_time": "09:00", "end_time": "18:00" },
            "fri": { "enabled": false, "start_time": "09:00", "end_time": "18:00" },
            "sat": { "enabled": false, "start_time": "09:00", "end_time": "18:00" },
            "sun": { "enabled": false, "start_time": "09:00", "end_time": "18:00" }
        })
    }

    fn hh_mm(hour: u8, minute: u8) -> HhMm {
        HhMm::try_new(hour, minute).expect("contract time should be valid")
    }

    fn contract_weekly_auto_run_config() -> WeeklyAutoRunConfig {
        WeeklyAutoRunConfig::try_new(
            DayAutoRunSlot::try_new(true, hh_mm(8, 30), hh_mm(12, 15))
                .expect("contract schedule slot should be valid"),
            DayAutoRunSlot::default(),
            DayAutoRunSlot::default(),
            DayAutoRunSlot::default(),
            DayAutoRunSlot::default(),
            DayAutoRunSlot::default(),
            DayAutoRunSlot::default(),
        )
        .expect("contract schedule should be valid")
    }

    fn contract_persisted_state(saved_at: DateTime<Utc>) -> PersistedState {
        PersistedState {
            schema_version: PERSISTED_STATE_SCHEMA_VERSION,
            status: TimerStatus::Working,
            remaining_seconds: 1234,
            session_type: SessionType::Work,
            current_cycle: 3,
            completed_pomodoros: 2,
            post_break_decision_pending: false,
            settings: TimerSettings::try_from_raw(
                2700,
                420,
                1200,
                3,
                PostBreakTransitionPolicy::StartNextWorkAutomatically,
            )
            .expect("contract settings should be valid"),
            saved_at,
            auto_run_feature_enabled: true,
            weekly_auto_run_config: contract_weekly_auto_run_config(),
            schedule_stop_state: ScheduleStopState::StopAtNextSessionBoundary,
        }
    }

    fn alternate_persisted_state(saved_at: DateTime<Utc>) -> PersistedState {
        PersistedState {
            schema_version: PERSISTED_STATE_SCHEMA_VERSION,
            status: TimerStatus::Working,
            remaining_seconds: 678,
            session_type: SessionType::Work,
            current_cycle: 9,
            completed_pomodoros: 5,
            post_break_decision_pending: false,
            settings: TimerSettings::try_from_raw(
                1800,
                600,
                1500,
                5,
                PostBreakTransitionPolicy::RequireManualContinue,
            )
            .expect("alternate contract settings should be valid"),
            saved_at,
            auto_run_feature_enabled: false,
            weekly_auto_run_config: WeeklyAutoRunConfig::default(),
            schedule_stop_state: ScheduleStopState::NotControlledBySchedule,
        }
    }

    fn contract_persisted_state_json(saved_at_utc: DateTime<Utc>) -> serde_json::Value {
        json!({
            "schema_version": PERSISTED_STATE_SCHEMA_VERSION,
            "status": "working",
            "remaining_seconds": 1234,
            "session_type": "work",
            "current_cycle": 3,
            "completed_pomodoros": 2,
            "post_break_decision_pending": false,
            "settings": {
                "work_duration_secs": 2700,
                "short_break_duration_secs": 420,
                "long_break_duration_secs": 1200,
                "long_break_interval": 3,
                "post_break_transition_policy": "start_next_work_automatically"
            },
            "saved_at": saved_at_utc,
            "auto_run_feature_enabled": true,
            "weekly_auto_run_config": {
                "mon": { "enabled": true, "start_time": "08:30", "end_time": "12:15" },
                "tue": { "enabled": false, "start_time": "09:00", "end_time": "18:00" },
                "wed": { "enabled": false, "start_time": "09:00", "end_time": "18:00" },
                "thu": { "enabled": false, "start_time": "09:00", "end_time": "18:00" },
                "fri": { "enabled": false, "start_time": "09:00", "end_time": "18:00" },
                "sat": { "enabled": false, "start_time": "09:00", "end_time": "18:00" },
                "sun": { "enabled": false, "start_time": "09:00", "end_time": "18:00" }
            },
            "schedule_stop_state": "stop_at_next_session_boundary"
        })
    }

    fn base_persisted_state_json(
        schema_version: u32,
        saved_at_utc: DateTime<Utc>,
    ) -> serde_json::Value {
        json!({
            "schema_version": schema_version,
            "status": "working",
            "remaining_seconds": 1200,
            "session_type": "work",
            "current_cycle": 2,
            "completed_pomodoros": 1,
            "post_break_decision_pending": false,
            "settings": {
                "work_duration_secs": 1500,
                "short_break_duration_secs": 300,
                "long_break_duration_secs": 900,
                "long_break_interval": 4,
                "post_break_transition_policy": "require_manual_continue"
            },
            "saved_at": saved_at_utc,
            "auto_run_feature_enabled": true,
            "weekly_auto_run_config": default_weekly_config_json(),
            "schedule_stop_state": "stop_at_next_session_boundary"
        })
    }

    fn write_timer_state_json(temp_dir: &Path, persisted_json: &serde_json::Value) {
        fs::write(
            temp_dir.join("timer_state.json"),
            serde_json::to_string_pretty(persisted_json).expect("json should serialize"),
        )
        .expect("json should be writable");
    }

    fn write_timer_state_raw(temp_dir: &Path, persisted_content: &str) {
        fs::write(temp_dir.join("timer_state.json"), persisted_content)
            .expect("state file content should be writable");
    }

    fn load_restored(persistence: &PersistenceManager) -> PersistedState {
        match persistence.load_state_sync() {
            PersistenceLoadOutcome::Restored(persisted_state) => persisted_state,
            PersistenceLoadOutcome::NoState => panic!("expected restored state, got no state"),
            PersistenceLoadOutcome::UnsupportedSchemaFallback => {
                panic!("expected restored state, got unsupported schema fallback")
            }
        }
    }

    fn build_one_shot_before_publish_save_failure_hook() -> BeforePublishSaveErrorHook {
        let call_count = Arc::new(AtomicUsize::new(0));

        Arc::new(move |path| {
            let call_index = call_count.fetch_add(1, Ordering::AcqRel);
            if call_index == 0 {
                Some(SaveError::Write {
                    path: path.to_path_buf(),
                    source: std::io::Error::other("injected before-publish save failure"),
                })
            } else {
                None
            }
        })
    }

    #[test]
    fn test_save_state_sync_writes_persisted_state_contract() {
        let temp_dir = create_temp_dir("save-contract");
        let saved_at_utc = create_utc_datetime(2025, 2, 1, 9, 0, 0);
        let persistence = PersistenceManager::new(Some(temp_dir.clone()));

        persistence
            .save_state_sync(&contract_persisted_state(saved_at_utc))
            .expect("sync save should succeed");

        let raw_json = fs::read_to_string(temp_dir.join("timer_state.json"))
            .expect("saved json should be readable");
        let parsed_json: serde_json::Value =
            serde_json::from_str(&raw_json).expect("saved json should be valid");

        assert_eq!(parsed_json, contract_persisted_state_json(saved_at_utc));

        cleanup_temp_dir(&temp_dir);
    }

    #[test]
    fn test_load_state_sync_restores_persisted_state_contract() {
        let temp_dir = create_temp_dir("load-contract");
        let saved_at_utc = create_utc_datetime(2025, 2, 1, 9, 0, 0);
        let persistence = PersistenceManager::new(Some(temp_dir.clone()));

        write_timer_state_json(&temp_dir, &contract_persisted_state_json(saved_at_utc));

        let restored = load_restored(&persistence);
        assert_eq!(restored.saved_at, saved_at_utc);
        assert_eq!(restored.status, TimerStatus::Working);
        assert_eq!(restored.remaining_seconds, 1234);
        assert_eq!(restored.current_cycle, 3);
        assert_eq!(restored.completed_pomodoros, 2);
        assert_eq!(restored.settings.work_duration_secs.get(), 2700);
        assert_eq!(
            restored.weekly_auto_run_config,
            contract_weekly_auto_run_config()
        );
        assert_eq!(
            restored.schedule_stop_state,
            ScheduleStopState::StopAtNextSessionBoundary
        );

        cleanup_temp_dir(&temp_dir);
    }

    #[test]
    fn test_save_then_load_state_sync_round_trips_all_persisted_fields() {
        let temp_dir = create_temp_dir("save-load-contract");
        let saved_at_utc = create_utc_datetime(2025, 2, 1, 9, 0, 0);
        let persistence = PersistenceManager::new(Some(temp_dir.clone()));

        persistence
            .save_state_sync(&contract_persisted_state(saved_at_utc))
            .expect("sync save should succeed");

        let restored = load_restored(&persistence);
        assert_eq!(
            serde_json::to_value(&restored).expect("state should serialize"),
            contract_persisted_state_json(saved_at_utc)
        );

        cleanup_temp_dir(&temp_dir);
    }

    #[test]
    fn test_failed_save_preserves_existing_persisted_state() {
        let temp_dir = create_temp_dir("failed-save-preserves-existing-state");
        let saved_at_utc = create_utc_datetime(2025, 2, 1, 9, 0, 0);
        let attempted_saved_at_utc = create_utc_datetime(2025, 2, 1, 9, 1, 0);
        let persistence = PersistenceManager::new(Some(temp_dir.clone()))
            .with_before_publish_save_error_hook(build_one_shot_before_publish_save_failure_hook());

        write_timer_state_json(&temp_dir, &contract_persisted_state_json(saved_at_utc));

        let save_result =
            persistence.save_state_sync(&alternate_persisted_state(attempted_saved_at_utc));

        assert!(save_result.is_err());
        let restored = load_restored(&persistence);
        assert_eq!(restored.saved_at, saved_at_utc);
        assert_eq!(restored.remaining_seconds, 1234);

        cleanup_temp_dir(&temp_dir);
    }

    #[test]
    fn test_failed_initial_save_does_not_publish_partial_state() {
        let temp_dir = create_temp_dir("failed-initial-save-no-partial-state");
        let saved_at_utc = create_utc_datetime(2025, 2, 1, 9, 0, 0);
        let persistence = PersistenceManager::new(Some(temp_dir.clone()))
            .with_before_publish_save_error_hook(build_one_shot_before_publish_save_failure_hook());

        let save_result = persistence.save_state_sync(&contract_persisted_state(saved_at_utc));

        assert!(save_result.is_err());
        assert!(matches!(
            persistence.load_state_sync(),
            PersistenceLoadOutcome::NoState
        ));

        cleanup_temp_dir(&temp_dir);
    }

    #[test]
    fn test_save_succeeds_after_failed_save_attempt() {
        let temp_dir = create_temp_dir("save-succeeds-after-failed-attempt");
        let saved_at_utc = create_utc_datetime(2025, 2, 1, 9, 0, 0);
        let successful_saved_at_utc = create_utc_datetime(2025, 2, 1, 9, 1, 0);
        let persistence = PersistenceManager::new(Some(temp_dir.clone()))
            .with_before_publish_save_error_hook(build_one_shot_before_publish_save_failure_hook());

        write_timer_state_json(&temp_dir, &contract_persisted_state_json(saved_at_utc));

        let first_save_result =
            persistence.save_state_sync(&alternate_persisted_state(successful_saved_at_utc));
        assert!(first_save_result.is_err());
        assert_eq!(load_restored(&persistence).saved_at, saved_at_utc);

        persistence
            .save_state_sync(&alternate_persisted_state(successful_saved_at_utc))
            .expect("sync save should succeed after a failed save attempt");

        let restored = load_restored(&persistence);
        assert_eq!(restored.saved_at, successful_saved_at_utc);
        assert_eq!(restored.remaining_seconds, 678);
        assert_eq!(restored.current_cycle, 9);

        cleanup_temp_dir(&temp_dir);
    }

    #[test]
    fn test_save_state_sync_serializes_concurrent_writes_without_corrupting_timer_state_file() {
        let temp_dir = create_temp_dir("save-sync-concurrent");
        let saved_at_utc = create_utc_datetime(2025, 2, 1, 9, 0, 0);
        let persistence = Arc::new(PersistenceManager::new(Some(temp_dir.clone())));
        let start_barrier = Arc::new(Barrier::new(3));
        let expected_states = [(301_u64, 1_u32), (907_u64, 2_u32)];

        let handles: Vec<_> = expected_states
            .into_iter()
            .map(|(remaining_seconds, current_cycle)| {
                let persistence = Arc::clone(&persistence);
                let start_barrier = Arc::clone(&start_barrier);

                thread::spawn(move || {
                    start_barrier.wait();
                    let persisted_state = PersistedState {
                        remaining_seconds,
                        current_cycle,
                        ..alternate_persisted_state(saved_at_utc)
                    };
                    persistence.save_state_sync(&persisted_state)
                })
            })
            .collect();

        start_barrier.wait();

        for handle in handles {
            handle
                .join()
                .expect("save thread should not panic")
                .expect("concurrent sync save should succeed");
        }

        let raw_json = fs::read_to_string(temp_dir.join("timer_state.json"))
            .expect("saved json should be readable");
        let persisted_state: PersistedState =
            serde_json::from_str(&raw_json).expect("saved json should stay valid");

        assert!(
            matches!(
                (persisted_state.remaining_seconds, persisted_state.current_cycle),
                (301, 1) | (907, 2)
            ),
            "expected one complete state to win the race, got remaining_seconds={} current_cycle={}",
            persisted_state.remaining_seconds,
            persisted_state.current_cycle
        );

        cleanup_temp_dir(&temp_dir);
    }

    #[test]
    fn test_load_mismatched_schema_file_returns_unsupported_schema_fallback() {
        let temp_dir = create_temp_dir("load-mismatched-schema");
        let saved_at_utc = create_utc_datetime(2025, 2, 1, 9, 0, 0);
        let persistence = PersistenceManager::new(Some(temp_dir.clone()));

        write_timer_state_json(
            &temp_dir,
            &base_persisted_state_json(PERSISTED_STATE_SCHEMA_VERSION + 1, saved_at_utc),
        );

        assert!(matches!(
            persistence.load_state_sync(),
            PersistenceLoadOutcome::UnsupportedSchemaFallback
        ));

        cleanup_temp_dir(&temp_dir);
    }

    #[test]
    fn test_load_malformed_state_file_returns_no_state() {
        let temp_dir = create_temp_dir("malformed-state-file");
        let persistence = PersistenceManager::new(Some(temp_dir.clone()));

        write_timer_state_raw(&temp_dir, "{ invalid json");

        assert!(matches!(
            persistence.load_state_sync(),
            PersistenceLoadOutcome::NoState
        ));

        cleanup_temp_dir(&temp_dir);
    }

    #[test]
    fn test_load_empty_state_file_returns_no_state() {
        let temp_dir = create_temp_dir("empty-state-file");
        let persistence = PersistenceManager::new(Some(temp_dir.clone()));

        write_timer_state_raw(&temp_dir, "");

        assert!(matches!(
            persistence.load_state_sync(),
            PersistenceLoadOutcome::NoState
        ));

        cleanup_temp_dir(&temp_dir);
    }

    #[test]
    fn test_load_unreadable_state_path_returns_no_state() {
        let temp_dir = create_temp_dir("unreadable-state-path");
        let persistence = PersistenceManager::new(Some(temp_dir.clone()));

        fs::create_dir(temp_dir.join("timer_state.json"))
            .expect("state path fixture directory should be creatable");

        assert!(matches!(
            persistence.load_state_sync(),
            PersistenceLoadOutcome::NoState
        ));

        cleanup_temp_dir(&temp_dir);
    }

    #[test]
    fn test_load_invalid_schedule_data_returns_no_state() {
        let temp_dir = create_temp_dir("invalid-schedule");
        let saved_at_utc = create_utc_datetime(2025, 2, 1, 9, 0, 0);
        let persistence = PersistenceManager::new(Some(temp_dir.clone()));

        let mut persisted_json =
            base_persisted_state_json(PERSISTED_STATE_SCHEMA_VERSION, saved_at_utc);
        persisted_json["weekly_auto_run_config"]["mon"]["start_time"] = json!("24:00");
        write_timer_state_json(&temp_dir, &persisted_json);

        assert!(matches!(
            persistence.load_state_sync(),
            PersistenceLoadOutcome::NoState
        ));

        cleanup_temp_dir(&temp_dir);
    }

    #[test]
    fn test_load_invalid_duration_data_returns_no_state() {
        let temp_dir = create_temp_dir("invalid-duration");
        let saved_at_utc = create_utc_datetime(2025, 2, 1, 9, 0, 0);
        let persistence = PersistenceManager::new(Some(temp_dir.clone()));

        let mut persisted_json =
            base_persisted_state_json(PERSISTED_STATE_SCHEMA_VERSION, saved_at_utc);
        persisted_json["settings"]["work_duration_secs"] = json!(0);
        write_timer_state_json(&temp_dir, &persisted_json);

        assert!(matches!(
            persistence.load_state_sync(),
            PersistenceLoadOutcome::NoState
        ));

        cleanup_temp_dir(&temp_dir);
    }

    #[test]
    fn test_new_with_none_disables_persistence_deterministically() {
        let persistence = PersistenceManager::new(None);
        let saved_at_utc = create_utc_datetime(2025, 2, 1, 9, 0, 0);

        persistence
            .save_state_sync(&contract_persisted_state(saved_at_utc))
            .expect("sync save without a path should be a no-op success");
        assert!(matches!(
            persistence.load_state_sync(),
            PersistenceLoadOutcome::NoState
        ));
    }
}
