use crate::external_adapters::emergency_shortcut::EmergencyShortcutStatus;
use crate::timer_runtime::observation::TimerSnapshot;
use pomolice_core::timer_core::observation::SessionCompletedPayload;

#[derive(Clone, serde::Serialize, specta::Type)]
#[serde(transparent)]
pub(crate) struct TimerSnapshotChanged(pub TimerSnapshot);
impl tauri_specta::Event for TimerSnapshotChanged {
    const NAME: &'static str = "timer_snapshot_changed";
}

#[derive(Clone, serde::Serialize, specta::Type)]
#[serde(transparent)]
pub(crate) struct SessionCompleted(pub SessionCompletedPayload);
impl tauri_specta::Event for SessionCompleted {
    const NAME: &'static str = "session_completed";
}

#[derive(Clone, serde::Serialize, specta::Type)]
#[serde(transparent)]
pub(crate) struct EmergencyShortcutChanged(pub EmergencyShortcutStatus);
impl tauri_specta::Event for EmergencyShortcutChanged {
    const NAME: &'static str = "emergency_shortcut_changed";
}

#[derive(Clone, serde::Serialize, specta::Type)]
pub(crate) struct AutoRunStopPending;
impl tauri_specta::Event for AutoRunStopPending {
    const NAME: &'static str = "auto_run_stop_pending";
}

#[derive(Clone, serde::Serialize, specta::Type)]
pub(crate) struct AutoRunStopped;
impl tauri_specta::Event for AutoRunStopped {
    const NAME: &'static str = "auto_run_stopped";
}

#[derive(Clone, serde::Serialize, specta::Type)]
pub(crate) struct InputMonitorFailed;
impl tauri_specta::Event for InputMonitorFailed {
    const NAME: &'static str = "input_monitor_failed";
}
