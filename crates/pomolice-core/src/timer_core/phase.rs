use chrono::{DateTime, Utc};

use crate::settings::ranges::bounded_u64_newtype;
use crate::settings::{LongBreakDurationSecs, ShortBreakDurationSecs, WorkDurationSecs};
use serde::{Deserialize, Serialize};

/// Timer status representing the current phase
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "snake_case")]
pub enum TimerStatus {
    Idle,
    Working,
    OnBreak,
}

/// Session type representing work or break sessions
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "snake_case")]
pub enum SessionType {
    Work,
    ShortBreak,
    LongBreak,
}

// Covers every session kind, so it is bounded by the longest one.
bounded_u64_newtype!(
    SessionDurationSecs,
    "session_duration_secs",
    1,
    WorkDurationSecs::MAX
);

// The conversions below are infallible only while every session kind fits
// inside the session range.
const _: () = assert!(
    SessionDurationSecs::MIN <= WorkDurationSecs::MIN
        && WorkDurationSecs::MAX <= SessionDurationSecs::MAX
        && SessionDurationSecs::MIN <= ShortBreakDurationSecs::MIN
        && ShortBreakDurationSecs::MAX <= SessionDurationSecs::MAX
        && SessionDurationSecs::MIN <= LongBreakDurationSecs::MIN
        && LongBreakDurationSecs::MAX <= SessionDurationSecs::MAX
);

impl From<WorkDurationSecs> for SessionDurationSecs {
    fn from(value: WorkDurationSecs) -> Self {
        Self(value.get())
    }
}

impl From<ShortBreakDurationSecs> for SessionDurationSecs {
    fn from(value: ShortBreakDurationSecs) -> Self {
        Self(value.get())
    }
}

impl From<LongBreakDurationSecs> for SessionDurationSecs {
    fn from(value: LongBreakDurationSecs) -> Self {
        Self(value.get())
    }
}

/// Whether the user is at the desk during the current break.
///
/// `Away` keeps the break running but makes its end wait for a manual
/// decision even under the automatic post-break policy, so an unattended
/// machine does not run through work sessions nobody is doing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum BreakAttendance {
    #[default]
    AtDesk,
    Away,
}

impl BreakAttendance {
    #[must_use]
    pub fn from_away_flag(away: bool) -> Self {
        if away {
            Self::Away
        } else {
            Self::AtDesk
        }
    }

    #[must_use]
    pub fn is_away(self) -> bool {
        self == Self::Away
    }
}

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
    /// Fixed when the break starts; settings saved during the break do not
    /// change it, so they cannot shorten the break.
    pub(super) duration_secs: SessionDurationSecs,
    pub(super) remaining_seconds: u64,
    pub(super) interaction_count: u32,
    pub(super) last_penalty_at: Option<DateTime<Utc>>,
    pub(super) attendance: BreakAttendance,
}

impl BreakSessionState {
    pub(super) fn starting(
        kind: BreakKind,
        duration_secs: SessionDurationSecs,
        remaining_seconds: u64,
    ) -> Self {
        Self {
            kind,
            duration_secs,
            remaining_seconds,
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
    /// Remaining time is measured against the wall clock. The duration is
    /// fixed when the session starts.
    Working {
        ends_at: DateTime<Utc>,
        duration_secs: SessionDurationSecs,
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

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn session_duration_rejects_out_of_range_persisted_values() {
        assert!(serde_json::from_value::<SessionDurationSecs>(json!(0)).is_err());
        assert!(serde_json::from_value::<SessionDurationSecs>(json!(7_201)).is_err());
    }
}
