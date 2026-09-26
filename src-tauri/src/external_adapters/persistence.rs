// Persistence Manager
// Saves and loads one typed JSON document without global statics

use atomic_write_file::AtomicWriteFile;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use std::fs;
use std::io::Write;
use std::marker::PhantomData;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use thiserror::Error;

use crate::logging_event::{log_error_event, log_info_event, log_warn_event, BackendLogEvent};
use crate::settings::{PersistedSettings, SETTINGS_SCHEMA_VERSION};
use crate::types::{PersistedState, PERSISTED_STATE_SCHEMA_VERSION};

#[cfg(test)]
type InjectedSaveErrorHook = Arc<dyn Fn() -> Option<SaveError> + Send + Sync>;

#[cfg(test)]
type BeforePublishSaveErrorHook = Arc<dyn Fn(&Path) -> Option<SaveError> + Send + Sync>;

/// A document stored as its own JSON file in the application data directory.
pub(crate) trait PersistedDocument: Serialize + DeserializeOwned {
    const FILE_NAME: &'static str;
    const SCHEMA_VERSION: u32;
}

impl PersistedDocument for PersistedState {
    const FILE_NAME: &'static str = "timer_state.json";
    const SCHEMA_VERSION: u32 = PERSISTED_STATE_SCHEMA_VERSION;
}

impl PersistedDocument for PersistedSettings {
    const FILE_NAME: &'static str = "settings.json";
    const SCHEMA_VERSION: u32 = SETTINGS_SCHEMA_VERSION;
}

#[derive(Debug, Clone, Copy, Deserialize)]
struct PersistedSchemaEnvelope {
    schema_version: u32,
}

pub(crate) enum PersistenceLoadOutcome<T> {
    /// No file exists yet.
    NoState,
    Restored(T),
    /// A file exists but could not be used: unreadable, malformed, from an
    /// unsupported schema, or holding values the application rejects.
    Discarded,
}

#[derive(Debug, Error)]
pub(crate) enum SaveError {
    #[error("failed to create persistence directory for {path}: {source}")]
    CreateDir {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("failed to serialize persisted document: {source}")]
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

/// Reads and writes the file of one `PersistedDocument` type.
pub(crate) struct PersistenceManager<T> {
    file_path: Option<PathBuf>,
    save_lock: Arc<Mutex<()>>,
    #[cfg(test)]
    injected_save_error_hook: Option<InjectedSaveErrorHook>,
    #[cfg(test)]
    before_publish_save_error_hook: Option<BeforePublishSaveErrorHook>,
    document: PhantomData<fn() -> T>,
}

impl<T> Clone for PersistenceManager<T> {
    fn clone(&self) -> Self {
        Self {
            file_path: self.file_path.clone(),
            save_lock: Arc::clone(&self.save_lock),
            #[cfg(test)]
            injected_save_error_hook: self.injected_save_error_hook.clone(),
            #[cfg(test)]
            before_publish_save_error_hook: self.before_publish_save_error_hook.clone(),
            document: PhantomData,
        }
    }
}

impl<T: PersistedDocument> PersistenceManager<T> {
    pub(crate) fn new(app_data_dir: Option<PathBuf>) -> Self {
        let file_path = app_data_dir.map(|path| path.join(T::FILE_NAME));
        Self {
            file_path,
            save_lock: Arc::new(Mutex::new(())),
            #[cfg(test)]
            injected_save_error_hook: None,
            #[cfg(test)]
            before_publish_save_error_hook: None,
            document: PhantomData,
        }
    }

    pub(crate) fn save_sync(&self, document: &T) -> Result<(), SaveError> {
        let Some(path) = self.file_path.as_deref() else {
            log_warn_event(BackendLogEvent::StoragePersistencePathUnavailable, &[]);
            return Ok(());
        };

        log_info_event(
            BackendLogEvent::StoragePersistenceWriteStarted,
            &[("path", path.display().to_string())],
        );

        #[cfg(test)]
        if let Some(hook) = &self.injected_save_error_hook {
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
        let _save_guard = self
            .save_lock
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let save_result = Self::save_document_to_path(
            document,
            path,
            #[cfg(test)]
            self.before_publish_save_error_hook.as_ref(),
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

    pub(crate) fn load_sync(&self) -> PersistenceLoadOutcome<T> {
        let Some(path) = self.file_path.as_deref() else {
            log_warn_event(BackendLogEvent::StoragePersistencePathUnavailable, &[]);
            return PersistenceLoadOutcome::NoState;
        };

        Self::load_document_from_path(path)
    }

    fn save_document_to_path(
        document: &T,
        path: &Path,
        #[cfg(test)] before_publish_save_error_hook: Option<&BeforePublishSaveErrorHook>,
    ) -> Result<(), SaveError> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|source| SaveError::CreateDir {
                path: parent.to_path_buf(),
                source,
            })?;
        }

        let serialized_json = serde_json::to_string_pretty(document)
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

    fn load_document_from_path(path: &Path) -> PersistenceLoadOutcome<T> {
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
                return PersistenceLoadOutcome::Discarded;
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
                return PersistenceLoadOutcome::Discarded;
            }
        };

        if envelope.schema_version != T::SCHEMA_VERSION {
            log_warn_event(
                BackendLogEvent::StoragePersistenceSchemaVersionUnsupported,
                &[
                    ("file_name", T::FILE_NAME.to_string()),
                    ("schema_version", envelope.schema_version.to_string()),
                    ("expected_schema_version", T::SCHEMA_VERSION.to_string()),
                ],
            );
            return PersistenceLoadOutcome::Discarded;
        }

        let document = match serde_json::from_str::<T>(&raw_json) {
            Ok(parsed) => parsed,
            Err(deserialize_error) => {
                log_warn_event(
                    BackendLogEvent::StoragePersistenceParseFailed,
                    &[
                        ("path", path.display().to_string()),
                        ("error_message", deserialize_error.to_string()),
                    ],
                );
                return PersistenceLoadOutcome::Discarded;
            }
        };

        log_info_event(
            BackendLogEvent::StoragePersistenceLoaded,
            &[("file_name", T::FILE_NAME.to_string())],
        );
        PersistenceLoadOutcome::Restored(document)
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
    use crate::settings::UserSettings;
    use crate::types::{ScheduleStopState, SessionDurationSecs, SessionType, TimerStatus};
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

    fn session_duration_secs(value: u64) -> SessionDurationSecs {
        SessionDurationSecs::try_new(value).expect("contract session duration should be valid")
    }

    fn contract_persisted_state(saved_at: DateTime<Utc>) -> PersistedState {
        PersistedState {
            schema_version: PERSISTED_STATE_SCHEMA_VERSION,
            status: TimerStatus::Working,
            remaining_seconds: 1234,
            session_duration_secs: session_duration_secs(2700),
            session_type: SessionType::Work,
            current_cycle: 3,
            completed_pomodoros: 2,
            post_break_decision_pending: false,
            saved_at,
            schedule_stop_state: ScheduleStopState::StopAtNextSessionBoundary,
        }
    }

    fn alternate_persisted_state(saved_at: DateTime<Utc>) -> PersistedState {
        PersistedState {
            schema_version: PERSISTED_STATE_SCHEMA_VERSION,
            status: TimerStatus::Working,
            remaining_seconds: 678,
            session_duration_secs: session_duration_secs(1800),
            session_type: SessionType::Work,
            current_cycle: 9,
            completed_pomodoros: 5,
            post_break_decision_pending: false,
            saved_at,
            schedule_stop_state: ScheduleStopState::NotControlledBySchedule,
        }
    }

    fn contract_persisted_state_json(saved_at_utc: DateTime<Utc>) -> serde_json::Value {
        json!({
            "schema_version": PERSISTED_STATE_SCHEMA_VERSION,
            "status": "working",
            "remaining_seconds": 1234,
            "session_duration_secs": 2700,
            "session_type": "work",
            "current_cycle": 3,
            "completed_pomodoros": 2,
            "post_break_decision_pending": false,
            "saved_at": saved_at_utc,
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
            "session_duration_secs": 1500,
            "session_type": "work",
            "current_cycle": 2,
            "completed_pomodoros": 1,
            "post_break_decision_pending": false,
            "saved_at": saved_at_utc,
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

    fn load_restored<T: PersistedDocument>(persistence: &PersistenceManager<T>) -> T {
        match persistence.load_sync() {
            PersistenceLoadOutcome::Restored(document) => document,
            PersistenceLoadOutcome::NoState => panic!("expected a restored document, got no file"),
            PersistenceLoadOutcome::Discarded => {
                panic!("expected a restored document, got a discarded file")
            }
        }
    }

    fn timer_state_persistence(temp_dir: &Path) -> PersistenceManager<PersistedState> {
        PersistenceManager::new(Some(temp_dir.to_path_buf()))
    }

    fn settings_persistence(temp_dir: &Path) -> PersistenceManager<PersistedSettings> {
        PersistenceManager::new(Some(temp_dir.to_path_buf()))
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
    fn test_save_sync_writes_persisted_state_contract() {
        let temp_dir = create_temp_dir("save-contract");
        let saved_at_utc = create_utc_datetime(2025, 2, 1, 9, 0, 0);
        let persistence = timer_state_persistence(&temp_dir);

        persistence
            .save_sync(&contract_persisted_state(saved_at_utc))
            .expect("sync save should succeed");

        let raw_json = fs::read_to_string(temp_dir.join("timer_state.json"))
            .expect("saved json should be readable");
        let parsed_json: serde_json::Value =
            serde_json::from_str(&raw_json).expect("saved json should be valid");

        assert_eq!(parsed_json, contract_persisted_state_json(saved_at_utc));

        cleanup_temp_dir(&temp_dir);
    }

    #[test]
    fn test_load_sync_restores_persisted_state_contract() {
        let temp_dir = create_temp_dir("load-contract");
        let saved_at_utc = create_utc_datetime(2025, 2, 1, 9, 0, 0);
        let persistence = timer_state_persistence(&temp_dir);

        write_timer_state_json(&temp_dir, &contract_persisted_state_json(saved_at_utc));

        let restored = load_restored(&persistence);
        assert_eq!(restored.saved_at, saved_at_utc);
        assert_eq!(restored.status, TimerStatus::Working);
        assert_eq!(restored.remaining_seconds, 1234);
        assert_eq!(restored.current_cycle, 3);
        assert_eq!(restored.completed_pomodoros, 2);
        assert_eq!(restored.session_duration_secs.get(), 2700);
        assert_eq!(
            restored.schedule_stop_state,
            ScheduleStopState::StopAtNextSessionBoundary
        );

        cleanup_temp_dir(&temp_dir);
    }

    #[test]
    fn test_save_then_load_sync_round_trips_all_persisted_fields() {
        let temp_dir = create_temp_dir("save-load-contract");
        let saved_at_utc = create_utc_datetime(2025, 2, 1, 9, 0, 0);
        let persistence = timer_state_persistence(&temp_dir);

        persistence
            .save_sync(&contract_persisted_state(saved_at_utc))
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
        let persistence = timer_state_persistence(&temp_dir)
            .with_before_publish_save_error_hook(build_one_shot_before_publish_save_failure_hook());

        write_timer_state_json(&temp_dir, &contract_persisted_state_json(saved_at_utc));

        let save_result = persistence.save_sync(&alternate_persisted_state(attempted_saved_at_utc));

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
        let persistence = timer_state_persistence(&temp_dir)
            .with_before_publish_save_error_hook(build_one_shot_before_publish_save_failure_hook());

        let save_result = persistence.save_sync(&contract_persisted_state(saved_at_utc));

        assert!(save_result.is_err());
        assert!(matches!(
            persistence.load_sync(),
            PersistenceLoadOutcome::NoState
        ));

        cleanup_temp_dir(&temp_dir);
    }

    #[test]
    fn test_save_succeeds_after_failed_save_attempt() {
        let temp_dir = create_temp_dir("save-succeeds-after-failed-attempt");
        let saved_at_utc = create_utc_datetime(2025, 2, 1, 9, 0, 0);
        let successful_saved_at_utc = create_utc_datetime(2025, 2, 1, 9, 1, 0);
        let persistence = timer_state_persistence(&temp_dir)
            .with_before_publish_save_error_hook(build_one_shot_before_publish_save_failure_hook());

        write_timer_state_json(&temp_dir, &contract_persisted_state_json(saved_at_utc));

        let first_save_result =
            persistence.save_sync(&alternate_persisted_state(successful_saved_at_utc));
        assert!(first_save_result.is_err());
        assert_eq!(load_restored(&persistence).saved_at, saved_at_utc);

        persistence
            .save_sync(&alternate_persisted_state(successful_saved_at_utc))
            .expect("sync save should succeed after a failed save attempt");

        let restored = load_restored(&persistence);
        assert_eq!(restored.saved_at, successful_saved_at_utc);
        assert_eq!(restored.remaining_seconds, 678);
        assert_eq!(restored.current_cycle, 9);

        cleanup_temp_dir(&temp_dir);
    }

    #[test]
    fn test_save_sync_serializes_concurrent_writes_without_corrupting_timer_state_file() {
        let temp_dir = create_temp_dir("save-sync-concurrent");
        let saved_at_utc = create_utc_datetime(2025, 2, 1, 9, 0, 0);
        let persistence = Arc::new(timer_state_persistence(&temp_dir));
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
                    persistence.save_sync(&persisted_state)
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
    fn test_load_mismatched_schema_file_is_discarded() {
        let temp_dir = create_temp_dir("load-mismatched-schema");
        let saved_at_utc = create_utc_datetime(2025, 2, 1, 9, 0, 0);
        let persistence = timer_state_persistence(&temp_dir);

        write_timer_state_json(
            &temp_dir,
            &base_persisted_state_json(PERSISTED_STATE_SCHEMA_VERSION + 1, saved_at_utc),
        );

        assert!(matches!(
            persistence.load_sync(),
            PersistenceLoadOutcome::Discarded
        ));

        cleanup_temp_dir(&temp_dir);
    }

    #[test]
    fn test_load_malformed_state_file_is_discarded() {
        let temp_dir = create_temp_dir("malformed-state-file");
        let persistence = timer_state_persistence(&temp_dir);

        write_timer_state_raw(&temp_dir, "{ invalid json");

        assert!(matches!(
            persistence.load_sync(),
            PersistenceLoadOutcome::Discarded
        ));

        cleanup_temp_dir(&temp_dir);
    }

    #[test]
    fn test_load_empty_state_file_is_discarded() {
        let temp_dir = create_temp_dir("empty-state-file");
        let persistence = timer_state_persistence(&temp_dir);

        write_timer_state_raw(&temp_dir, "");

        assert!(matches!(
            persistence.load_sync(),
            PersistenceLoadOutcome::Discarded
        ));

        cleanup_temp_dir(&temp_dir);
    }

    #[test]
    fn test_load_unreadable_state_path_is_discarded() {
        let temp_dir = create_temp_dir("unreadable-state-path");
        let persistence = timer_state_persistence(&temp_dir);

        fs::create_dir(temp_dir.join("timer_state.json"))
            .expect("state path fixture directory should be creatable");

        assert!(matches!(
            persistence.load_sync(),
            PersistenceLoadOutcome::Discarded
        ));

        cleanup_temp_dir(&temp_dir);
    }

    #[test]
    fn test_load_out_of_range_session_duration_is_discarded() {
        let temp_dir = create_temp_dir("invalid-session-duration");
        let saved_at_utc = create_utc_datetime(2025, 2, 1, 9, 0, 0);
        let persistence = timer_state_persistence(&temp_dir);

        let mut persisted_json =
            base_persisted_state_json(PERSISTED_STATE_SCHEMA_VERSION, saved_at_utc);
        persisted_json["session_duration_secs"] = json!(0);
        write_timer_state_json(&temp_dir, &persisted_json);

        assert!(matches!(
            persistence.load_sync(),
            PersistenceLoadOutcome::Discarded
        ));

        cleanup_temp_dir(&temp_dir);
    }

    #[test]
    fn test_settings_are_saved_to_their_own_file_and_restored() {
        let temp_dir = create_temp_dir("settings-round-trip");
        let persistence = settings_persistence(&temp_dir);
        let persisted_settings = PersistedSettings {
            schema_version: SETTINGS_SCHEMA_VERSION,
            settings: UserSettings::default(),
        };

        persistence
            .save_sync(&persisted_settings)
            .expect("settings save should succeed");

        assert!(temp_dir.join("settings.json").is_file());
        assert!(!temp_dir.join("timer_state.json").exists());
        assert_eq!(load_restored(&persistence), persisted_settings);

        cleanup_temp_dir(&temp_dir);
    }

    #[test]
    fn test_settings_file_breaking_a_settings_rule_is_discarded() {
        let temp_dir = create_temp_dir("settings-rule-violation");
        let persistence = settings_persistence(&temp_dir);
        let mut settings_json = serde_json::to_value(PersistedSettings {
            schema_version: SETTINGS_SCHEMA_VERSION,
            settings: UserSettings::default(),
        })
        .expect("settings should serialize");
        settings_json["settings"]["weekly_auto_run_config"]["mon"] =
            json!({ "enabled": true, "start_time": "09:00", "end_time": "09:00" });
        fs::write(
            temp_dir.join("settings.json"),
            serde_json::to_string(&settings_json).expect("json should serialize"),
        )
        .expect("settings file should be writable");

        assert!(matches!(
            persistence.load_sync(),
            PersistenceLoadOutcome::Discarded
        ));

        cleanup_temp_dir(&temp_dir);
    }

    #[test]
    fn test_missing_settings_file_is_no_state() {
        let temp_dir = create_temp_dir("settings-missing");
        let persistence = settings_persistence(&temp_dir);

        assert!(matches!(
            persistence.load_sync(),
            PersistenceLoadOutcome::NoState
        ));

        cleanup_temp_dir(&temp_dir);
    }

    #[test]
    fn test_new_with_none_disables_persistence_deterministically() {
        let persistence: PersistenceManager<PersistedState> = PersistenceManager::new(None);
        let saved_at_utc = create_utc_datetime(2025, 2, 1, 9, 0, 0);

        persistence
            .save_sync(&contract_persisted_state(saved_at_utc))
            .expect("sync save without a path should be a no-op success");
        assert!(matches!(
            persistence.load_sync(),
            PersistenceLoadOutcome::NoState
        ));
    }
}
