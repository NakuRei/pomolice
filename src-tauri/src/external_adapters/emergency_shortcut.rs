use crate::ipc::responses::EmergencyShortcutStatus;
use parking_lot::Mutex;
use tauri::Manager;
#[cfg(target_os = "linux")]
use tauri_specta::Event;

#[cfg(target_os = "linux")]
mod portal;

pub(crate) struct EmergencyShortcutState(pub(crate) Mutex<EmergencyShortcutStatus>);

pub(crate) fn uses_portal() -> bool {
    #[cfg(target_os = "linux")]
    {
        std::env::var("XDG_SESSION_TYPE").is_ok_and(|value| value == "wayland")
            || std::env::var("WAYLAND_DISPLAY").is_ok_and(|value| !value.is_empty())
    }
    #[cfg(not(target_os = "linux"))]
    false
}

pub(crate) fn setup(app: &tauri::AppHandle) {
    let initial = EmergencyShortcutStatus::InputMonitor;
    #[cfg(target_os = "linux")]
    let initial = if uses_portal() {
        EmergencyShortcutStatus::Registering
    } else {
        initial
    };
    app.manage(EmergencyShortcutState(Mutex::new(initial)));

    #[cfg(target_os = "linux")]
    if uses_portal() {
        let app = app.clone();
        tauri::async_runtime::spawn(async move {
            let result = async {
                let connection = zbus::Connection::session().await?;
                portal::run(
                    connection,
                    &app.config().identifier,
                    |status| publish_status(&app, status),
                    || {
                        crate::app_effects::dispatch_from_callback(
                            app.clone(),
                            crate::timer_core::Event::EscapeComboPressed,
                        );
                    },
                )
                .await
            }
            .await;
            if let Err(error) = result {
                log::warn!("Emergency shortcut unavailable: {error}");
                publish_status(&app, EmergencyShortcutStatus::Unavailable);
            }
        });
    }
}

#[cfg(target_os = "linux")]
fn publish_status(app: &tauri::AppHandle, status: EmergencyShortcutStatus) {
    *app.state::<EmergencyShortcutState>().0.lock() = status.clone();
    let _ = crate::ipc::events::EmergencyShortcutChanged(status).emit_to(app, "main");
}
