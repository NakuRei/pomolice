use crate::storage::SettingsFileStatus;
use pomolice_core::timer_core::{observation::TimerState, BreakLockTarget};
use serde::Serialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "snake_case")]
pub(crate) enum EffectStatus {
    Succeeded,
    Failed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "snake_case")]
pub(crate) enum BreakLockAction {
    Enter,
    Release,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, specta::Type)]
pub(crate) struct BreakLockOutcome {
    pub action: BreakLockAction,
    pub status: EffectStatus,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, specta::Type)]
pub(crate) struct BreakLockState {
    pub target: BreakLockTarget,
    pub last_attempt: Option<BreakLockOutcome>,
}

/// Latest attempts in this application run. None means not attempted;
/// a failed lock operation leaves the physical window state uncertain.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, specta::Type)]
pub(crate) struct TimerEffectsState {
    pub break_lock: BreakLockState,
    pub last_timer_save: Option<EffectStatus>,
}

/// Reads and accepted commands return the same observation contract.
/// A rejected command changes no timer state.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
pub(crate) struct TimerSnapshot {
    pub revision: SnapshotRevision,
    pub timer: FullStatePayload,
    pub effects: TimerEffectsState,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, specta::Type)]
#[specta(transparent)]
pub(crate) struct SnapshotRevision(#[specta(type = String)] pub u64);

impl Serialize for SnapshotRevision {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_str(&self.0)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
pub(crate) struct FullStatePayload {
    #[serde(flatten)]
    pub state: TimerState,
    pub settings_file_status: SettingsFileStatus,
}
