use serde::Serialize;

use crate::timer_core::observation::ActiveWindowChangedPayload;
use crate::timer_core::observation::SessionCompletedPayload;
use crate::timer_core::observation::SettingsChangedPayload;
use crate::timer_core::observation::TimerTickPayload;

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "snake_case")]
pub enum BreakLockTarget {
    Locked,
    #[default]
    Unlocked,
}

/// Every side effect a transition can request. The shell runs them in order
/// while holding the dispatch permit, so their order is the observable order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Effect {
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
