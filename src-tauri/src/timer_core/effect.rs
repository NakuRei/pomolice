use serde::Serialize;

use crate::ipc::responses::{
    ActiveWindowChangedPayload, SessionCompletedPayload, SettingsChangedPayload, TimerTickPayload,
};

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "snake_case")]
pub(crate) enum BreakLockTarget {
    Locked,
    #[default]
    Unlocked,
}

/// Every side effect a transition can request. The shell runs them in order
/// while holding the dispatch permit, so their order is the observable order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Effect {
    PublishTimerState(TimerTickPayload),
    EmitSessionCompleted(SessionCompletedPayload),
    EmitAutoRunStopPending,
    EmitAutoRunStopped,
    EmitActiveWindowChanged(ActiveWindowChangedPayload),
    EmitSettingsChanged(Box<SettingsChangedPayload>),
    EmitBootAutoStartOfferClosed,
    Persist,
    SetBreakLockTarget(BreakLockTarget),
}
