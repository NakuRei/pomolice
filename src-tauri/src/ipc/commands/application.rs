use tauri::{Manager, Runtime, WebviewWindow};

use crate::external_adapters::emergency_shortcut::EmergencyShortcutState;
use crate::external_adapters::tray_icon;
use crate::ipc::errors::{BackendErrorCode, BackendErrorResponse};
use crate::ipc::responses::EmergencyShortcutStatus;

#[tauri::command]
#[specta::specta]
pub(crate) async fn get_emergency_shortcut_status<R: Runtime>(
    window: WebviewWindow<R>,
) -> Result<EmergencyShortcutStatus, BackendErrorResponse> {
    if window.label() != "main" {
        return Err(BackendErrorResponse::from_code(
            BackendErrorCode::UnauthorizedWindow,
        ));
    }
    Ok(window.state::<EmergencyShortcutState>().0.lock().clone())
}

#[tauri::command]
#[specta::specta]
pub(crate) async fn has_input_monitor_failed<R: Runtime>(
    window: WebviewWindow<R>,
) -> Result<bool, BackendErrorResponse> {
    if window.label() != "main" {
        return Err(BackendErrorResponse::from_code(
            BackendErrorCode::UnauthorizedWindow,
        ));
    }
    let state = window.state::<crate::state::AppState>();
    let monitor = state.input_monitor.lock().await;
    Ok(monitor.has_observed_failure())
}

#[tauri::command]
#[specta::specta]
pub(crate) async fn is_tray_available<R: Runtime>(
    window: WebviewWindow<R>,
) -> Result<bool, BackendErrorResponse> {
    if window.label() != "main" {
        return Err(BackendErrorResponse::from_code(
            BackendErrorCode::UnauthorizedWindow,
        ));
    }
    Ok(tray_icon::is_tray_available(window.app_handle()))
}

#[tauri::command]
#[specta::specta]
pub(crate) async fn exit_without_tray<R: Runtime>(
    window: WebviewWindow<R>,
) -> Result<(), BackendErrorResponse> {
    if window.label() != "main" {
        return Err(BackendErrorResponse::from_code(
            BackendErrorCode::UnauthorizedWindow,
        ));
    }
    let app = window.app_handle();
    if tray_icon::is_tray_available(app) {
        return Err(BackendErrorResponse::from_code(
            BackendErrorCode::InvalidStateTransition,
        ));
    }
    app.exit(0);
    Ok(())
}
