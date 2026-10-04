use super::authorize_main_window;
use crate::ipc::result::CommandResponse;
use tauri::{Manager, Runtime, WebviewWindow};

use crate::external_adapters::emergency_shortcut::EmergencyShortcutState;
use crate::external_adapters::emergency_shortcut::EmergencyShortcutStatus;
use crate::external_adapters::tray_icon;
use crate::ipc::errors::BackendErrorResponse;

#[tauri::command]
#[specta::specta]
pub(crate) async fn get_emergency_shortcut_status<R: Runtime>(
    window: WebviewWindow<R>,
) -> CommandResponse<EmergencyShortcutStatus> {
    if let Err(error) = authorize_main_window(&window, "get_emergency_shortcut_status") {
        return CommandResponse::Rejected {
            error: BackendErrorResponse::from(&error),
        };
    }
    CommandResponse::Accepted {
        value: window.state::<EmergencyShortcutState>().0.lock().clone(),
    }
}

#[tauri::command]
#[specta::specta]
pub(crate) async fn has_input_monitor_failed<R: Runtime>(
    window: WebviewWindow<R>,
) -> CommandResponse<bool> {
    if let Err(error) = authorize_main_window(&window, "has_input_monitor_failed") {
        return CommandResponse::Rejected {
            error: BackendErrorResponse::from(&error),
        };
    }
    let state = window.state::<crate::state::AppState>();
    let monitor = state.input_monitor.lock().await;
    CommandResponse::Accepted {
        value: monitor.has_observed_failure(),
    }
}

#[tauri::command]
#[specta::specta]
pub(crate) async fn is_tray_available<R: Runtime>(
    window: WebviewWindow<R>,
) -> CommandResponse<bool> {
    if let Err(error) = authorize_main_window(&window, "is_tray_available") {
        return CommandResponse::Rejected {
            error: BackendErrorResponse::from(&error),
        };
    }
    CommandResponse::Accepted {
        value: tray_icon::is_tray_available(window.app_handle()),
    }
}

#[tauri::command]
#[specta::specta]
pub(crate) async fn exit_without_tray<R: Runtime>(window: WebviewWindow<R>) -> CommandResponse<()> {
    if let Err(error) = authorize_main_window(&window, "exit_without_tray") {
        return CommandResponse::Rejected {
            error: BackendErrorResponse::from(&error),
        };
    }
    let app = window.app_handle();
    if tray_icon::is_tray_available(app) {
        return CommandResponse::Rejected {
            error: BackendErrorResponse::InvalidStateTransition,
        };
    }
    app.exit(0);
    CommandResponse::Accepted { value: () }
}
