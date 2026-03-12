// Persistence Manager
// Saves and loads timer state without global statics

use atomic_write_file::AtomicWriteFile;
use chrono::{DateTime, Utc};
use directories::ProjectDirs;
use serde::{Deserialize, Serialize};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tracing::{error, info, warn};

use crate::managers::clock::{Clock, SystemClock};
use crate::types::{
    ScheduledStopState, SessionType, TimerSettings, TimerStartSource, TimerState, TimerStatus,
    WeeklyAutoRunConfig, PERSISTED_STATE_SCHEMA_VERSION_V3,
};

/// Persisted timer state stored in `timer_state.json`.
///
/// Usage example:
/// ```rust,ignore
/// let persisted = PersistedState {
///     schema_version: 3,
///     status: TimerStatus::Working,
///     remaining_seconds: 1200,
///     session_type: SessionType::Work,
///     current_cycle: 2,
///     completed_pomodoros: 1,
///     post_break_decision_pending: false,
///     settings: TimerSettings::default(),
///     saved_at: chrono::Utc::now(),
///     auto_run_feature_enabled: true,
///     weekly_auto_run_config: WeeklyAutoRunConfig::default(),
///     timer_start_source: Some(TimerStartSource::ManualInsideWindow),
///     scheduled_stop_state: ScheduledStopState::Pending,
/// };
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PersistedState {
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
    #[serde(default)]
    pub weekly_auto_run_config: WeeklyAutoRunConfig,
    #[serde(default)]
    pub timer_start_source: Option<TimerStartSource>,
    #[serde(default)]
    pub scheduled_stop_state: ScheduledStopState,
}

impl PersistedState {
    /// Build persisted schema v3 from current runtime state.
    pub fn from_timer_state(state: &TimerState, clock: &dyn Clock) -> Self {
        Self {
            schema_version: PERSISTED_STATE_SCHEMA_VERSION_V3,
            status: state.status,
            remaining_seconds: state.remaining_seconds,
            session_type: state.session_type,
            current_cycle: state.current_cycle,
            completed_pomodoros: state.completed_pomodoros,
            post_break_decision_pending: state.post_break_decision_pending,
            settings: state.settings.clone(),
            saved_at: clock.now_utc(),
            auto_run_feature_enabled: state.auto_run_feature_enabled,
            weekly_auto_run_config: state.weekly_auto_run_config.clone(),
            timer_start_source: state.timer_start_source,
            scheduled_stop_state: state.scheduled_stop_state,
        }
    }

    /// Calculate elapsed seconds since this state was saved.
    pub fn elapsed_seconds_since_save(&self, clock: &dyn Clock) -> u64 {
        let now = clock.now_utc();
        let elapsed = now.signed_duration_since(self.saved_at);
        elapsed.num_seconds().max(0) as u64
    }

    /// Convert persisted state into `TimerState` with schema migration.
    pub fn into_timer_state(self, clock: &dyn Clock) -> TimerState {
        let elapsed_seconds = self.elapsed_seconds_since_save(clock);

        let PersistedState {
            schema_version,
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

        if schema_version != PERSISTED_STATE_SCHEMA_VERSION_V3 {
            warn!(
                schema_version,
                expected_schema_version = PERSISTED_STATE_SCHEMA_VERSION_V3,
                "Persisted schema version is unsupported. Resetting timer state to defaults."
            );
            return TimerState::default();
        }

        let mut normalized_status = status;
        let mut normalized_session_type = session_type;
        let mut normalized_remaining_seconds = remaining_seconds;
        let mut normalized_current_cycle = current_cycle;
        let mut normalized_completed_pomodoros = completed_pomodoros;
        let mut normalized_timer_start_source = timer_start_source;
        let mut normalized_scheduled_stop_state = scheduled_stop_state;

        // Manual decision pending is valid only in idle state with work preview.
        if post_break_decision_pending {
            if status != TimerStatus::Idle
                || session_type != SessionType::Work
                || remaining_seconds != settings.work_duration_secs
            {
                warn!(
                    persisted_status = ?status,
                    persisted_session_type = ?session_type,
                    persisted_remaining_seconds = remaining_seconds,
                    "Persisted post-break pending state was inconsistent. Normalizing to idle work preview."
                );
            }

            normalized_status = TimerStatus::Idle;
            normalized_session_type = SessionType::Work;
            normalized_remaining_seconds = settings.work_duration_secs;
            normalized_timer_start_source = None;
            normalized_scheduled_stop_state = ScheduledStopState::None;
        }

        if normalized_status == TimerStatus::OnBreak {
            warn!(
                persisted_session_type = ?session_type,
                persisted_remaining_seconds = remaining_seconds,
                persisted_completed_pomodoros = completed_pomodoros,
                "Persisted break session cannot be resumed safely. Resetting to idle work preview."
            );

            normalized_status = TimerStatus::Idle;
            normalized_session_type = SessionType::Work;
            normalized_remaining_seconds = settings.work_duration_secs;
            normalized_current_cycle = current_cycle;
            normalized_completed_pomodoros = completed_pomodoros.saturating_sub(1);
            normalized_timer_start_source = None;
            normalized_scheduled_stop_state = ScheduledStopState::None;
        }

        let adjusted_remaining_seconds = if normalized_status == TimerStatus::Working
            || normalized_status == TimerStatus::OnBreak
        {
            normalized_remaining_seconds.saturating_sub(elapsed_seconds)
        } else {
            normalized_remaining_seconds
        };

        let (normalized_timer_start_source, normalized_scheduled_stop_state) =
            normalize_recovered_session_stop_behavior(
                normalized_status,
                normalized_timer_start_source,
                normalized_scheduled_stop_state,
            );

        TimerState {
            status: normalized_status,
            remaining_seconds: adjusted_remaining_seconds,
            session_type: normalized_session_type,
            current_cycle: normalized_current_cycle,
            completed_pomodoros: normalized_completed_pomodoros,
            break_interaction_count: 0,
            last_penalty_time: None,
            skip_attempts: 0,
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
pub struct PersistenceManager {
    clock: Arc<dyn Clock + Send + Sync>,
}

impl PersistenceManager {
    /// Create a new persistence manager.
    pub fn new(clock: Arc<dyn Clock + Send + Sync>) -> Self {
        Self { clock }
    }

    /// Get the path to the state file.
    fn get_state_file_path() -> Option<PathBuf> {
        ProjectDirs::from("io.github", "NakuRei", "pomolice").map(|dirs| {
            let data_dir = dirs.data_dir();
            data_dir.join("timer_state.json")
        })
    }

    /// Save timer state to disk.
    pub fn save_state(&self, state: &TimerState) {
        info!("Saving timer state to disk");
        let persisted_state = PersistedState::from_timer_state(state, self.clock.as_ref());

        if let Some(path) = Self::get_state_file_path() {
            if let Err(write_error) = Self::save_persisted_state_to_path(&persisted_state, &path) {
                error!("Failed to save timer state to {:?}: {}", path, write_error);
            } else {
                info!("Successfully saved state to {:?}", path);
            }
        } else {
            warn!("Could not determine state file path");
        }
    }

    /// Load timer state from disk.
    #[allow(dead_code)]
    pub fn load_state(&self) -> Option<TimerState> {
        self.load_state_with_saved_at_utc()
            .map(|(timer_state, _saved_at_utc)| timer_state)
    }

    /// Load timer state and the persisted save timestamp.
    pub fn load_state_with_saved_at_utc(&self) -> Option<(TimerState, DateTime<Utc>)> {
        let path = Self::get_state_file_path()?;
        let persisted_state = Self::load_persisted_state_from_path(&path)?;
        let saved_at_utc = persisted_state.saved_at;
        let timer_state = persisted_state.into_timer_state(self.clock.as_ref());
        Some((timer_state, saved_at_utc))
    }

    fn save_persisted_state_to_path(
        persisted_state: &PersistedState,
        path: &Path,
    ) -> std::result::Result<(), String> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|create_dir_error| create_dir_error.to_string())?;
        }

        let serialized_json = serde_json::to_string_pretty(persisted_state)
            .map_err(|serialize_error| serialize_error.to_string())?;

        let mut temp_file = AtomicWriteFile::options()
            .open(path)
            .map_err(|open_error| open_error.to_string())?;

        temp_file
            .write_all(serialized_json.as_bytes())
            .map_err(|write_error| write_error.to_string())?;

        temp_file
            .commit()
            .map_err(|commit_error| commit_error.to_string())?;

        Ok(())
    }

    fn load_persisted_state_from_path(path: &Path) -> Option<PersistedState> {
        if !path.exists() {
            info!("No persisted state file found at {:?}", path);
            return None;
        }

        info!("Loading timer state from {:?}", path);

        let raw_json = match fs::read_to_string(path) {
            Ok(content) => content,
            Err(read_error) => {
                error!("Failed to read state file {:?}: {}", path, read_error);
                return None;
            }
        };

        let persisted_state = match serde_json::from_str::<PersistedState>(&raw_json) {
            Ok(parsed) => parsed,
            Err(deserialize_error) => {
                warn!(
                    "Persisted state parse failed. Resetting to default state on startup. path={:?}, error={}",
                    path, deserialize_error
                );
                return None;
            }
        };

        info!("Successfully loaded persisted state");
        Some(persisted_state)
    }

    #[allow(dead_code)]
    fn load_state_from_path(clock: &dyn Clock, path: &Path) -> Option<TimerState> {
        let persisted_state = Self::load_persisted_state_from_path(path)?;
        Some(persisted_state.into_timer_state(clock))
    }

    /// Clear persisted state (for testing or reset).
    #[allow(dead_code)]
    pub fn clear_state(&self) {
        if let Some(path) = Self::get_state_file_path() {
            let _ = fs::remove_file(path);
        }
    }
}

impl Default for PersistenceManager {
    fn default() -> Self {
        Self::new(Arc::new(SystemClock))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::managers::clock::FixedClock;
    use chrono::{Local, TimeZone};
    use serde_json::json;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn create_test_state_file_path(test_name: &str) -> PathBuf {
        let timestamp_nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time should be after unix epoch")
            .as_nanos();

        std::env::temp_dir().join(format!(
            "pomolice-persistence-{}-{}-{}.json",
            test_name,
            std::process::id(),
            timestamp_nanos
        ))
    }

    fn remove_test_state_file(path: &Path) {
        let _ = fs::remove_file(path);
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

    fn create_fixed_clock_for_utc_datetime(now_utc: DateTime<Utc>) -> Arc<FixedClock> {
        Arc::new(FixedClock::new(now_utc.with_timezone(&Local)))
    }

    #[test]
    fn test_elapsed_seconds_calculation() {
        let saved_at_utc = create_utc_datetime(2025, 2, 1, 9, 0, 0);
        let fixed_clock =
            create_fixed_clock_for_utc_datetime(saved_at_utc + chrono::Duration::seconds(30));

        let persisted = PersistedState {
            schema_version: PERSISTED_STATE_SCHEMA_VERSION_V3,
            status: TimerStatus::Working,
            remaining_seconds: 100,
            session_type: SessionType::Work,
            current_cycle: 1,
            completed_pomodoros: 0,
            post_break_decision_pending: false,
            settings: TimerSettings::default(),
            saved_at: saved_at_utc,
            auto_run_feature_enabled: false,
            weekly_auto_run_config: WeeklyAutoRunConfig::default(),
            timer_start_source: Some(TimerStartSource::ManualOutsideWindow),
            scheduled_stop_state: ScheduledStopState::None,
        };

        let elapsed = persisted.elapsed_seconds_since_save(fixed_clock.as_ref());
        assert_eq!(elapsed, 30);
    }

    #[test]
    fn test_state_file_path() {
        let path = PersistenceManager::get_state_file_path();
        assert!(path.is_some());
        assert!(path
            .expect("state file path should exist")
            .ends_with("timer_state.json"));
    }

    #[test]
    fn test_save_state_writes_schema_version_three() {
        let path = create_test_state_file_path("save_schema_v3");
        let fixed_clock =
            create_fixed_clock_for_utc_datetime(create_utc_datetime(2025, 2, 1, 9, 0, 0));

        let mut state = TimerState::default();
        state.status = TimerStatus::Working;
        state.auto_run_feature_enabled = true;
        state.timer_start_source = Some(TimerStartSource::ManualInsideWindow);
        state.scheduled_stop_state = ScheduledStopState::Pending;

        let persisted_state = PersistedState::from_timer_state(&state, fixed_clock.as_ref());
        PersistenceManager::save_persisted_state_to_path(&persisted_state, &path)
            .expect("state should be written");

        let raw_json = fs::read_to_string(&path).expect("saved json should be readable");
        let parsed_json: serde_json::Value =
            serde_json::from_str(&raw_json).expect("saved json should be valid");

        assert_eq!(
            parsed_json["schema_version"],
            json!(PERSISTED_STATE_SCHEMA_VERSION_V3)
        );
        assert_eq!(
            parsed_json["timer_start_source"],
            json!("manual_inside_window")
        );
        assert_eq!(parsed_json["scheduled_stop_state"], json!("pending"));

        remove_test_state_file(&path);
    }

    #[test]
    fn test_load_state_schema_v2_resets_to_default_state() {
        let path = create_test_state_file_path("load_schema_v2");
        let saved_at_utc = create_utc_datetime(2025, 2, 1, 9, 0, 0);
        let fixed_clock = create_fixed_clock_for_utc_datetime(saved_at_utc);

        let legacy_schema_json = json!({
            "schema_version": 2,
            "status": "working",
            "remaining_seconds": 1200,
            "session_type": "work",
            "current_cycle": 2,
            "completed_pomodoros": 1,
            "post_break_decision_pending": true,
            "settings": {
                "work_duration_secs": 1500,
                "short_break_duration_secs": 300,
                "long_break_duration_secs": 900,
                "long_break_interval": 4,
                "post_break_transition_policy": "start_next_work_automatically"
            },
            "saved_at": saved_at_utc,
            "auto_run_feature_enabled": true,
            "weekly_auto_run_config": {
                "mon": { "enabled": true, "start_time": "09:00", "end_time": "18:00" },
                "tue": { "enabled": false, "start_time": "09:00", "end_time": "18:00" },
                "wed": { "enabled": false, "start_time": "09:00", "end_time": "18:00" },
                "thu": { "enabled": false, "start_time": "09:00", "end_time": "18:00" },
                "fri": { "enabled": false, "start_time": "09:00", "end_time": "18:00" },
                "sat": { "enabled": false, "start_time": "09:00", "end_time": "18:00" },
                "sun": { "enabled": false, "start_time": "09:00", "end_time": "18:00" }
            },
            "timer_start_source": "manual_inside_window",
            "scheduled_stop_state": "pending"
        });

        fs::write(
            &path,
            serde_json::to_string_pretty(&legacy_schema_json)
                .expect("legacy schema json should serialize"),
        )
        .expect("legacy schema json should be writable");

        let loaded_state = PersistenceManager::load_state_from_path(fixed_clock.as_ref(), &path)
            .expect("state should load");

        assert_eq!(loaded_state.status, TimerStatus::Idle);
        assert_eq!(loaded_state.session_type, SessionType::Work);
        assert_eq!(
            loaded_state.remaining_seconds,
            TimerSettings::default().work_duration_secs
        );
        assert!(!loaded_state.auto_run_feature_enabled);
        assert!(!loaded_state.post_break_decision_pending);
        assert_eq!(
            loaded_state.weekly_auto_run_config,
            WeeklyAutoRunConfig::default()
        );
        assert!(loaded_state.timer_start_source.is_none());
        assert_eq!(loaded_state.scheduled_stop_state, ScheduledStopState::None);

        remove_test_state_file(&path);
    }

    #[test]
    fn test_load_state_unknown_schema_version_uses_safe_fallback() {
        let path = create_test_state_file_path("load_unknown_schema");
        let saved_at_utc = create_utc_datetime(2025, 2, 1, 9, 0, 0);
        let fixed_clock = create_fixed_clock_for_utc_datetime(saved_at_utc);

        let unknown_schema_json = json!({
            "schema_version": 99,
            "status": "on_break",
            "remaining_seconds": 240,
            "session_type": "short_break",
            "current_cycle": 3,
            "completed_pomodoros": 2,
            "post_break_decision_pending": false,
            "settings": {
                "work_duration_secs": 1500,
                "short_break_duration_secs": 300,
                "long_break_duration_secs": 900,
                "long_break_interval": 4
            },
            "saved_at": saved_at_utc,
            "auto_run_feature_enabled": true,
            "weekly_auto_run_config": {
                "mon": { "enabled": true, "start_time": "09:00", "end_time": "18:00" },
                "tue": { "enabled": false, "start_time": "09:00", "end_time": "18:00" },
                "wed": { "enabled": false, "start_time": "09:00", "end_time": "18:00" },
                "thu": { "enabled": false, "start_time": "09:00", "end_time": "18:00" },
                "fri": { "enabled": false, "start_time": "09:00", "end_time": "18:00" },
                "sat": { "enabled": false, "start_time": "09:00", "end_time": "18:00" },
                "sun": { "enabled": false, "start_time": "09:00", "end_time": "18:00" }
            },
            "timer_start_source": "manual_inside_window",
            "scheduled_stop_state": "pending"
        });

        fs::write(
            &path,
            serde_json::to_string_pretty(&unknown_schema_json).expect("json should serialize"),
        )
        .expect("unknown schema json should be writable");

        let loaded_state = PersistenceManager::load_state_from_path(fixed_clock.as_ref(), &path)
            .expect("state should load");

        assert_eq!(loaded_state.status, TimerStatus::Idle);
        assert_eq!(loaded_state.session_type, SessionType::Work);
        assert_eq!(
            loaded_state.remaining_seconds,
            TimerSettings::default().work_duration_secs
        );
        assert!(!loaded_state.auto_run_feature_enabled);
        assert!(!loaded_state.post_break_decision_pending);
        assert_eq!(
            loaded_state.weekly_auto_run_config,
            WeeklyAutoRunConfig::default()
        );
        assert!(loaded_state.timer_start_source.is_none());
        assert_eq!(loaded_state.scheduled_stop_state, ScheduledStopState::None);

        remove_test_state_file(&path);
    }

    #[test]
    fn test_running_restore_keeps_pending_stop_state() {
        let saved_at_utc = create_utc_datetime(2025, 2, 1, 9, 0, 0);
        let fixed_clock = create_fixed_clock_for_utc_datetime(saved_at_utc);

        let persisted = PersistedState {
            schema_version: PERSISTED_STATE_SCHEMA_VERSION_V3,
            status: TimerStatus::Working,
            remaining_seconds: 800,
            session_type: SessionType::Work,
            current_cycle: 2,
            completed_pomodoros: 1,
            post_break_decision_pending: false,
            settings: TimerSettings::default(),
            saved_at: saved_at_utc,
            auto_run_feature_enabled: true,
            weekly_auto_run_config: WeeklyAutoRunConfig::default(),
            timer_start_source: Some(TimerStartSource::ManualInsideWindow),
            scheduled_stop_state: ScheduledStopState::Pending,
        };

        let restored_state = persisted.into_timer_state(fixed_clock.as_ref());
        assert_eq!(
            restored_state.timer_start_source,
            Some(TimerStartSource::ManualInsideWindow)
        );
        assert_eq!(
            restored_state.scheduled_stop_state,
            ScheduledStopState::Pending
        );
    }

    #[test]
    fn test_idle_restore_normalizes_pending_stop_state_to_none() {
        let saved_at_utc = create_utc_datetime(2025, 2, 1, 9, 0, 0);
        let fixed_clock = create_fixed_clock_for_utc_datetime(saved_at_utc);

        let persisted = PersistedState {
            schema_version: PERSISTED_STATE_SCHEMA_VERSION_V3,
            status: TimerStatus::Idle,
            remaining_seconds: 1500,
            session_type: SessionType::Work,
            current_cycle: 0,
            completed_pomodoros: 0,
            post_break_decision_pending: false,
            settings: TimerSettings::default(),
            saved_at: saved_at_utc,
            auto_run_feature_enabled: true,
            weekly_auto_run_config: WeeklyAutoRunConfig::default(),
            timer_start_source: Some(TimerStartSource::ManualInsideWindow),
            scheduled_stop_state: ScheduledStopState::Pending,
        };

        let restored_state = persisted.into_timer_state(fixed_clock.as_ref());
        assert!(restored_state.timer_start_source.is_none());
        assert_eq!(
            restored_state.scheduled_stop_state,
            ScheduledStopState::None
        );
    }

    #[test]
    fn test_persisted_state_conversion_adjusts_elapsed_time() {
        let saved_at_utc = create_utc_datetime(2025, 2, 1, 9, 0, 0);
        let fixed_clock =
            create_fixed_clock_for_utc_datetime(saved_at_utc + chrono::Duration::seconds(30));

        let persisted = PersistedState {
            schema_version: PERSISTED_STATE_SCHEMA_VERSION_V3,
            status: TimerStatus::Working,
            remaining_seconds: 100,
            session_type: SessionType::Work,
            current_cycle: 1,
            completed_pomodoros: 0,
            post_break_decision_pending: false,
            settings: TimerSettings::default(),
            saved_at: saved_at_utc,
            auto_run_feature_enabled: false,
            weekly_auto_run_config: WeeklyAutoRunConfig::default(),
            timer_start_source: Some(TimerStartSource::ManualOutsideWindow),
            scheduled_stop_state: ScheduledStopState::None,
        };

        let timer_state = persisted.into_timer_state(fixed_clock.as_ref());
        assert_eq!(timer_state.remaining_seconds, 70);
    }

    #[test]
    fn test_post_break_pending_restore_forces_idle_work_preview_without_elapsed_subtraction() {
        let saved_at_utc = create_utc_datetime(2025, 2, 1, 9, 0, 0);
        let fixed_clock =
            create_fixed_clock_for_utc_datetime(saved_at_utc + chrono::Duration::seconds(45));

        let persisted = PersistedState {
            schema_version: PERSISTED_STATE_SCHEMA_VERSION_V3,
            status: TimerStatus::Working,
            remaining_seconds: 12,
            session_type: SessionType::ShortBreak,
            current_cycle: 2,
            completed_pomodoros: 1,
            post_break_decision_pending: true,
            settings: TimerSettings {
                work_duration_secs: 1500,
                ..TimerSettings::default()
            },
            saved_at: saved_at_utc,
            auto_run_feature_enabled: true,
            weekly_auto_run_config: WeeklyAutoRunConfig::default(),
            timer_start_source: Some(TimerStartSource::ManualInsideWindow),
            scheduled_stop_state: ScheduledStopState::Pending,
        };

        let restored_state = persisted.into_timer_state(fixed_clock.as_ref());
        assert_eq!(restored_state.status, TimerStatus::Idle);
        assert_eq!(restored_state.session_type, SessionType::Work);
        assert_eq!(restored_state.remaining_seconds, 1500);
        assert!(restored_state.post_break_decision_pending);
        assert!(restored_state.timer_start_source.is_none());
        assert_eq!(
            restored_state.scheduled_stop_state,
            ScheduledStopState::None
        );
    }

    #[test]
    fn test_on_break_restore_rewinds_to_idle_work_preview_and_reverts_completed_count() {
        let saved_at_utc = create_utc_datetime(2025, 2, 1, 9, 0, 0);
        let fixed_clock =
            create_fixed_clock_for_utc_datetime(saved_at_utc + chrono::Duration::seconds(45));

        let persisted = PersistedState {
            schema_version: PERSISTED_STATE_SCHEMA_VERSION_V3,
            status: TimerStatus::OnBreak,
            remaining_seconds: 120,
            session_type: SessionType::LongBreak,
            current_cycle: 4,
            completed_pomodoros: 4,
            post_break_decision_pending: false,
            settings: TimerSettings {
                work_duration_secs: 1500,
                ..TimerSettings::default()
            },
            saved_at: saved_at_utc,
            auto_run_feature_enabled: true,
            weekly_auto_run_config: WeeklyAutoRunConfig::default(),
            timer_start_source: Some(TimerStartSource::ManualInsideWindow),
            scheduled_stop_state: ScheduledStopState::Pending,
        };

        let restored_state = persisted.into_timer_state(fixed_clock.as_ref());
        assert_eq!(restored_state.status, TimerStatus::Idle);
        assert_eq!(restored_state.session_type, SessionType::Work);
        assert_eq!(restored_state.remaining_seconds, 1500);
        assert_eq!(restored_state.current_cycle, 4);
        assert_eq!(restored_state.completed_pomodoros, 3);
        assert!(!restored_state.post_break_decision_pending);
        assert!(restored_state.timer_start_source.is_none());
        assert_eq!(
            restored_state.scheduled_stop_state,
            ScheduledStopState::None
        );
    }

    #[test]
    fn test_load_state_with_corrupted_file_returns_none() {
        let path = create_test_state_file_path("corrupted");
        let fixed_clock =
            create_fixed_clock_for_utc_datetime(create_utc_datetime(2025, 2, 1, 9, 0, 0));
        fs::write(&path, b"not valid json {{{").expect("corrupted fixture should be writable");

        let loaded_state = PersistenceManager::load_state_from_path(fixed_clock.as_ref(), &path);
        assert!(
            loaded_state.is_none(),
            "corrupted state file should return None"
        );

        remove_test_state_file(&path);
    }
}
