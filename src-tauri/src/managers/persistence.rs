// Persistence Manager
// Saves and loads timer state without global statics

use atomic_write_file::AtomicWriteFile;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use thiserror::Error;

use crate::logging_event::{log_error_event, log_info_event, log_warn_event, BackendLogEvent};
use crate::managers::clock::{Clock, SystemClock};
use crate::types::{
    ScheduledStopState, SessionType, TimerSettings, TimerStartSource, TimerState, TimerStatus,
    WeeklyAutoRunConfig, PERSISTED_STATE_SCHEMA_VERSION_V4,
};

#[cfg(test)]
type BeforeSaveHook = Arc<dyn Fn() + Send + Sync>;

#[cfg(test)]
type InjectedSaveErrorHook = Arc<dyn Fn() -> Option<SaveError> + Send + Sync>;

#[derive(Debug, Clone, Copy, Deserialize)]
struct PersistedSchemaEnvelope {
    schema_version: u32,
}

/// Persisted timer state stored in `timer_state.json`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct PersistedState {
    pub schema_version: u32,
    pub status: TimerStatus,
    pub remaining_seconds: u64,
    pub session_type: SessionType,
    pub current_cycle: u32,
    pub completed_pomodoros: u32,
    #[serde(default)]
    pub post_break_decision_pending: bool,
    pub settings: TimerSettings,
    pub saved_at: DateTime<Utc>,
    #[serde(default)]
    pub auto_run_feature_enabled: bool,
    pub weekly_auto_run_config: WeeklyAutoRunConfig,
    #[serde(default)]
    pub timer_start_source: Option<TimerStartSource>,
    #[serde(default)]
    pub scheduled_stop_state: ScheduledStopState,
}

pub(crate) enum PersistenceLoadOutcome {
    NoState,
    Restored {
        timer_state: TimerState,
        saved_at_utc: DateTime<Utc>,
    },
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

struct RestoredSessionState {
    status: TimerStatus,
    session_type: SessionType,
    remaining_seconds: u64,
    current_cycle: u32,
    completed_pomodoros: u32,
    timer_start_source: Option<TimerStartSource>,
    scheduled_stop_state: ScheduledStopState,
}

fn log_restore_normalized(state: &RestoredSessionState) {
    log_warn_event(
        BackendLogEvent::StoragePersistenceRestoreNormalized,
        &[
            ("status", format!("{:?}", state.status)),
            ("session_type", format!("{:?}", state.session_type)),
            ("remaining_seconds", state.remaining_seconds.to_string()),
            ("completed_pomodoros", state.completed_pomodoros.to_string()),
        ],
    );
}

fn normalize_pending_post_break_decision(
    state: &mut RestoredSessionState,
    settings: &TimerSettings,
    post_break_decision_pending: bool,
) {
    if !post_break_decision_pending {
        return;
    }

    if state.status != TimerStatus::Idle
        || state.session_type != SessionType::Work
        || state.remaining_seconds != settings.work_duration_secs.get()
    {
        log_restore_normalized(state);
    }

    state.status = TimerStatus::Idle;
    state.session_type = SessionType::Work;
    state.remaining_seconds = settings.work_duration_secs.get();
    state.timer_start_source = None;
    state.scheduled_stop_state = ScheduledStopState::None;
}

fn normalize_restored_break_session(state: &mut RestoredSessionState, settings: &TimerSettings) {
    if state.status != TimerStatus::OnBreak {
        return;
    }

    log_restore_normalized(state);
    state.status = TimerStatus::Idle;
    state.session_type = SessionType::Work;
    state.remaining_seconds = settings.work_duration_secs.get();
    state.completed_pomodoros = state.completed_pomodoros.saturating_sub(1);
    state.timer_start_source = None;
    state.scheduled_stop_state = ScheduledStopState::None;
}

fn adjusted_remaining_seconds(state: &RestoredSessionState, elapsed_seconds: u64) -> u64 {
    debug_assert_ne!(
        state.status,
        TimerStatus::OnBreak,
        "OnBreak should be normalized before elapsed subtraction"
    );

    if state.status == TimerStatus::Working {
        state.remaining_seconds.saturating_sub(elapsed_seconds)
    } else {
        state.remaining_seconds
    }
}

impl PersistedState {
    pub(crate) fn from_timer_state(state: TimerState, clock: &dyn Clock) -> Self {
        let TimerState {
            status,
            remaining_seconds,
            session_type,
            current_cycle,
            completed_pomodoros,
            post_break_decision_pending,
            settings,
            auto_run_feature_enabled,
            weekly_auto_run_config,
            timer_start_source,
            scheduled_stop_state,
            ..
        } = state;

        Self {
            schema_version: PERSISTED_STATE_SCHEMA_VERSION_V4,
            status,
            remaining_seconds,
            session_type,
            current_cycle,
            completed_pomodoros,
            post_break_decision_pending,
            settings,
            saved_at: clock.now_utc(),
            auto_run_feature_enabled,
            weekly_auto_run_config,
            timer_start_source,
            scheduled_stop_state,
        }
    }

    pub(crate) fn elapsed_seconds_since_save(&self, clock: &dyn Clock) -> u64 {
        let now = clock.now_utc();
        let elapsed = now.signed_duration_since(self.saved_at);
        u64::try_from(elapsed.num_seconds().max(0)).unwrap_or_default()
    }

    pub(crate) fn into_timer_state(self, clock: &dyn Clock) -> TimerState {
        let elapsed_seconds = self.elapsed_seconds_since_save(clock);

        let PersistedState {
            schema_version: _,
            status,
            remaining_seconds,
            session_type,
            current_cycle,
            completed_pomodoros,
            post_break_decision_pending,
            settings,
            saved_at: _,
            auto_run_feature_enabled,
            weekly_auto_run_config,
            timer_start_source,
            scheduled_stop_state,
        } = self;

        let mut restored_state = RestoredSessionState {
            status,
            session_type,
            remaining_seconds,
            current_cycle,
            completed_pomodoros,
            timer_start_source,
            scheduled_stop_state,
        };
        normalize_pending_post_break_decision(
            &mut restored_state,
            &settings,
            post_break_decision_pending,
        );
        normalize_restored_break_session(&mut restored_state, &settings);

        let (normalized_timer_start_source, normalized_scheduled_stop_state) =
            normalize_recovered_session_stop_behavior(
                restored_state.status,
                restored_state.timer_start_source,
                restored_state.scheduled_stop_state,
            );

        TimerState {
            status: restored_state.status,
            remaining_seconds: adjusted_remaining_seconds(&restored_state, elapsed_seconds),
            session_type: restored_state.session_type,
            current_cycle: restored_state.current_cycle,
            completed_pomodoros: restored_state.completed_pomodoros,
            break_interaction_count: 0,
            last_penalty_time: None,
            settings,
            auto_run_feature_enabled,
            weekly_auto_run_config,
            timer_start_source: normalized_timer_start_source,
            scheduled_stop_state: normalized_scheduled_stop_state,
            post_break_decision_pending,
            is_fullscreen_break: false,
            last_transition: None,
        }
    }
}

fn normalize_recovered_session_stop_behavior(
    status: TimerStatus,
    timer_start_source: Option<TimerStartSource>,
    scheduled_stop_state: ScheduledStopState,
) -> (Option<TimerStartSource>, ScheduledStopState) {
    match status {
        TimerStatus::Idle => (None, ScheduledStopState::None),
        TimerStatus::Working | TimerStatus::OnBreak => (
            Some(timer_start_source.unwrap_or(TimerStartSource::ManualOutsideWindow)),
            scheduled_stop_state,
        ),
    }
}

/// Manager for state persistence.
#[derive(Clone)]
pub(crate) struct PersistenceManager {
    clock: Arc<dyn Clock + Send + Sync>,
    state_file_path: Option<PathBuf>,
    save_lock: Arc<Mutex<()>>,
    #[cfg(test)]
    before_save_hook: Option<BeforeSaveHook>,
    #[cfg(test)]
    injected_save_error_hook: Option<InjectedSaveErrorHook>,
}

impl PersistenceManager {
    pub(crate) fn new(clock: Arc<dyn Clock + Send + Sync>, app_data_dir: Option<PathBuf>) -> Self {
        let state_file_path = app_data_dir.map(|path| path.join("timer_state.json"));
        Self {
            clock,
            state_file_path,
            save_lock: Arc::new(Mutex::new(())),
            #[cfg(test)]
            before_save_hook: None,
            #[cfg(test)]
            injected_save_error_hook: None,
        }
    }

    pub(crate) fn has_state_path(&self) -> bool {
        self.state_file_path.is_some()
    }

    pub(crate) fn save_state_sync(&self, timer_state: TimerState) -> Result<(), SaveError> {
        Self::save_state_sync_with_context(
            self.clock.as_ref(),
            self.state_file_path.as_deref(),
            self.save_lock.as_ref(),
            #[cfg(test)]
            self.before_save_hook.as_ref(),
            #[cfg(test)]
            self.injected_save_error_hook.as_ref(),
            timer_state,
        )
    }

    pub(crate) fn load_state_sync(&self) -> PersistenceLoadOutcome {
        Self::load_state_sync_with_context(self.clock.as_ref(), self.state_file_path.as_deref())
    }

    fn save_state_sync_with_context(
        clock: &dyn Clock,
        state_file_path: Option<&Path>,
        save_lock: &Mutex<()>,
        #[cfg(test)] before_save_hook: Option<&BeforeSaveHook>,
        #[cfg(test)] injected_save_error_hook: Option<&InjectedSaveErrorHook>,
        timer_state: TimerState,
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
        if let Some(hook) = before_save_hook {
            hook();
        }

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

        // Production saves are already serialized by AppState's ordered-save lane. This inner lock
        // stays as a safety net for direct sync callers (mostly tests) so persistence itself never
        // overlaps atomic file commits, but it is intentionally not the public FIFO contract.
        let _save_guard = save_lock
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let save_result = Self::save_state_to_path(clock, timer_state, path);

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

    fn load_state_sync_with_context(
        clock: &dyn Clock,
        state_file_path: Option<&Path>,
    ) -> PersistenceLoadOutcome {
        let Some(path) = state_file_path else {
            log_warn_event(BackendLogEvent::StoragePersistencePathUnavailable, &[]);
            return PersistenceLoadOutcome::NoState;
        };

        Self::load_state_from_path(clock, path)
    }

    fn save_state_to_path(
        clock: &dyn Clock,
        timer_state: TimerState,
        path: &Path,
    ) -> Result<(), SaveError> {
        let persisted_state = PersistedState::from_timer_state(timer_state, clock);
        Self::save_persisted_state_to_path(&persisted_state, path)
    }

    fn save_persisted_state_to_path(
        persisted_state: &PersistedState,
        path: &Path,
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

        temp_file.commit().map_err(|source| SaveError::Commit {
            path: path.to_path_buf(),
            source,
        })?;

        Ok(())
    }

    fn load_state_from_path(clock: &dyn Clock, path: &Path) -> PersistenceLoadOutcome {
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

        if envelope.schema_version != PERSISTED_STATE_SCHEMA_VERSION_V4 {
            log_warn_event(
                BackendLogEvent::StoragePersistenceSchemaVersionUnsupported,
                &[
                    ("schema_version", envelope.schema_version.to_string()),
                    (
                        "expected_schema_version",
                        PERSISTED_STATE_SCHEMA_VERSION_V4.to_string(),
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
        let saved_at_utc = persisted_state.saved_at;
        PersistenceLoadOutcome::Restored {
            timer_state: persisted_state.into_timer_state(clock),
            saved_at_utc,
        }
    }

    #[cfg(test)]
    pub(crate) fn with_before_save_hook(mut self, hook: BeforeSaveHook) -> Self {
        self.before_save_hook = Some(hook);
        self
    }

    #[cfg(test)]
    pub(crate) fn with_injected_save_error_hook(mut self, hook: InjectedSaveErrorHook) -> Self {
        self.injected_save_error_hook = Some(hook);
        self
    }
}

impl Default for PersistenceManager {
    fn default() -> Self {
        Self::new(Arc::new(SystemClock), None)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::managers::clock::testing::FixedClock;
    use crate::types::{
        DayAutoRunSlot, HhMm, PostBreakTransitionPolicy, PERSISTED_STATE_SCHEMA_VERSION_V3,
        PERSISTED_STATE_SCHEMA_VERSION_V4,
    };
    use chrono::{Local, TimeZone};
    use serde_json::json;
    use std::sync::{Arc, Barrier};
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

    fn fixed_clock(now_utc: DateTime<Utc>) -> Arc<FixedClock> {
        Arc::new(FixedClock::new(now_utc.with_timezone(&Local)))
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

    fn enabled_day_slot(
        start_hour: u8,
        start_minute: u8,
        end_hour: u8,
        end_minute: u8,
    ) -> DayAutoRunSlot {
        DayAutoRunSlot::try_new(
            true,
            hh_mm(start_hour, start_minute),
            hh_mm(end_hour, end_minute),
        )
        .expect("contract schedule slot should be valid")
    }

    fn contract_weekly_auto_run_config() -> WeeklyAutoRunConfig {
        WeeklyAutoRunConfig::try_new(
            enabled_day_slot(8, 30, 12, 15),
            DayAutoRunSlot::default(),
            DayAutoRunSlot::default(),
            DayAutoRunSlot::default(),
            DayAutoRunSlot::default(),
            DayAutoRunSlot::default(),
            DayAutoRunSlot::default(),
        )
        .expect("contract schedule should be valid")
    }

    fn contract_timer_settings() -> TimerSettings {
        TimerSettings::try_from_raw(
            2700,
            420,
            1200,
            3,
            PostBreakTransitionPolicy::StartNextWorkAutomatically,
        )
        .expect("contract settings should be valid")
    }

    fn contract_timer_state() -> TimerState {
        TimerState {
            status: TimerStatus::Working,
            remaining_seconds: 1234,
            session_type: SessionType::Work,
            current_cycle: 3,
            completed_pomodoros: 2,
            post_break_decision_pending: false,
            settings: contract_timer_settings(),
            auto_run_feature_enabled: true,
            weekly_auto_run_config: contract_weekly_auto_run_config(),
            timer_start_source: Some(TimerStartSource::ManualInsideWindow),
            scheduled_stop_state: ScheduledStopState::Pending,
            ..TimerState::default()
        }
    }

    fn contract_weekly_config_json() -> serde_json::Value {
        json!({
            "mon": { "enabled": true, "start_time": "08:30", "end_time": "12:15" },
            "tue": { "enabled": false, "start_time": "09:00", "end_time": "18:00" },
            "wed": { "enabled": false, "start_time": "09:00", "end_time": "18:00" },
            "thu": { "enabled": false, "start_time": "09:00", "end_time": "18:00" },
            "fri": { "enabled": false, "start_time": "09:00", "end_time": "18:00" },
            "sat": { "enabled": false, "start_time": "09:00", "end_time": "18:00" },
            "sun": { "enabled": false, "start_time": "09:00", "end_time": "18:00" }
        })
    }

    fn contract_persisted_state_json(saved_at_utc: DateTime<Utc>) -> serde_json::Value {
        json!({
            "schema_version": PERSISTED_STATE_SCHEMA_VERSION_V4,
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
            "weekly_auto_run_config": contract_weekly_config_json(),
            "timer_start_source": "manual_inside_window",
            "scheduled_stop_state": "pending"
        })
    }

    fn assert_timer_settings_match_contract(settings: &TimerSettings) {
        assert_eq!(settings.work_duration_secs.get(), 2700);
        assert_eq!(settings.short_break_duration_secs.get(), 420);
        assert_eq!(settings.long_break_duration_secs.get(), 1200);
        assert_eq!(settings.long_break_interval.get(), 3);
        assert_eq!(
            settings.post_break_transition_policy,
            PostBreakTransitionPolicy::StartNextWorkAutomatically
        );
    }

    fn assert_timer_state_matches_persistence_contract(timer_state: &TimerState) {
        assert_eq!(timer_state.status, TimerStatus::Working);
        assert_eq!(timer_state.remaining_seconds, 1234);
        assert_eq!(timer_state.session_type, SessionType::Work);
        assert_eq!(timer_state.current_cycle, 3);
        assert_eq!(timer_state.completed_pomodoros, 2);
        assert!(!timer_state.post_break_decision_pending);
        assert_timer_settings_match_contract(&timer_state.settings);
        assert!(timer_state.auto_run_feature_enabled);
        assert_eq!(
            timer_state.weekly_auto_run_config,
            contract_weekly_auto_run_config()
        );
        assert_eq!(
            timer_state.timer_start_source,
            Some(TimerStartSource::ManualInsideWindow)
        );
        assert_eq!(
            timer_state.scheduled_stop_state,
            ScheduledStopState::Pending
        );
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
            "timer_start_source": "manual_inside_window",
            "scheduled_stop_state": "pending"
        })
    }

    fn working_timer_state(remaining_seconds: u64, current_cycle: u32) -> TimerState {
        TimerState {
            status: TimerStatus::Working,
            remaining_seconds,
            current_cycle,
            timer_start_source: Some(TimerStartSource::ManualInsideWindow),
            scheduled_stop_state: ScheduledStopState::Pending,
            ..TimerState::default()
        }
    }

    #[test]
    fn test_save_state_sync_writes_v4_persisted_state_contract() {
        let temp_dir = create_temp_dir("save-v4-contract");
        let saved_at_utc = create_utc_datetime(2025, 2, 1, 9, 0, 0);
        let clock = fixed_clock(saved_at_utc);
        let persistence = PersistenceManager::new(clock, Some(temp_dir.clone()));

        let state = contract_timer_state();

        persistence
            .save_state_sync(state)
            .expect("sync save should succeed");

        let raw_json = fs::read_to_string(temp_dir.join("timer_state.json"))
            .expect("saved json should be readable");
        let parsed_json: serde_json::Value =
            serde_json::from_str(&raw_json).expect("saved json should be valid");

        assert_eq!(parsed_json, contract_persisted_state_json(saved_at_utc));

        cleanup_temp_dir(&temp_dir);
    }

    #[test]
    fn test_load_state_sync_restores_v4_persisted_state_contract() {
        let temp_dir = create_temp_dir("load-v4-contract");
        let saved_at_utc = create_utc_datetime(2025, 2, 1, 9, 0, 0);
        let clock = fixed_clock(saved_at_utc);
        let persistence = PersistenceManager::new(clock, Some(temp_dir.clone()));

        fs::write(
            temp_dir.join("timer_state.json"),
            serde_json::to_string_pretty(&contract_persisted_state_json(saved_at_utc))
                .expect("contract json should serialize"),
        )
        .expect("contract json should be writable");

        let outcome = persistence.load_state_sync();

        match outcome {
            PersistenceLoadOutcome::Restored {
                timer_state,
                saved_at_utc: restored_saved_at_utc,
            } => {
                assert_eq!(restored_saved_at_utc, saved_at_utc);
                assert_timer_state_matches_persistence_contract(&timer_state);
            }
            PersistenceLoadOutcome::NoState => panic!("expected restored state, got no state"),
            PersistenceLoadOutcome::UnsupportedSchemaFallback => {
                panic!("expected restored state, got unsupported schema fallback")
            }
        }

        cleanup_temp_dir(&temp_dir);
    }

    #[test]
    fn test_load_state_sync_restores_v4_file_missing_backward_compatible_fields() {
        let temp_dir = create_temp_dir("load-v4-missing-backward-compatible-fields");
        let saved_at_utc = create_utc_datetime(2025, 2, 1, 9, 0, 0);
        let clock = fixed_clock(saved_at_utc);
        let persistence = PersistenceManager::new(clock, Some(temp_dir.clone()));
        let mut persisted_json =
            base_persisted_state_json(PERSISTED_STATE_SCHEMA_VERSION_V4, saved_at_utc);

        let persisted_object = persisted_json
            .as_object_mut()
            .expect("base persisted state json should be an object");
        persisted_object.remove("post_break_decision_pending");
        persisted_object.remove("auto_run_feature_enabled");
        persisted_object.remove("timer_start_source");
        persisted_object.remove("scheduled_stop_state");

        fs::write(
            temp_dir.join("timer_state.json"),
            serde_json::to_string_pretty(&persisted_json).expect("json should serialize"),
        )
        .expect("json should be writable");

        match persistence.load_state_sync() {
            PersistenceLoadOutcome::Restored {
                timer_state,
                saved_at_utc: restored_saved_at_utc,
            } => {
                assert_eq!(restored_saved_at_utc, saved_at_utc);
                assert_eq!(timer_state.status, TimerStatus::Working);
                assert_eq!(timer_state.remaining_seconds, 1200);
                assert_eq!(timer_state.session_type, SessionType::Work);
                assert_eq!(timer_state.current_cycle, 2);
                assert_eq!(timer_state.completed_pomodoros, 1);
                assert_eq!(timer_state.settings.work_duration_secs.get(), 1500);
                assert_eq!(timer_state.settings.short_break_duration_secs.get(), 300);
                assert_eq!(timer_state.settings.long_break_duration_secs.get(), 900);
                assert_eq!(timer_state.settings.long_break_interval.get(), 4);
                assert_eq!(
                    timer_state.settings.post_break_transition_policy,
                    PostBreakTransitionPolicy::RequireManualContinue
                );
                assert_eq!(
                    timer_state.weekly_auto_run_config,
                    WeeklyAutoRunConfig::default()
                );
                assert!(!timer_state.post_break_decision_pending);
                assert!(!timer_state.auto_run_feature_enabled);
                assert_eq!(
                    timer_state.timer_start_source,
                    Some(TimerStartSource::ManualOutsideWindow)
                );
                assert_eq!(timer_state.scheduled_stop_state, ScheduledStopState::None);
            }
            PersistenceLoadOutcome::NoState => panic!("expected restored state, got no state"),
            PersistenceLoadOutcome::UnsupportedSchemaFallback => {
                panic!("expected restored state, got unsupported schema fallback")
            }
        }

        cleanup_temp_dir(&temp_dir);
    }

    #[test]
    fn test_save_then_load_state_sync_round_trips_all_persisted_fields() {
        let temp_dir = create_temp_dir("save-load-v4-contract");
        let saved_at_utc = create_utc_datetime(2025, 2, 1, 9, 0, 0);
        let clock = fixed_clock(saved_at_utc);
        let persistence = PersistenceManager::new(clock, Some(temp_dir.clone()));

        persistence
            .save_state_sync(contract_timer_state())
            .expect("sync save should succeed");

        match persistence.load_state_sync() {
            PersistenceLoadOutcome::Restored {
                timer_state,
                saved_at_utc: restored_saved_at_utc,
            } => {
                assert_eq!(restored_saved_at_utc, saved_at_utc);
                assert_timer_state_matches_persistence_contract(&timer_state);
            }
            PersistenceLoadOutcome::NoState => panic!("expected restored state, got no state"),
            PersistenceLoadOutcome::UnsupportedSchemaFallback => {
                panic!("expected restored state, got unsupported schema fallback")
            }
        }

        cleanup_temp_dir(&temp_dir);
    }

    #[test]
    fn test_save_state_sync_serializes_concurrent_writes_without_corrupting_timer_state_file() {
        let temp_dir = create_temp_dir("save-sync-concurrent");
        let clock = fixed_clock(create_utc_datetime(2025, 2, 1, 9, 0, 0));
        let persistence = Arc::new(PersistenceManager::new(clock, Some(temp_dir.clone())));
        let start_barrier = Arc::new(Barrier::new(3));
        let expected_states = [(301_u64, 1_u32), (907_u64, 2_u32)];

        let handles: Vec<_> = expected_states
            .into_iter()
            .map(|(remaining_seconds, current_cycle)| {
                let persistence = Arc::clone(&persistence);
                let start_barrier = Arc::clone(&start_barrier);

                thread::spawn(move || {
                    start_barrier.wait();
                    persistence
                        .save_state_sync(working_timer_state(remaining_seconds, current_cycle))
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
    fn test_load_v3_file_returns_unsupported_schema_fallback() {
        let temp_dir = create_temp_dir("load-v3");
        let saved_at_utc = create_utc_datetime(2025, 2, 1, 9, 0, 0);
        let clock = fixed_clock(saved_at_utc);
        let persistence = PersistenceManager::new(clock, Some(temp_dir.clone()));

        fs::write(
            temp_dir.join("timer_state.json"),
            serde_json::to_string_pretty(&base_persisted_state_json(
                PERSISTED_STATE_SCHEMA_VERSION_V3,
                saved_at_utc,
            ))
            .expect("legacy json should serialize"),
        )
        .expect("legacy json should be writable");

        assert!(matches!(
            persistence.load_state_sync(),
            PersistenceLoadOutcome::UnsupportedSchemaFallback
        ));

        cleanup_temp_dir(&temp_dir);
    }

    #[test]
    fn test_load_invalid_v4_schedule_data_returns_no_state() {
        let temp_dir = create_temp_dir("invalid-v4-schedule");
        let saved_at_utc = create_utc_datetime(2025, 2, 1, 9, 0, 0);
        let clock = fixed_clock(saved_at_utc);
        let persistence = PersistenceManager::new(clock, Some(temp_dir.clone()));

        let mut persisted_json =
            base_persisted_state_json(PERSISTED_STATE_SCHEMA_VERSION_V4, saved_at_utc);
        persisted_json["weekly_auto_run_config"]["mon"]["start_time"] = json!("24:00");

        fs::write(
            temp_dir.join("timer_state.json"),
            serde_json::to_string_pretty(&persisted_json).expect("json should serialize"),
        )
        .expect("json should be writable");

        assert!(matches!(
            persistence.load_state_sync(),
            PersistenceLoadOutcome::NoState
        ));

        cleanup_temp_dir(&temp_dir);
    }

    #[test]
    fn test_load_invalid_v4_duration_data_returns_no_state() {
        let temp_dir = create_temp_dir("invalid-v4-duration");
        let saved_at_utc = create_utc_datetime(2025, 2, 1, 9, 0, 0);
        let clock = fixed_clock(saved_at_utc);
        let persistence = PersistenceManager::new(clock, Some(temp_dir.clone()));

        let mut persisted_json =
            base_persisted_state_json(PERSISTED_STATE_SCHEMA_VERSION_V4, saved_at_utc);
        persisted_json["settings"]["work_duration_secs"] = json!(0);

        fs::write(
            temp_dir.join("timer_state.json"),
            serde_json::to_string_pretty(&persisted_json).expect("json should serialize"),
        )
        .expect("json should be writable");

        assert!(matches!(
            persistence.load_state_sync(),
            PersistenceLoadOutcome::NoState
        ));

        cleanup_temp_dir(&temp_dir);
    }

    #[test]
    fn test_new_with_none_disables_persistence_deterministically() {
        let clock = fixed_clock(create_utc_datetime(2025, 2, 1, 9, 0, 0));
        let persistence = PersistenceManager::new(clock, None);

        persistence
            .save_state_sync(TimerState::default())
            .expect("sync save without a path should be a no-op success");
        assert!(matches!(
            persistence.load_state_sync(),
            PersistenceLoadOutcome::NoState
        ));
    }
}
