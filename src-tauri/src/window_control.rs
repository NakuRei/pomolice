// Window control module for Phase 3: Fullscreen Break Enforcement
// Provides INTERNAL commands to control window state: fullscreen, always-on-top, etc.
// Security: These functions are NOT exposed to frontend - only callable from timer_state module

use tauri::{AppHandle, Manager, Runtime, PhysicalPosition, PhysicalSize};
use std::sync::Mutex;

// Global state to store window position and size before fullscreen
static WINDOW_STATE: Mutex<Option<WindowState>> = Mutex::new(None);

#[derive(Clone)]
struct WindowState {
    position: PhysicalPosition<i32>,
    size: PhysicalSize<u32>,
}

/// Enter fullscreen mode on all monitors
///
/// INTERNAL USE ONLY - Called by timer_state::start_break
/// Creates fullscreen windows on all available monitors to prevent users
/// from working on secondary displays during break time.
pub(crate) async fn enter_fullscreen_all_monitors<R: Runtime>(
    app: AppHandle<R>,
) -> Result<(), String> {
    use tauri::WebviewUrl;
    use tauri::WebviewWindowBuilder;

    // Get main window and set it to fullscreen
    let main_window = app
        .get_webview_window("main")
        .ok_or_else(|| "Main window not found".to_string())?;

    // Save current window position and size before fullscreen
    let current_position = main_window
        .outer_position()
        .map_err(|e| format!("Failed to get window position: {}", e))?;
    let current_size = main_window
        .outer_size()
        .map_err(|e| format!("Failed to get window size: {}", e))?;

    // Store the state globally
    {
        let mut state = WINDOW_STATE.lock().unwrap();
        *state = Some(WindowState {
            position: current_position,
            size: current_size,
        });
    }

    // First, hide the window briefly to break any ongoing OS-level resize operation
    main_window
        .hide()
        .map_err(|e| format!("Failed to hide window: {}", e))?;

    // Small delay to ensure OS releases any resize locks
    std::thread::sleep(std::time::Duration::from_millis(50));

    // Disable window decorations to prevent any resize operations
    main_window
        .set_decorations(false)
        .map_err(|e| format!("Failed to disable decorations: {}", e))?;

    // Disable resizable to stop any ongoing resize operations
    main_window
        .set_resizable(false)
        .map_err(|e| format!("Failed to disable resizable: {}", e))?;

    // Get the monitor where main window is currently displayed
    let main_monitor = main_window
        .current_monitor()
        .map_err(|e| format!("Failed to get current monitor: {}", e))?;

    // If we have a monitor, reset window position and size to match the monitor
    if let Some(monitor) = &main_monitor {
        let position = monitor.position();
        let size = monitor.size();

        // Reset window to monitor's position and size before setting fullscreen
        main_window
            .set_position(tauri::Position::Physical(tauri::PhysicalPosition {
                x: position.x,
                y: position.y,
            }))
            .map_err(|e| format!("Failed to set window position: {}", e))?;

        main_window
            .set_size(tauri::Size::Physical(tauri::PhysicalSize {
                width: size.width,
                height: size.height,
            }))
            .map_err(|e| format!("Failed to set window size: {}", e))?;
    }

    // Set main window to fullscreen and always on top
    main_window
        .set_fullscreen(true)
        .map_err(|e| format!("Failed to set main window fullscreen: {}", e))?;

    main_window
        .set_always_on_top(true)
        .map_err(|e| format!("Failed to set main window always on top: {}", e))?;

    // Best-effort: keep main visible across virtual desktops where supported
    let _ = main_window.set_visible_on_all_workspaces(true);
    let _ = main_window.set_skip_taskbar(true);

    // Show the window again
    main_window
        .show()
        .map_err(|e| format!("Failed to show window: {}", e))?;
    // Retry focus because Windows can reject focus requests immediately after show.
    for _ in 0..5 {
        if main_window.set_focus().is_ok() {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(50));
    }

    // Get all available monitors
    let monitors = main_window
        .available_monitors()
        .map_err(|e| format!("Failed to get monitors: {}", e))?;

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
            .visible_on_all_workspaces(true)
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
/// INTERNAL USE ONLY - Called by timer_state::complete_break
/// Closes all overlay windows and restores main window to normal state.
pub(crate) async fn exit_fullscreen_all_monitors<R: Runtime>(
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

    let _ = main_window.set_visible_on_all_workspaces(false);
    let _ = main_window.set_skip_taskbar(false);

    // Re-enable decorations
    main_window
        .set_decorations(true)
        .map_err(|e| format!("Failed to enable decorations: {}", e))?;

    // Re-enable resizable
    main_window
        .set_resizable(true)
        .map_err(|e| format!("Failed to enable resizable: {}", e))?;

    // Restore window to previous position and size
    let saved_state = {
        let state = WINDOW_STATE.lock().unwrap();
        state.clone()
    };

    if let Some(state) = saved_state {
        // Restore previous size
        main_window
            .set_size(tauri::Size::Physical(state.size))
            .map_err(|e| format!("Failed to restore window size: {}", e))?;

        // Restore previous position
        main_window
            .set_position(tauri::Position::Physical(state.position))
            .map_err(|e| format!("Failed to restore window position: {}", e))?;
    } else {
        // Fallback: restore to default size (800x600) and center
        main_window
            .set_size(tauri::Size::Logical(tauri::LogicalSize {
                width: 800.0,
                height: 600.0,
            }))
            .map_err(|e| format!("Failed to restore window size: {}", e))?;

        main_window
            .center()
            .map_err(|e| format!("Failed to center window: {}", e))?;
    }

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
