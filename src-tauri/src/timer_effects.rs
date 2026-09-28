use serde::Serialize;

use crate::timer_core::BreakLockTarget;
use crate::types::FullStatePayload;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum EffectStatus {
    Succeeded,
    Failed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum BreakLockAction {
    Enter,
    Release,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub(crate) struct BreakLockOutcome {
    pub action: BreakLockAction,
    pub status: EffectStatus,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize)]
pub(crate) struct BreakLockState {
    pub target: BreakLockTarget,
    pub last_attempt: Option<BreakLockOutcome>,
}

/// Latest attempts in this application run. None means not attempted;
/// a failed lock operation leaves the physical window state uncertain.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize)]
pub(crate) struct TimerEffectsState {
    pub revision: u64,
    pub break_lock: BreakLockState,
    pub last_timer_save: Option<EffectStatus>,
}

/// Reads and accepted commands return the same observation contract.
/// A rejected command changes no timer state.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub(crate) struct TimerSnapshot {
    pub timer: FullStatePayload,
    pub effects: TimerEffectsState,
}
