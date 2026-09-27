use crate::types::{
    ActiveWindowChangedPayload, SessionCompletedPayload, SettingsChangedPayload, TimerTickPayload,
};

/// Every side effect a transition can request. The shell runs them in order
/// while holding the dispatch permit, so their order is the observable order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Effect {
    PublishTimerState(TimerTickPayload),
    EmitSessionCompleted(SessionCompletedPayload),
    EmitAutoRunStopPending,
    EmitAutoRunStopped,
    EmitActiveWindowChanged(ActiveWindowChangedPayload),
    EmitSettingsChanged(SettingsChangedPayload),
    EmitBootAutoStartOfferClosed,
    Persist,
    /// Start input monitoring and lock every monitor. Reports back as
    /// `Event::BreakLockEntered` or `Event::BreakLockEnterFailed`.
    EnterBreakLock,
    /// Stop input monitoring and release the lock. Reports back as
    /// `Event::BreakLockReleased` or `Event::BreakLockReleaseFailed`.
    ReleaseBreakLock,
}
