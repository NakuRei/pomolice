// Window State Manager
// Manages window state and fullscreen mode without global statics

use tauri::{
    AppHandle, Manager, PhysicalPosition, PhysicalSize, Runtime, WebviewUrl, WebviewWindowBuilder,
};

use crate::error::{PomoliceError, Result};

#[derive(Clone)]
struct WindowState {
    position: PhysicalPosition<i32>,
    size: PhysicalSize<u32>,
}

/// Manager for window state
pub struct WindowStateManager {
    saved_state: Option<WindowState>,
}

impl WindowStateManager {
    /// Create a new window state manager
    pub fn new() -> Self {
        Self { saved_state: None }
    }

    /// Enter fullscreen mode on all monitors
    pub async fn enter_fullscreen_all_monitors<R: Runtime>(
        &mut self,
        app: AppHandle<R>,
    ) -> Result<()> {
        // Get main window and set it to fullscreen
        let main_window = app
            .get_webview_window("main")
            .ok_or_else(|| PomoliceError::WindowControl("Main window not found".to_string()))?;

        // Restore window from minimized state FIRST
        main_window
            .unminimize()
            .map_err(|e| PomoliceError::WindowControl(format!("Failed to unminimize: {}", e)))?;

        // Wait for OS to update window state
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;

        // Save current window position and size
        let current_position = main_window
            .outer_position()
            .map_err(|e| PomoliceError::WindowControl(format!("Failed to get position: {}", e)))?;
        let current_size = main_window
            .outer_size()
            .map_err(|e| PomoliceError::WindowControl(format!("Failed to get size: {}", e)))?;

        // Store the state (only if values are valid)
        if current_position.x > -30000 && current_size.width > 0 {
            self.saved_state = Some(WindowState {
                position: current_position,
                size: current_size,
            });
        }

        // Hide the window briefly
        main_window
            .hide()
            .map_err(|e| PomoliceError::WindowControl(format!("Failed to hide: {}", e)))?;

        tokio::time::sleep(std::time::Duration::from_millis(50)).await;

        // Disable decorations and resizing
        main_window.set_decorations(false).map_err(|e| {
            PomoliceError::WindowControl(format!("Failed to set decorations: {}", e))
        })?;

        main_window
            .set_resizable(false)
            .map_err(|e| PomoliceError::WindowControl(format!("Failed to set resizable: {}", e)))?;

        // Get the monitor where main window is currently displayed
        let main_monitor = main_window
            .current_monitor()
            .map_err(|e| PomoliceError::WindowControl(format!("Failed to get monitor: {}", e)))?
            .or_else(|| main_window.primary_monitor().ok().flatten())
            .or_else(|| {
                main_window
                    .available_monitors()
                    .ok()
                    .and_then(|monitors| monitors.into_iter().next())
            });

        // Reset window position and size to match the monitor
        if let Some(monitor) = &main_monitor {
            let position = monitor.position();
            let size = monitor.size();

            main_window
                .set_position(tauri::Position::Physical(tauri::PhysicalPosition {
                    x: position.x,
                    y: position.y,
                }))
                .map_err(|e| {
                    PomoliceError::WindowControl(format!("Failed to set position: {}", e))
                })?;

            main_window
                .set_size(tauri::Size::Physical(tauri::PhysicalSize {
                    width: size.width,
                    height: size.height,
                }))
                .map_err(|e| PomoliceError::WindowControl(format!("Failed to set size: {}", e)))?;
        }

        // Set main window to fullscreen and always on top
        main_window.set_fullscreen(true).map_err(|e| {
            PomoliceError::WindowControl(format!("Failed to set fullscreen: {}", e))
        })?;

        main_window.set_always_on_top(true).map_err(|e| {
            PomoliceError::WindowControl(format!("Failed to set always on top: {}", e))
        })?;

        let _ = main_window.set_visible_on_all_workspaces(true);
        let _ = main_window.set_skip_taskbar(true);

        // Show the window again
        main_window
            .show()
            .map_err(|e| PomoliceError::WindowControl(format!("Failed to show: {}", e)))?;

        // Retry focus
        for _ in 0..5 {
            if main_window.set_focus().is_ok() {
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        }

        // Get all available monitors
        let monitors = main_window
            .available_monitors()
            .map_err(|e| PomoliceError::WindowControl(format!("Failed to get monitors: {}", e)))?;

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
            WebviewWindowBuilder::new(
                &app,
                &window_label,
                WebviewUrl::App("break-overlay.html".into()),
            )
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
            .map_err(|e| {
                PomoliceError::WindowControl(format!("Failed to create overlay: {}", e))
            })?;
        }

        Ok(())
    }

    /// Exit fullscreen mode on all monitors
    pub async fn exit_fullscreen_all_monitors<R: Runtime>(
        &mut self,
        app: AppHandle<R>,
    ) -> Result<()> {
        // Close all overlay windows
        let windows = app.webview_windows();
        for (label, window) in windows.iter() {
            if label.starts_with("break-overlay-") {
                window.close().map_err(|e| {
                    PomoliceError::WindowControl(format!("Failed to close overlay: {}", e))
                })?;
            }
        }

        // Restore main window
        let main_window = app
            .get_webview_window("main")
            .ok_or_else(|| PomoliceError::WindowControl("Main window not found".to_string()))?;

        main_window.set_always_on_top(false).map_err(|e| {
            PomoliceError::WindowControl(format!("Failed to disable always on top: {}", e))
        })?;

        main_window.set_fullscreen(false).map_err(|e| {
            PomoliceError::WindowControl(format!("Failed to exit fullscreen: {}", e))
        })?;

        let _ = main_window.set_visible_on_all_workspaces(false);
        let _ = main_window.set_skip_taskbar(false);

        // Re-enable decorations and resizing
        main_window.set_decorations(true).map_err(|e| {
            PomoliceError::WindowControl(format!("Failed to enable decorations: {}", e))
        })?;

        main_window.set_resizable(true).map_err(|e| {
            PomoliceError::WindowControl(format!("Failed to enable resizable: {}", e))
        })?;

        // Restore window to previous position and size
        if let Some(state) = &self.saved_state {
            main_window
                .set_size(tauri::Size::Physical(state.size))
                .map_err(|e| {
                    PomoliceError::WindowControl(format!("Failed to restore size: {}", e))
                })?;

            main_window
                .set_position(tauri::Position::Physical(state.position))
                .map_err(|e| {
                    PomoliceError::WindowControl(format!("Failed to restore position: {}", e))
                })?;
        } else {
            // Fallback: restore to default size and center
            main_window
                .set_size(tauri::Size::Logical(tauri::LogicalSize {
                    width: 800.0,
                    height: 600.0,
                }))
                .map_err(|e| {
                    PomoliceError::WindowControl(format!("Failed to restore size: {}", e))
                })?;

            main_window
                .center()
                .map_err(|e| PomoliceError::WindowControl(format!("Failed to center: {}", e)))?;
        }

        Ok(())
    }
}

impl Default for WindowStateManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_window_state_manager_initialization() {
        let manager = WindowStateManager::new();
        assert!(manager.saved_state.is_none());
    }

    #[test]
    fn test_window_state_manager_default() {
        let manager = WindowStateManager::default();
        assert!(manager.saved_state.is_none());
    }

    // Note: enter_fullscreen_all_monitors and exit_fullscreen_all_monitors
    // require real Tauri AppHandle and cannot be unit tested without integration tests.
    // These functions are tested during manual integration testing.
}
