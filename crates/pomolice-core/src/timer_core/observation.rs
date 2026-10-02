use crate::settings::input::SettingsInput;
use crate::types::{SessionTransitionReason, SessionType, TimerStatus};
use serde::Serialize;

/// Payload sent to frontend when a session completes
#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
pub struct SessionCompletedPayload {
    pub transition_reason: SessionTransitionReason,
    pub completed_session_type: SessionType,
    pub next_status: TimerStatus,
    pub next_session_type: SessionType,
    pub requires_manual_continue_decision: bool,
}

impl SessionCompletedPayload {
    #[must_use]
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

    #[must_use]
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

    #[must_use]
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

    #[must_use]
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

/// Observable timer state at the requested instant.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
pub struct TimerState {
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
    /// Present while the boot auto-start prompt is still waiting for a decision.
    pub boot_auto_start_offer: Option<BootAutoStartOfferPayload>,
    pub active_window_summary: Option<ActiveWindowSummaryPayload>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
pub struct BootAutoStartOfferPayload {
    #[specta(type = String)]
    pub anchor_local_date: chrono::NaiveDate,
    pub window: ActiveWindowSummaryPayload,
}

/// Summary of the currently active schedule window.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
pub struct ActiveWindowSummaryPayload {
    pub weekday_key: crate::types::WeekdayKey,
    pub start_time: String,
    pub end_time: String,
    pub crosses_midnight: bool,
}
