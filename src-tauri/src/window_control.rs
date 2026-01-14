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

/// Enter fullscreen mode on all monitors
///
/// Creates fullscreen windows on all available monitors to prevent users
/// from working on secondary displays during break time.
#[tauri::command]
pub async fn enter_fullscreen_all_monitors<R: Runtime>(
    app: AppHandle<R>,
) -> Result<(), String> {
    use tauri::WebviewUrl;
    use tauri::WebviewWindowBuilder;

    // Get main window and set it to fullscreen
    let main_window = app
        .get_webview_window("main")
        .ok_or_else(|| "Main window not found".to_string())?;

    // Set main window to fullscreen and always on top
    main_window
        .set_fullscreen(true)
        .map_err(|e| format!("Failed to set main window fullscreen: {}", e))?;

    main_window
        .set_always_on_top(true)
        .map_err(|e| format!("Failed to set main window always on top: {}", e))?;

    // Get all available monitors
    let monitors = main_window
        .available_monitors()
        .map_err(|e| format!("Failed to get monitors: {}", e))?;

    // Get the monitor where main window is currently displayed
    let main_monitor = main_window
        .current_monitor()
        .map_err(|e| format!("Failed to get current monitor: {}", e))?;

    // Create overlay windows for other monitors
    for (index, monitor) in monitors.iter().enumerate() {
        // Skip the monitor where main window is displayed
        if let (Some(ref main_mon), Some(mon_name)) = (&main_monitor, monitor.name()) {
            if let Some(main_name) = main_mon.name() {
                if main_name == mon_name {
                    continue;
                }
            }
        }

        let window_label = format!("break-overlay-{}", index);

        // Check if window already exists
        if app.get_webview_window(&window_label).is_some() {
            continue;
        }

        let position = monitor.position();
        let size = monitor.size();

        // Create overlay window for this monitor
        WebviewWindowBuilder::new(&app, &window_label, WebviewUrl::App("break-overlay.html".into()))
            .title("Break Overlay")
            .position(position.x as f64, position.y as f64)
            .inner_size(size.width as f64, size.height as f64)
            .fullscreen(true)
            .always_on_top(true)
            .decorations(false)
            .resizable(false)
            .skip_taskbar(true)
            .build()
            .map_err(|e| format!("Failed to create overlay window: {}", e))?;
    }

    Ok(())
}

/// Exit fullscreen mode on all monitors
///
/// Closes all overlay windows and restores main window to normal state.
#[tauri::command]
pub async fn exit_fullscreen_all_monitors<R: Runtime>(
    app: AppHandle<R>,
) -> Result<(), String> {
    // Close all overlay windows
    let windows = app.webview_windows();
    for (label, window) in windows.iter() {
        if label.starts_with("break-overlay-") {
            window
                .close()
                .map_err(|e| format!("Failed to close overlay window: {}", e))?;
        }
    }

    // Restore main window
    let main_window = app
        .get_webview_window("main")
        .ok_or_else(|| "Main window not found".to_string())?;

    main_window
        .set_always_on_top(false)
        .map_err(|e| format!("Failed to disable always on top: {}", e))?;

    main_window
        .set_fullscreen(false)
        .map_err(|e| format!("Failed to exit fullscreen: {}", e))?;

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
