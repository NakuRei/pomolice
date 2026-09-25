use chrono::{DateTime, Local};

use crate::types::{BreakAttendance, SessionType, TimerStatus};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum BreakKind {
    Short,
    Long,
}

impl BreakKind {
    pub(super) fn session_type(self) -> SessionType {
        match self {
            Self::Short => SessionType::ShortBreak,
            Self::Long => SessionType::LongBreak,
        }
    }
}

/// Created with the break and dropped with it, so nothing here can leak into
/// the next session.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct BreakSessionState {
    pub(super) kind: BreakKind,
    /// Counts down one per observed tick.
    pub(super) remaining_seconds: u64,
    pub(super) is_fullscreen: bool,
    pub(super) interaction_count: u32,
    pub(super) last_penalty_at: Option<DateTime<Local>>,
    pub(super) attendance: BreakAttendance,
}

impl BreakSessionState {
    pub(super) fn starting(kind: BreakKind, remaining_seconds: u64) -> Self {
        Self {
            kind,
            remaining_seconds,
            is_fullscreen: false,
            interaction_count: 0,
            last_penalty_at: None,
            attendance: BreakAttendance::AtDesk,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum TimerPhase {
    /// The idle preview shows the configured work duration.
    Idle {
        post_break_decision_pending: bool,
    },
    /// Remaining time is measured against the wall clock.
    Working {
        ends_at: DateTime<Local>,
    },
    OnBreak(BreakSessionState),
}

impl TimerPhase {
    pub(super) fn status(&self) -> TimerStatus {
        match self {
            Self::Idle { .. } => TimerStatus::Idle,
            Self::Working { .. } => TimerStatus::Working,
            Self::OnBreak(_) => TimerStatus::OnBreak,
        }
    }

    pub(super) fn post_break_decision_pending(&self) -> bool {
        matches!(
            self,
            Self::Idle {
                post_break_decision_pending: true
            }
        )
    }

    pub(super) fn current_break(&self) -> Option<&BreakSessionState> {
        match self {
            Self::OnBreak(current_break) => Some(current_break),
            Self::Idle { .. } | Self::Working { .. } => None,
        }
    }

    pub(super) fn current_break_mut(&mut self) -> Option<&mut BreakSessionState> {
        match self {
            Self::OnBreak(current_break) => Some(current_break),
            Self::Idle { .. } | Self::Working { .. } => None,
        }
    }
}
