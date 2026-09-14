use tauri::{Manager, WebviewWindow};

use crate::commands::error_response::{BackendErrorCode, BackendErrorResponse};
use crate::external_adapters::emergency_shortcut::{
    EmergencyShortcutState, EmergencyShortcutStatus,
};
use crate::external_adapters::tray_icon;

#[tauri::command]
pub(crate) async fn get_emergency_shortcut_status(
    window: WebviewWindow,
) -> Result<EmergencyShortcutStatus, BackendErrorResponse> {
    if window.label() != "main" {
        return Err(BackendErrorResponse::from_code(
            BackendErrorCode::UnauthorizedWindow,
        ));
    }
    Ok(window.state::<EmergencyShortcutState>().0.lock().clone())
}

#[tauri::command]
pub(crate) async fn has_input_monitor_failed(
    window: WebviewWindow,
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
pub(crate) async fn is_tray_available(window: WebviewWindow) -> Result<bool, BackendErrorResponse> {
    if window.label() != "main" {
        return Err(BackendErrorResponse::from_code(
            BackendErrorCode::UnauthorizedWindow,
        ));
    }
    Ok(tray_icon::is_tray_available(window.app_handle()))
}

#[tauri::command]
pub(crate) async fn exit_without_tray(window: WebviewWindow) -> Result<(), BackendErrorResponse> {
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
