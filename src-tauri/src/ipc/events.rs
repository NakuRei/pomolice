use crate::external_adapters::emergency_shortcut::EmergencyShortcutStatus;
use crate::timer_runtime::observation::SettingsChangedPayload;
use crate::timer_runtime::observation::TimerEffectsState;
use pomolice_core::timer_core::observation::ActiveWindowChangedPayload;
use pomolice_core::timer_core::observation::SessionCompletedPayload;
use pomolice_core::timer_core::observation::TimerTickPayload;

#[derive(Clone, serde::Serialize, specta::Type)]
#[serde(transparent)]
pub(crate) struct TimerTick(pub TimerTickPayload);
impl tauri_specta::Event for TimerTick {
    const NAME: &'static str = "timer_tick";
}

#[derive(Clone, serde::Serialize, specta::Type)]
#[serde(transparent)]
pub(crate) struct SessionCompleted(pub SessionCompletedPayload);
impl tauri_specta::Event for SessionCompleted {
    const NAME: &'static str = "session_completed";
}

#[derive(Clone, serde::Serialize, specta::Type)]
#[serde(transparent)]
pub(crate) struct ActiveWindowChanged(pub ActiveWindowChangedPayload);
impl tauri_specta::Event for ActiveWindowChanged {
    const NAME: &'static str = "active_window_changed";
}

#[derive(Clone, serde::Serialize, specta::Type)]
#[serde(transparent)]
pub(crate) struct SettingsChanged(pub SettingsChangedPayload);
impl tauri_specta::Event for SettingsChanged {
    const NAME: &'static str = "settings_changed";
}

#[derive(Clone, serde::Serialize, specta::Type)]
#[serde(transparent)]
pub(crate) struct TimerEffectsChanged(pub TimerEffectsState);
impl tauri_specta::Event for TimerEffectsChanged {
    const NAME: &'static str = "timer_effects_changed";
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
pub(crate) struct BootAutoStartOfferClosed;
impl tauri_specta::Event for BootAutoStartOfferClosed {
    const NAME: &'static str = "boot_auto_start_offer_closed";
}

#[derive(Clone, serde::Serialize, specta::Type)]
pub(crate) struct InputMonitorFailed;
impl tauri_specta::Event for InputMonitorFailed {
    const NAME: &'static str = "input_monitor_failed";
}
