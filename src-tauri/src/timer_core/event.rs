use crate::settings::UserSettings;
use crate::types::{BreakAttendance, PostBreakDecisionAction};

/// Every input that can change timer state. The shell turns commands, the
/// ticker and input-monitor callbacks into one of these.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Event {
    /// Applied once after restore, before the ticker starts.
    Started,
    Tick,
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
