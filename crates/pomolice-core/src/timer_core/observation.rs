use crate::schedule::boundary::ActiveWindow;
use crate::schedule::WeekdayKey;
use crate::settings::input::SettingsInput;
use crate::timer_core::{SessionType, TimerStatus};
use chrono::{Datelike, Weekday};
use serde::{Deserialize, Serialize};

/// Reason emitted for a session-boundary transition.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "snake_case")]
pub enum SessionTransitionReason {
    WorkToBreak,
    BreakWaitingDecision,
    BreakAutoStarted,
    ScheduledStopExecuted,
}

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
    pub weekday_key: WeekdayKey,
    pub start_time: String,
    pub end_time: String,
    pub crosses_midnight: bool,
}

impl From<&ActiveWindow> for ActiveWindowSummaryPayload {
    fn from(window: &ActiveWindow) -> Self {
        ActiveWindowSummaryPayload {
            weekday_key: match window.anchor_local_date.weekday() {
                Weekday::Mon => WeekdayKey::Mon,
                Weekday::Tue => WeekdayKey::Tue,
                Weekday::Wed => WeekdayKey::Wed,
                Weekday::Thu => WeekdayKey::Thu,
                Weekday::Fri => WeekdayKey::Fri,
                Weekday::Sat => WeekdayKey::Sat,
                Weekday::Sun => WeekdayKey::Sun,
            },
            start_time: window.start_time.clone(),
            end_time: window.end_time.clone(),
            crosses_midnight: window.crosses_midnight,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn session_completed_payloads_carry_the_transition_contract() {
        let work_to_break = SessionCompletedPayload::work_to_break(SessionType::LongBreak);
        assert_eq!(
            work_to_break.transition_reason,
            SessionTransitionReason::WorkToBreak
        );
        assert_eq!(work_to_break.completed_session_type, SessionType::Work);
        assert_eq!(work_to_break.next_status, TimerStatus::OnBreak);
        assert_eq!(work_to_break.next_session_type, SessionType::LongBreak);
        assert!(!work_to_break.requires_manual_continue_decision);

        let waiting = SessionCompletedPayload::break_waiting_decision(SessionType::ShortBreak);
        assert_eq!(
            waiting.transition_reason,
            SessionTransitionReason::BreakWaitingDecision
        );
        assert_eq!(waiting.completed_session_type, SessionType::ShortBreak);
        assert_eq!(waiting.next_status, TimerStatus::Idle);
        assert_eq!(waiting.next_session_type, SessionType::Work);
        assert!(waiting.requires_manual_continue_decision);
    }
}
