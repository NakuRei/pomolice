use crate::settings::UserSettings;
use crate::types::{BreakAttendance, PostBreakDecisionAction};

/// Every input that can change timer state. The shell turns commands, the
/// ticker, input-monitor callbacks and break-lock outcomes into one of these.
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
    /// Outcome of `Effect::EnterBreakLock`, fed back inside the same dispatch.
    BreakLockEntered,
    BreakLockEnterFailed,
    /// Outcome of `Effect::ReleaseBreakLock`, fed back inside the same dispatch.
    BreakLockReleased,
    BreakLockReleaseFailed,
}
