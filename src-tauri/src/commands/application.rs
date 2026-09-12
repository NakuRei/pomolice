use tauri::{Manager, WebviewWindow};

use crate::commands::error_response::{BackendErrorCode, BackendErrorResponse};
use crate::external_adapters::tray_icon;

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
