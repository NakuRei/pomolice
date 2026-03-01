// Common type definitions for the timer application

use serde::{Deserialize, Serialize};

/// Persisted state schema version without auto-run fields.
pub const PERSISTED_STATE_SCHEMA_VERSION_V1: u32 = 1;

/// Persisted state schema version with auto-run related fields.
pub const PERSISTED_STATE_SCHEMA_VERSION_V2: u32 = 2;

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

#[allow(dead_code)]
fn default_slot_start_time() -> String {
    "09:00".to_string()
}

#[allow(dead_code)]
fn default_slot_end_time() -> String {
    "18:00".to_string()
}

/// A single weekday schedule slot for automatic timer execution.
///
/// Usage example:
/// ```rust,ignore
/// let slot = DayAutoRunSlot {
///     enabled: true,
///     start_time: "09:00".to_string(),
///     end_time: "18:00".to_string(),
/// };
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
#[allow(dead_code)]
pub struct DayAutoRunSlot {
    pub enabled: bool,
    #[serde(default = "default_slot_start_time", alias = "startTime")]
    pub start_time: String,
    #[serde(default = "default_slot_end_time", alias = "endTime")]
    pub end_time: String,
}

impl Default for DayAutoRunSlot {
    fn default() -> Self {
        Self {
            enabled: false,
            start_time: default_slot_start_time(),
            end_time: default_slot_end_time(),
        }
    }
}

/// Weekly schedule configuration normalized to Monday-Sunday keys.
///
/// Usage examples:
/// ```rust,ignore
/// // Standard daytime schedule
/// let mut weekly = WeeklyAutoRunConfig::default();
/// weekly.mon = DayAutoRunSlot {
///     enabled: true,
///     start_time: "09:00".to_string(),
///     end_time: "18:00".to_string(),
/// };
///
/// // Overnight schedule (crosses midnight)
/// weekly.fri = DayAutoRunSlot {
///     enabled: true,
///     start_time: "22:00".to_string(),
///     end_time: "02:00".to_string(),
/// };
///
/// // Disabled day (slot values preserved but ignored)
/// weekly.sun = DayAutoRunSlot {
///     enabled: false,
///     start_time: "09:00".to_string(),
///     end_time: "18:00".to_string(),
/// };
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
#[allow(dead_code)]
pub struct WeeklyAutoRunConfig {
    pub mon: DayAutoRunSlot,
    pub tue: DayAutoRunSlot,
    pub wed: DayAutoRunSlot,
    pub thu: DayAutoRunSlot,
    pub fri: DayAutoRunSlot,
    pub sat: DayAutoRunSlot,
    pub sun: DayAutoRunSlot,
}

impl Default for WeeklyAutoRunConfig {
    fn default() -> Self {
        Self {
            mon: DayAutoRunSlot::default(),
            tue: DayAutoRunSlot::default(),
            wed: DayAutoRunSlot::default(),
            thu: DayAutoRunSlot::default(),
            fri: DayAutoRunSlot::default(),
            sat: DayAutoRunSlot::default(),
            sun: DayAutoRunSlot::default(),
        }
    }
}

/// Source that started the current work timer.
///
/// Usage example:
/// ```rust,ignore
/// let source = TimerStartSource::ManualInsideWindow;
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[allow(dead_code)]
pub enum TimerStartSource {
    ManualOutsideWindow,
    ManualInsideWindow,
    ScheduledAutoStart,
}

impl Default for TimerStartSource {
    fn default() -> Self {
        Self::ManualOutsideWindow
    }
}

/// Current state for session-boundary scheduled stop execution.
///
/// Usage example:
/// ```rust,ignore
/// let stop_state = ScheduledStopState::Pending;
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[allow(dead_code)]
pub enum ScheduledStopState {
    None,
    Pending,
}

impl Default for ScheduledStopState {
    fn default() -> Self {
        Self::None
    }
}

/// Unique boundary identifier used to deduplicate processed schedule boundaries.
///
/// Usage example:
/// ```rust,ignore
/// let id = BoundaryOccurrenceId::new("mon|start|1738707600");
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(transparent)]
#[allow(dead_code)]
pub struct BoundaryOccurrenceId(pub String);

#[allow(dead_code)]
impl BoundaryOccurrenceId {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }
}

/// Runtime state for the one-time startup confirmation decision.
///
/// Usage example:
/// ```rust,ignore
/// let runtime_state = BootPromptRuntimeState::AwaitingDecision;
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[allow(dead_code)]
pub enum BootPromptRuntimeState {
    AwaitingDecision,
    ResolvedStart,
    ResolvedLater,
    NotApplicable,
}

impl Default for BootPromptRuntimeState {
    fn default() -> Self {
        Self::NotApplicable
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
    pub should_prompt_start_on_boot: bool,
    pub active_window_summary: Option<ActiveWindowSummaryPayload>,
}

/// Summary of the currently active schedule window.
#[derive(Debug, Clone, Serialize)]
pub struct ActiveWindowSummaryPayload {
    pub weekday_key: String,
    pub start_time: String,
    pub end_time: String,
    pub crosses_midnight: bool,
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
    pub auto_run_feature_enabled: bool,
    pub weekly_auto_run_config: WeeklyAutoRunConfig,
    pub timer_start_source: Option<TimerStartSource>,
    pub scheduled_stop_state: ScheduledStopState,

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
            auto_run_feature_enabled: false,
            weekly_auto_run_config: WeeklyAutoRunConfig::default(),
            timer_start_source: None,
            scheduled_stop_state: ScheduledStopState::None,
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
            should_prompt_start_on_boot: false,
            active_window_summary: None,
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
    use serde::Deserialize;

    #[test]
    fn test_timer_state_default() {
        let state = TimerState::default();
        assert_eq!(state.status, TimerStatus::Idle);
        assert_eq!(state.session_type, SessionType::Work);
        assert_eq!(state.remaining_seconds, 25 * 60);
        assert!(!state.auto_run_feature_enabled);
        assert_eq!(state.weekly_auto_run_config, WeeklyAutoRunConfig::default());
        assert!(state.timer_start_source.is_none());
        assert_eq!(state.scheduled_stop_state, ScheduledStopState::None);
    }

    #[test]
    fn test_timer_settings_default() {
        let settings = TimerSettings::default();
        assert_eq!(settings.work_duration_secs, 25 * 60);
        assert_eq!(settings.short_break_duration_secs, 5 * 60);
        assert_eq!(settings.long_break_duration_secs, 15 * 60);
        assert_eq!(settings.long_break_interval, 4);
    }

    #[test]
    fn test_day_auto_run_slot_default() {
        let slot = DayAutoRunSlot::default();
        assert!(!slot.enabled);
        assert_eq!(slot.start_time, "09:00");
        assert_eq!(slot.end_time, "18:00");
    }

    #[test]
    fn test_day_auto_run_slot_deserialize_missing_fields_uses_defaults() {
        let slot: DayAutoRunSlot = serde_json::from_str(r#"{"enabled":true}"#)
            .expect("day slot should deserialize with missing times");
        assert!(slot.enabled);
        assert_eq!(slot.start_time, "09:00");
        assert_eq!(slot.end_time, "18:00");
    }

    #[test]
    fn test_day_auto_run_slot_deserialize_accepts_camel_case_aliases() {
        let slot: DayAutoRunSlot =
            serde_json::from_str(r#"{"enabled":true,"startTime":"08:00","endTime":"17:00"}"#)
                .expect("day slot should deserialize from camelCase aliases");
        assert!(slot.enabled);
        assert_eq!(slot.start_time, "08:00");
        assert_eq!(slot.end_time, "17:00");
    }

    #[test]
    fn test_weekly_auto_run_config_default_all_days_disabled() {
        let config = WeeklyAutoRunConfig::default();
        assert!(!config.mon.enabled);
        assert!(!config.tue.enabled);
        assert!(!config.wed.enabled);
        assert!(!config.thu.enabled);
        assert!(!config.fri.enabled);
        assert!(!config.sat.enabled);
        assert!(!config.sun.enabled);
    }

    #[test]
    fn test_weekly_auto_run_config_deserialize_empty_object_uses_defaults() {
        let config: WeeklyAutoRunConfig =
            serde_json::from_str("{}").expect("weekly config should deserialize from empty object");
        assert!(!config.mon.enabled);
        assert_eq!(config.mon.start_time, "09:00");
        assert_eq!(config.mon.end_time, "18:00");
        assert!(!config.sun.enabled);
    }

    #[test]
    fn test_timer_start_source_default() {
        assert_eq!(
            TimerStartSource::default(),
            TimerStartSource::ManualOutsideWindow
        );
    }

    #[test]
    fn test_scheduled_stop_state_default() {
        assert_eq!(ScheduledStopState::default(), ScheduledStopState::None);
    }

    #[test]
    fn test_boundary_occurrence_id_default_and_constructor() {
        let default_id = BoundaryOccurrenceId::default();
        assert_eq!(default_id.0, "");

        let custom_id = BoundaryOccurrenceId::new("mon|start|1738707600");
        assert_eq!(custom_id.0, "mon|start|1738707600");
    }

    #[test]
    fn test_boot_prompt_runtime_state_default() {
        assert_eq!(
            BootPromptRuntimeState::default(),
            BootPromptRuntimeState::NotApplicable
        );
    }

    #[test]
    fn test_enum_fields_deserialize_with_serde_default() {
        #[derive(Debug, Deserialize)]
        struct EnumDefaultsWrapper {
            #[serde(default)]
            timer_start_source: TimerStartSource,
            #[serde(default)]
            scheduled_stop_state: ScheduledStopState,
            #[serde(default)]
            boot_prompt_runtime_state: BootPromptRuntimeState,
        }

        let wrapper: EnumDefaultsWrapper =
            serde_json::from_str("{}").expect("enum defaults wrapper should deserialize");

        assert_eq!(
            wrapper.timer_start_source,
            TimerStartSource::ManualOutsideWindow
        );
        assert_eq!(wrapper.scheduled_stop_state, ScheduledStopState::None);
        assert_eq!(
            wrapper.boot_prompt_runtime_state,
            BootPromptRuntimeState::NotApplicable
        );
    }
}
