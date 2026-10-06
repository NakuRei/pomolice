use tauri::{AppHandle, Manager, Runtime};

/// Bring the main window to the front, even when it is hidden to the tray or minimized.
pub(crate) fn reveal<R: Runtime>(app: &AppHandle<R>) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.show();
        let _ = window.unminimize();
        let _ = window.set_focus();
    }
}
