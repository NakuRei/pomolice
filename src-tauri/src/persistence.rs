// Persistence Module
// Handles saving and loading timer state for recovery after app restart

use crate::timer_state::{SessionType, TimerSettings, TimerState, TimerStatus};
use chrono::{DateTime, Utc};
use directories::ProjectDirs;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use std::sync::Mutex;
use std::time::Instant;

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
    pub fn elapsed_seconds_since_save(&self) -> u64 {
        let now = Utc::now();
        let elapsed = now.signed_duration_since(self.saved_at);
        elapsed.num_seconds().max(0) as u64
    }
}

// Debounce state saving to avoid excessive disk writes
static LAST_SAVE: Mutex<Option<Instant>> = Mutex::new(None);
const SAVE_DEBOUNCE_MS: u64 = 500;

/// Get the path to the state file
fn get_state_file_path() -> Option<PathBuf> {
    ProjectDirs::from("io.github", "NakuRei", "pomolice").map(|dirs| {
        let data_dir = dirs.data_dir();
        data_dir.join("timer_state.json")
    })
}

/// Save timer state to disk (debounced)
pub fn save_state(state: &TimerState) {
    // Debounce: only save once per 500ms
    {
        let mut last_save = LAST_SAVE.lock().unwrap();
        if let Some(last) = *last_save {
            if last.elapsed().as_millis() < SAVE_DEBOUNCE_MS as u128 {
                return;
            }
        }
        *last_save = Some(Instant::now());
    }

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

    if let Some(path) = get_state_file_path() {
        // Ensure directory exists
        if let Some(parent) = path.parent() {
            if let Err(e) = fs::create_dir_all(parent) {
                eprintln!("Failed to create data directory: {}", e);
                return;
            }
        }

        // Serialize and write
        match serde_json::to_string_pretty(&persisted) {
            Ok(json) => {
                if let Err(e) = fs::write(&path, json) {
                    eprintln!("Failed to write state file: {}", e);
                }
            }
            Err(e) => {
                eprintln!("Failed to serialize state: {}", e);
            }
        }
    }
}

/// Load timer state from disk
pub fn load_state() -> Option<PersistedState> {
    let path = get_state_file_path()?;

    if !path.exists() {
        return None;
    }

    let content = fs::read_to_string(&path).ok()?;
    let persisted: PersistedState = serde_json::from_str(&content).ok()?;

    Some(persisted)
}

/// Clear persisted state (for testing or reset)
#[allow(dead_code)]
pub fn clear_state() {
    if let Some(path) = get_state_file_path() {
        let _ = fs::remove_file(path);
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
        let path = get_state_file_path();
        assert!(path.is_some());
        let path = path.unwrap();
        assert!(path.ends_with("timer_state.json"));
    }
}
