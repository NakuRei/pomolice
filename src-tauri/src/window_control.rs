// Window control module for Phase 3: Fullscreen Break Enforcement
// Provides commands to control window state: fullscreen, always-on-top, etc.

use tauri::{AppHandle, Manager, Runtime};

/// Enter fullscreen mode with always-on-top
///
/// This command makes the window fullscreen and ensures it stays on top
/// of all other windows, making it difficult for users to switch away.
#[tauri::command]
pub async fn enter_fullscreen<R: Runtime>(
    app: AppHandle<R>,
) -> Result<(), String> {
    let window = app
        .get_webview_window("main")
        .ok_or_else(|| "Main window not found".to_string())?;

    // Set fullscreen mode
    window
        .set_fullscreen(true)
        .map_err(|e| format!("Failed to set fullscreen: {}", e))?;

    // Set always on top
    window
        .set_always_on_top(true)
        .map_err(|e| format!("Failed to set always on top: {}", e))?;

    Ok(())
}

/// Exit fullscreen mode and restore normal window state
///
/// This command restores the window to its normal state after break completion.
#[tauri::command]
pub async fn exit_fullscreen<R: Runtime>(
    app: AppHandle<R>,
) -> Result<(), String> {
    let window = app
        .get_webview_window("main")
        .ok_or_else(|| "Main window not found".to_string())?;

    // Disable always on top
    window
        .set_always_on_top(false)
        .map_err(|e| format!("Failed to disable always on top: {}", e))?;

    // Exit fullscreen mode
    window
        .set_fullscreen(false)
        .map_err(|e| format!("Failed to exit fullscreen: {}", e))?;

    Ok(())
}

/// Set always-on-top behavior
///
/// Utility command to control always-on-top independently.
#[tauri::command]
pub async fn set_always_on_top<R: Runtime>(
    app: AppHandle<R>,
    always_on_top: bool,
) -> Result<(), String> {
    let window = app
        .get_webview_window("main")
        .ok_or_else(|| "Main window not found".to_string())?;

    window
        .set_always_on_top(always_on_top)
        .map_err(|e| format!("Failed to set always on top: {}", e))?;

    Ok(())
}

/// Restore window to normal state
///
/// Ensures window is visible, focused, and in normal mode (not minimized).
#[tauri::command]
pub async fn restore_window<R: Runtime>(
    app: AppHandle<R>,
) -> Result<(), String> {
    let window = app
        .get_webview_window("main")
        .ok_or_else(|| "Main window not found".to_string())?;

    // Show the window if hidden
    window
        .show()
        .map_err(|e| format!("Failed to show window: {}", e))?;

    // Unminimize if minimized
    window
        .unminimize()
        .map_err(|e| format!("Failed to unminimize window: {}", e))?;

    // Bring to front and focus
    window
        .set_focus()
        .map_err(|e| format!("Failed to focus window: {}", e))?;

    Ok(())
}

#[cfg(test)]
mod tests {
    // Note: Window control tests require a running Tauri application
    // These are placeholder tests that verify function signatures
    // Real testing will be done via manual testing and integration tests

    #[test]
    fn test_window_control_module_exists() {
        // This test just verifies the module compiles
        assert!(true);
    }
}
