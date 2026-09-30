use crate::ipc::settings::{SettingsFileStatus, SettingsInput};
use crate::timer_core::BreakLockTarget;
use crate::types::{SessionTransitionReason, SessionType, TimerStatus};
use serde::Serialize;

/// Payload sent to frontend on each timer tick
#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
pub(crate) struct TimerTickPayload {
    pub status: TimerStatus,
    pub remaining_seconds: u64,
    /// Total length of the current session; while idle, the length the next
    /// work session will have.
    pub session_duration_secs: u64,
    pub session_type: SessionType,
    pub current_cycle: u32,
    pub completed_pomodoros: u32,
    pub break_interaction_count: u32,
    pub away_during_break: bool,
}

/// Payload sent to frontend when a session completes
#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
pub(crate) struct SessionCompletedPayload {
    pub transition_reason: SessionTransitionReason,
    pub completed_session_type: SessionType,
    pub next_status: TimerStatus,
    pub next_session_type: SessionType,
    pub requires_manual_continue_decision: bool,
}

impl SessionCompletedPayload {
    pub(crate) fn work_to_break(next_session_type: SessionType) -> Self {
        debug_assert!(matches!(
            next_session_type,
            SessionType::ShortBreak | SessionType::LongBreak
        ));
        Self {
            transition_reason: SessionTransitionReason::WorkToBreak,
            completed_session_type: SessionType::Work,
            next_status: TimerStatus::OnBreak,
            next_session_type,
            requires_manual_continue_decision: false,
        }
    }

    pub(crate) fn break_waiting_decision(completed_session_type: SessionType) -> Self {
        debug_assert!(matches!(
            completed_session_type,
            SessionType::ShortBreak | SessionType::LongBreak
        ));
        Self {
            transition_reason: SessionTransitionReason::BreakWaitingDecision,
            completed_session_type,
            next_status: TimerStatus::Idle,
            next_session_type: SessionType::Work,
            requires_manual_continue_decision: true,
        }
    }

    pub(crate) fn break_auto_started(completed_session_type: SessionType) -> Self {
        debug_assert!(matches!(
            completed_session_type,
            SessionType::ShortBreak | SessionType::LongBreak
        ));
        Self {
            transition_reason: SessionTransitionReason::BreakAutoStarted,
            completed_session_type,
            next_status: TimerStatus::Working,
            next_session_type: SessionType::Work,
            requires_manual_continue_decision: false,
        }
    }

    pub(crate) fn scheduled_stop_executed(completed_session_type: SessionType) -> Self {
        Self {
            transition_reason: SessionTransitionReason::ScheduledStopExecuted,
            completed_session_type,
            next_status: TimerStatus::Idle,
            next_session_type: SessionType::Work,
            requires_manual_continue_decision: false,
        }
    }
}

/// Full timer state sent on initial sync
#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
pub(crate) struct FullStatePayload {
    pub status: TimerStatus,
    pub remaining_seconds: u64,
    pub session_duration_secs: u64,
    pub session_type: SessionType,
    pub current_cycle: u32,
    pub completed_pomodoros: u32,
    pub break_interaction_count: u32,
    pub away_during_break: bool,
    pub post_break_decision_pending: bool,
    pub settings: SettingsInput,
    pub settings_file_status: SettingsFileStatus,
    /// Present while the boot auto-start prompt is still waiting for a decision.
    pub boot_auto_start_offer: Option<BootAutoStartOfferPayload>,
    pub active_window_summary: Option<ActiveWindowSummaryPayload>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
pub(crate) struct BootAutoStartOfferPayload {
    pub anchor_local_date: chrono::NaiveDate,
    pub window: ActiveWindowSummaryPayload,
}

/// Summary of the currently active schedule window.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
pub(crate) struct ActiveWindowSummaryPayload {
    pub weekday_key: crate::types::WeekdayKey,
    pub start_time: String,
    pub end_time: String,
    pub crosses_midnight: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
pub(crate) struct ActiveWindowChangedPayload {
    pub active_window_summary: Option<ActiveWindowSummaryPayload>,
}

/// Payload sent to frontend when the settings in effect change
#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
pub(crate) struct SettingsChangedPayload {
    pub settings: SettingsInput,
    pub settings_file_status: SettingsFileStatus,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "snake_case")]
pub(crate) enum EffectStatus {
    Succeeded,
    Failed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "snake_case")]
pub(crate) enum BreakLockAction {
    Enter,
    Release,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, specta::Type)]
pub(crate) struct BreakLockOutcome {
    pub action: BreakLockAction,
    pub status: EffectStatus,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, specta::Type)]
pub(crate) struct BreakLockState {
    pub target: BreakLockTarget,
    pub last_attempt: Option<BreakLockOutcome>,
}

/// Latest attempts in this application run. None means not attempted;
/// a failed lock operation leaves the physical window state uncertain.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, specta::Type)]
pub(crate) struct TimerEffectsState {
    pub revision: EffectRevision,
    pub break_lock: BreakLockState,
    pub last_timer_save: Option<EffectStatus>,
}

/// Reads and accepted commands return the same observation contract.
/// A rejected command changes no timer state.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
pub(crate) struct TimerSnapshot {
    pub timer: FullStatePayload,
    pub effects: TimerEffectsState,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, specta::Type)]
#[specta(transparent)]
pub(crate) struct EffectRevision(#[specta(type = String)] pub u64);

impl Serialize for EffectRevision {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_str(&self.0)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize, specta::Type)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub(crate) enum EmergencyShortcutStatus {
    InputMonitor,
    Registering,
    Registered { trigger: String },
    Unavailable,
}
