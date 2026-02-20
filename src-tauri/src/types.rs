// Common type definitions for the timer application

use serde::{Deserialize, Serialize};

/// Timer status representing the current phase
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TimerStatus {
    Idle,
    Working,
    OnBreak,
}

/// Session type representing work or break sessions
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SessionType {
    Work,
    ShortBreak,
    LongBreak,
}

/// Timer settings synchronized from frontend
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimerSettings {
    pub work_duration_secs: u64,
    pub short_break_duration_secs: u64,
    pub long_break_duration_secs: u64,
    pub long_break_interval: u32,
}

impl Default for TimerSettings {
    fn default() -> Self {
        Self {
            work_duration_secs: 25 * 60,       // 25 minutes
            short_break_duration_secs: 5 * 60, // 5 minutes
            long_break_duration_secs: 15 * 60, // 15 minutes
            long_break_interval: 4,
        }
    }
}

/// Payload sent to frontend on each timer tick
#[derive(Debug, Clone, Serialize)]
pub struct TimerTickPayload {
    pub status: TimerStatus,
    pub remaining_seconds: u64,
    pub session_type: SessionType,
    pub current_cycle: u32,
    pub completed_pomodoros: u32,
    pub break_interaction_count: u32,
    pub is_fullscreen_break: bool,
}

/// Payload sent to frontend when a session completes
#[derive(Debug, Clone, Serialize)]
pub struct SessionCompletedPayload {
    pub completed_session_type: SessionType,
    pub next_session_type: Option<SessionType>,
    pub show_continue_dialog: bool,
}

/// Full timer state sent on initial sync
#[derive(Debug, Clone, Serialize)]
pub struct FullStatePayload {
    pub status: TimerStatus,
    pub remaining_seconds: u64,
    pub session_type: SessionType,
    pub current_cycle: u32,
    pub completed_pomodoros: u32,
    pub break_interaction_count: u32,
    pub is_fullscreen_break: bool,
    pub show_continue_dialog: bool,
    pub settings: TimerSettings,
}

/// Application state for timer management
#[derive(Debug)]
pub struct TimerState {
    // Current phase
    pub status: TimerStatus,

    // Timer countdown (in seconds)
    pub remaining_seconds: u64,

    // Session management
    pub session_type: SessionType,
    pub current_cycle: u32,
    pub completed_pomodoros: u32,

    // Break enforcement
    pub break_interaction_count: u32,
    pub last_penalty_time: Option<std::time::Instant>,
    pub skip_attempts: u32,

    // Settings (synced from frontend)
    pub settings: TimerSettings,

    // UI state
    pub show_continue_dialog: bool,
    pub is_fullscreen_break: bool,

    // Internal
    pub last_transition: Option<std::time::Instant>,
}

impl Default for TimerState {
    fn default() -> Self {
        let settings = TimerSettings::default();
        Self {
            status: TimerStatus::Idle,
            remaining_seconds: settings.work_duration_secs,
            session_type: SessionType::Work,
            current_cycle: 0,
            completed_pomodoros: 0,
            break_interaction_count: 0,
            last_penalty_time: None,
            skip_attempts: 0,
            settings,
            show_continue_dialog: false,
            is_fullscreen_break: false,
            last_transition: None,
        }
    }
}

impl TimerState {
    pub fn to_tick_payload(&self) -> TimerTickPayload {
        TimerTickPayload {
            status: self.status,
            remaining_seconds: self.remaining_seconds,
            session_type: self.session_type,
            current_cycle: self.current_cycle,
            completed_pomodoros: self.completed_pomodoros,
            break_interaction_count: self.break_interaction_count,
            is_fullscreen_break: self.is_fullscreen_break,
        }
    }

    pub fn to_full_payload(&self) -> FullStatePayload {
        FullStatePayload {
            status: self.status,
            remaining_seconds: self.remaining_seconds,
            session_type: self.session_type,
            current_cycle: self.current_cycle,
            completed_pomodoros: self.completed_pomodoros,
            break_interaction_count: self.break_interaction_count,
            is_fullscreen_break: self.is_fullscreen_break,
            show_continue_dialog: self.show_continue_dialog,
            settings: self.settings.clone(),
        }
    }

    /// Get break duration based on session type
    pub fn get_break_duration(&self) -> u64 {
        match self.session_type {
            SessionType::ShortBreak => self.settings.short_break_duration_secs,
            SessionType::LongBreak => self.settings.long_break_duration_secs,
            SessionType::Work => 0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_timer_state_default() {
        let state = TimerState::default();
        assert_eq!(state.status, TimerStatus::Idle);
        assert_eq!(state.session_type, SessionType::Work);
        assert_eq!(state.remaining_seconds, 25 * 60);
    }

    #[test]
    fn test_timer_settings_default() {
        let settings = TimerSettings::default();
        assert_eq!(settings.work_duration_secs, 25 * 60);
        assert_eq!(settings.short_break_duration_secs, 5 * 60);
        assert_eq!(settings.long_break_duration_secs, 15 * 60);
        assert_eq!(settings.long_break_interval, 4);
    }
}
