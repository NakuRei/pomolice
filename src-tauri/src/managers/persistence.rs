// Persistence Manager
// Handles saving and loading timer state without global statics

use atomic_write_file::AtomicWriteFile;
use chrono::{DateTime, Utc};
use directories::ProjectDirs;
use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use std::fs;
use std::io::Write;
use std::path::PathBuf;
use std::time::Instant;
use tracing::{error, info, warn};

use crate::types::{SessionType, TimerSettings, TimerState, TimerStatus};

/// Persisted state that can be serialized to disk
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PersistedState {
    pub status: TimerStatus,
    pub remaining_seconds: u64,
    pub session_type: SessionType,
    pub current_cycle: u32,
    pub completed_pomodoros: u32,
    pub show_continue_dialog: bool,
    pub settings: TimerSettings,
    pub saved_at: DateTime<Utc>,
}

impl PersistedState {
    /// Calculate elapsed seconds since this state was saved
    #[allow(dead_code)]
    pub fn elapsed_seconds_since_save(&self) -> u64 {
        let now = Utc::now();
        let elapsed = now.signed_duration_since(self.saved_at);
        elapsed.num_seconds().max(0) as u64
    }

    /// Convert to TimerState
    pub fn into_timer_state(self) -> TimerState {
        TimerState {
            status: self.status,
            remaining_seconds: self.remaining_seconds,
            session_type: self.session_type,
            current_cycle: self.current_cycle,
            completed_pomodoros: self.completed_pomodoros,
            break_interaction_count: 0,
            last_penalty_time: None,
            skip_attempts: 0,
            settings: self.settings,
            show_continue_dialog: self.show_continue_dialog,
            is_fullscreen_break: false,
            last_transition: None,
        }
    }
}

/// Manager for state persistence
pub struct PersistenceManager {
    last_save: Mutex<Option<Instant>>,
}

impl PersistenceManager {
    const SAVE_DEBOUNCE_MS: u64 = 500;

    /// Create a new persistence manager
    pub fn new() -> Self {
        Self {
            last_save: Mutex::new(None),
        }
    }

    /// Get the path to the state file
    fn get_state_file_path() -> Option<PathBuf> {
        ProjectDirs::from("io.github", "NakuRei", "pomolice").map(|dirs| {
            let data_dir = dirs.data_dir();
            data_dir.join("timer_state.json")
        })
    }

    /// Save timer state to disk (debounced)
    pub fn save_state(&self, state: &TimerState) {
        // Debounce: only save once per 500ms
        {
            let mut last_save = self.last_save.lock();
            if let Some(last) = *last_save {
                if last.elapsed().as_millis() < Self::SAVE_DEBOUNCE_MS as u128 {
                    return;
                }
            }
            *last_save = Some(Instant::now());
        }

        info!("Saving timer state to disk");

        let persisted = PersistedState {
            status: state.status,
            remaining_seconds: state.remaining_seconds,
            session_type: state.session_type,
            current_cycle: state.current_cycle,
            completed_pomodoros: state.completed_pomodoros,
            show_continue_dialog: state.show_continue_dialog,
            settings: state.settings.clone(),
            saved_at: Utc::now(),
        };

        if let Some(path) = Self::get_state_file_path() {
            // Ensure directory exists
            if let Some(parent) = path.parent() {
                if let Err(e) = fs::create_dir_all(parent) {
                    error!("Failed to create data directory: {}", e);
                    return;
                }
            }

            // Serialize and write atomically: write to a temp file in the same
            // directory, then rename it to the target path. This ensures the
            // target file is never left in a truncated/partial state on crash.
            match serde_json::to_string_pretty(&persisted) {
                Ok(json) => match AtomicWriteFile::options().open(&path) {
                    Ok(mut tmp) => {
                        if let Err(e) = tmp.write_all(json.as_bytes()) {
                            error!("Failed to write state to temp file: {}", e);
                        } else if let Err(e) = tmp.commit() {
                            error!("Failed to commit state file atomically: {}", e);
                        } else {
                            info!("Successfully saved state to {:?}", path);
                        }
                    }
                    Err(e) => {
                        error!("Failed to open atomic write file at {:?}: {}", path, e);
                    }
                },
                Err(e) => {
                    error!("Failed to serialize state: {}", e);
                }
            }
        } else {
            warn!("Could not determine state file path");
        }
    }

    /// Load timer state from disk
    pub fn load_state(&self) -> Option<TimerState> {
        let path = Self::get_state_file_path()?;

        if !path.exists() {
            info!("No persisted state file found at {:?}", path);
            return None;
        }

        info!("Loading timer state from {:?}", path);

        let content = match fs::read_to_string(&path) {
            Ok(c) => c,
            Err(e) => {
                error!("Failed to read state file: {}", e);
                return None;
            }
        };

        let persisted: PersistedState = match serde_json::from_str(&content) {
            Ok(p) => p,
            Err(e) => {
                error!("Failed to deserialize state: {}", e);
                return None;
            }
        };

        info!("Successfully loaded persisted state");
        Some(persisted.into_timer_state())
    }

    /// Clear persisted state (for testing or reset)
    #[allow(dead_code)]
    pub fn clear_state(&self) {
        if let Some(path) = Self::get_state_file_path() {
            let _ = fs::remove_file(path);
        }
    }
}

impl Default for PersistenceManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_elapsed_seconds_calculation() {
        let persisted = PersistedState {
            status: TimerStatus::Working,
            remaining_seconds: 100,
            session_type: SessionType::Work,
            current_cycle: 1,
            completed_pomodoros: 0,
            show_continue_dialog: false,
            settings: TimerSettings::default(),
            saved_at: Utc::now() - chrono::Duration::seconds(30),
        };

        let elapsed = persisted.elapsed_seconds_since_save();
        // Allow for small timing differences
        assert!(elapsed >= 29 && elapsed <= 31);
    }

    #[test]
    fn test_state_file_path() {
        let path = PersistenceManager::get_state_file_path();
        assert!(path.is_some());
        let path = path.unwrap();
        assert!(path.ends_with("timer_state.json"));
    }

    #[test]
    fn test_persisted_state_conversion() {
        let settings = TimerSettings {
            work_duration_secs: 1500,
            short_break_duration_secs: 300,
            long_break_duration_secs: 900,
            long_break_interval: 4,
        };

        let persisted = PersistedState {
            status: TimerStatus::Working,
            remaining_seconds: 1200,
            session_type: SessionType::Work,
            current_cycle: 2,
            completed_pomodoros: 1,
            show_continue_dialog: false,
            settings: settings.clone(),
            saved_at: Utc::now(),
        };

        let timer_state = persisted.into_timer_state();
        assert_eq!(timer_state.status, TimerStatus::Working);
        assert_eq!(timer_state.remaining_seconds, 1200);
        assert_eq!(timer_state.session_type, SessionType::Work);
        assert_eq!(timer_state.current_cycle, 2);
        assert_eq!(timer_state.completed_pomodoros, 1);
        assert!(!timer_state.show_continue_dialog);
        assert_eq!(timer_state.break_interaction_count, 0);
        assert_eq!(timer_state.skip_attempts, 0);
        assert!(!timer_state.is_fullscreen_break);
        assert!(timer_state.last_penalty_time.is_none());
        assert!(timer_state.last_transition.is_none());
    }

    #[test]
    fn test_persistence_manager_initialization() {
        let manager = PersistenceManager::new();
        let last_save = manager.last_save.lock();
        assert!(last_save.is_none());
    }

    #[test]
    fn test_persistence_manager_default() {
        let manager = PersistenceManager::default();
        let last_save = manager.last_save.lock();
        assert!(last_save.is_none());
    }

    #[test]
    fn test_save_and_load_state() {
        let manager = PersistenceManager::new();

        // Create a test state
        let mut state = TimerState::default();
        state.status = TimerStatus::Working;
        state.remaining_seconds = 1500;
        state.session_type = SessionType::Work;
        state.current_cycle = 1;
        state.completed_pomodoros = 0;
        state.show_continue_dialog = false;

        // Save the state
        manager.save_state(&state);

        // Load the state
        let loaded = manager.load_state();
        assert!(loaded.is_some());

        let loaded_state = loaded.unwrap();
        assert_eq!(loaded_state.status, TimerStatus::Working);
        assert_eq!(loaded_state.remaining_seconds, 1500);
        assert_eq!(loaded_state.session_type, SessionType::Work);
        assert_eq!(loaded_state.current_cycle, 1);
        assert_eq!(loaded_state.completed_pomodoros, 0);
        assert!(!loaded_state.show_continue_dialog);

        // Clean up
        manager.clear_state();
    }

    #[test]
    fn test_load_nonexistent_state() {
        let manager = PersistenceManager::new();

        // Clear state first to ensure it doesn't exist
        manager.clear_state();

        let loaded = manager.load_state();
        assert!(loaded.is_none());
    }

    #[test]
    fn test_load_state_with_corrupted_file() {
        // Simulate a partial write by writing invalid JSON to the state file.
        // load_state() must return None without panicking.
        if let Some(path) = PersistenceManager::get_state_file_path() {
            let _ = fs::create_dir_all(path.parent().unwrap());
            fs::write(&path, b"not valid json {{{").expect("failed to write corrupted fixture");

            let manager = PersistenceManager::new();
            let loaded = manager.load_state();
            assert!(loaded.is_none(), "expected None for corrupted state file");

            let _ = fs::remove_file(&path);
        }
    }
}
