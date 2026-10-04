use crate::settings::UserSettings;
use crate::timer_core::BreakAttendance;
use serde::{Deserialize, Serialize};

/// Action selected while a post-break manual decision is pending.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "snake_case")]
pub enum PostBreakDecisionAction {
    Continue,
    StopCycle,
}

/// Every input that can change timer state. The shell turns commands, the
/// ticker and input-monitor callbacks into one of these.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Event {
    /// Reevaluate wall time and credit only the explicitly supplied break seconds.
    /// Zero reevaluates deadlines and schedules without consuming a running break.
    /// Credit applies only to a break already in progress; excess seconds are discarded.
    TimeObserved {
        credited_break_seconds: u64,
    },
    StartWorkRequested,
    BootPromptResolved {
        start_now: bool,
    },
    PostBreakDecisionResolved {
        action: PostBreakDecisionAction,
    },
    BreakAttendanceSet {
        attendance: BreakAttendance,
    },
    EmergencyStopRequested,
    /// Accepted in any state. A session already running keeps the duration
    /// it started with; the new durations apply from the next session.
    SettingsChanged {
        settings: UserSettings,
    },
    InputDetected,
    EscapeComboPressed,
}
