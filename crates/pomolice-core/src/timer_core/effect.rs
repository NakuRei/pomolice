use serde::Serialize;

use crate::timer_core::observation::SessionCompletedPayload;

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "snake_case")]
pub enum BreakLockTarget {
    Locked,
    #[default]
    Unlocked,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Effect {
    EmitSessionCompleted(SessionCompletedPayload),
    EmitAutoRunStopPending,
    EmitAutoRunStopped,
    Persist,
    SetBreakLockTarget(BreakLockTarget),
}
