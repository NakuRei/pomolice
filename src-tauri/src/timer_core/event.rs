use crate::types::{
    BreakAttendance, LongBreakDurationSecs, LongBreakIntervalCount, PostBreakDecisionAction,
    PostBreakTransitionPolicy, ShortBreakDurationSecs, TimerSettings, WeeklyAutoRunConfig,
    WorkDurationSecs,
};

/// Every input that can change timer state. The shell turns commands, the
/// ticker, input-monitor callbacks and break-lock outcomes into one of these.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Event {
    /// Applied once after restore, before the ticker starts.
    Started,
    Tick,
    StartWorkRequested {
        settings: TimerSettings,
    },
    /// The frontend fetched the full state. Only the first observation decides
    /// whether the boot auto-start prompt is offered.
    FirstFullStateObserved,
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
    TimerDurationSettingsChanged {
        work_duration_secs: WorkDurationSecs,
        short_break_duration_secs: ShortBreakDurationSecs,
        long_break_duration_secs: LongBreakDurationSecs,
        long_break_interval: LongBreakIntervalCount,
    },
    AutomationPoliciesChanged {
        auto_run_feature_enabled: bool,
        weekly_auto_run_config: WeeklyAutoRunConfig,
        post_break_transition_policy: PostBreakTransitionPolicy,
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
