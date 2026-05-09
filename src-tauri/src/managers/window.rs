// Window State Manager
// Manages window state and fullscreen mode without global statics

use std::time::Duration;

use tauri::{
    AppHandle, Manager, Monitor, PhysicalPosition, PhysicalSize, Runtime, WebviewUrl,
    WebviewWindow, WebviewWindowBuilder,
};

use crate::error::{PomoliceError, Result};

#[derive(Clone)]
pub(crate) struct WindowState {
    position: PhysicalPosition<i32>,
    size: PhysicalSize<u32>,
}

/// Manager for window state
pub(crate) struct WindowStateManager {
    saved_state: Option<WindowState>,
}

impl WindowStateManager {
    /// Create a new window state manager
    pub(crate) fn new() -> Self {
        Self { saved_state: None }
    }

    /// Capture the previous main-window state before fullscreen break entry mutates layout.
    pub(crate) async fn capture_main_window_state_for_break<R: Runtime>(
        app: &AppHandle<R>,
    ) -> Result<Option<WindowState>> {
        let main_window = Self::main_window(app)?;
        Self::restore_and_capture_main_window_state(&main_window).await
    }

    /// Complete fullscreen break entry after the previous main-window state has been captured.
    pub(crate) async fn enter_fullscreen_all_monitors<R: Runtime>(app: AppHandle<R>) -> Result<()> {
        let main_window = Self::main_window(&app)?;

        let main_monitor = Self::configure_main_window_for_break(&main_window).await?;
        Self::create_overlay_windows_for_other_monitors(&app, &main_window, main_monitor.as_ref())?;

        Ok(())
    }

    fn main_window<R: Runtime>(app: &AppHandle<R>) -> Result<WebviewWindow<R>> {
        app.get_webview_window("main")
            .ok_or_else(|| PomoliceError::WindowControl("Main window not found".to_string()))
    }

    async fn restore_and_capture_main_window_state<R: Runtime>(
        main_window: &WebviewWindow<R>,
    ) -> Result<Option<WindowState>> {
        main_window
            .unminimize()
            .map_err(|e| PomoliceError::WindowControl(format!("Failed to unminimize: {e}")))?;

        tokio::time::sleep(Duration::from_millis(100)).await;
        let saved_state = Self::capture_main_window_state(main_window)?;

        main_window
            .hide()
            .map_err(|e| PomoliceError::WindowControl(format!("Failed to hide: {e}")))?;

        tokio::time::sleep(Duration::from_millis(50)).await;
        Ok(saved_state)
    }

    fn capture_main_window_state<R: Runtime>(
        main_window: &WebviewWindow<R>,
    ) -> Result<Option<WindowState>> {
        let current_position = main_window
            .outer_position()
            .map_err(|e| PomoliceError::WindowControl(format!("Failed to get position: {e}")))?;
        let current_size = main_window
            .outer_size()
            .map_err(|e| PomoliceError::WindowControl(format!("Failed to get size: {e}")))?;

        if current_position.x > -30000 && current_size.width > 0 {
            return Ok(Some(WindowState {
                position: current_position,
                size: current_size,
            }));
        }

        Ok(None)
    }

    pub(crate) fn set_saved_state(&mut self, saved_state: Option<WindowState>) {
        self.saved_state = saved_state;
    }

    async fn configure_main_window_for_break<R: Runtime>(
        main_window: &WebviewWindow<R>,
    ) -> Result<Option<Monitor>> {
        main_window
            .set_decorations(false)
            .map_err(|e| PomoliceError::WindowControl(format!("Failed to set decorations: {e}")))?;
        main_window
            .set_resizable(false)
            .map_err(|e| PomoliceError::WindowControl(format!("Failed to set resizable: {e}")))?;

        let main_monitor = Self::find_main_monitor(main_window)?;
        if let Some(monitor) = &main_monitor {
            Self::apply_monitor_bounds(main_window, monitor)?;
        }

        main_window
            .set_fullscreen(true)
            .map_err(|e| PomoliceError::WindowControl(format!("Failed to set fullscreen: {e}")))?;
        main_window.set_always_on_top(true).map_err(|e| {
            PomoliceError::WindowControl(format!("Failed to set always on top: {e}"))
        })?;

        let _ = main_window.set_visible_on_all_workspaces(true);
        let _ = main_window.set_skip_taskbar(true);

        main_window
            .show()
            .map_err(|e| PomoliceError::WindowControl(format!("Failed to show: {e}")))?;
        Self::focus_main_window_with_retry(main_window).await;

        Ok(main_monitor)
    }

    fn find_main_monitor<R: Runtime>(main_window: &WebviewWindow<R>) -> Result<Option<Monitor>> {
        let current_monitor = main_window
            .current_monitor()
            .map_err(|e| PomoliceError::WindowControl(format!("Failed to get monitor: {e}")))?;

        Ok(current_monitor
            .or_else(|| main_window.primary_monitor().ok().flatten())
            .or_else(|| {
                main_window
                    .available_monitors()
                    .ok()
                    .and_then(|monitors| monitors.into_iter().next())
            }))
    }

    fn apply_monitor_bounds<R: Runtime>(
        main_window: &WebviewWindow<R>,
        monitor: &Monitor,
    ) -> Result<()> {
        let position = monitor.position();
        let size = monitor.size();

        main_window
            .set_position(tauri::Position::Physical(tauri::PhysicalPosition {
                x: position.x,
                y: position.y,
            }))
            .map_err(|e| PomoliceError::WindowControl(format!("Failed to set position: {e}")))?;

        main_window
            .set_size(tauri::Size::Physical(tauri::PhysicalSize {
                width: size.width,
                height: size.height,
            }))
            .map_err(|e| PomoliceError::WindowControl(format!("Failed to set size: {e}")))?;

        Ok(())
    }

    async fn focus_main_window_with_retry<R: Runtime>(main_window: &WebviewWindow<R>) {
        for _ in 0..5 {
            if main_window.set_focus().is_ok() {
                break;
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
    }

    fn create_overlay_windows_for_other_monitors<R: Runtime>(
        app: &AppHandle<R>,
        main_window: &WebviewWindow<R>,
        main_monitor: Option<&Monitor>,
    ) -> Result<()> {
        let monitors = main_window
            .available_monitors()
            .map_err(|e| PomoliceError::WindowControl(format!("Failed to get monitors: {e}")))?;

        for (index, monitor) in monitors.iter().enumerate() {
            if Self::should_skip_overlay_monitor(main_monitor, monitor) {
                continue;
            }

            let window_label = format!("break-overlay-{index}");
            if app.get_webview_window(&window_label).is_some() {
                continue;
            }

            Self::create_overlay_window(app, &window_label, monitor)?;
        }

        Ok(())
    }

    fn should_skip_overlay_monitor(main_monitor: Option<&Monitor>, monitor: &Monitor) -> bool {
        let Some(main_monitor) = main_monitor else {
            return false;
        };
        let Some(main_name) = main_monitor.name() else {
            return false;
        };
        let Some(monitor_name) = monitor.name() else {
            return false;
        };

        main_name == monitor_name
    }

    fn create_overlay_window<R: Runtime>(
        app: &AppHandle<R>,
        window_label: &str,
        monitor: &Monitor,
    ) -> Result<()> {
        let position = monitor.position();
        let size = monitor.size();

        WebviewWindowBuilder::new(
            app,
            window_label,
            WebviewUrl::App("break-overlay.html".into()),
        )
        .title("Break Overlay")
        .position(f64::from(position.x), f64::from(position.y))
        .inner_size(f64::from(size.width), f64::from(size.height))
        .fullscreen(true)
        .always_on_top(true)
        .visible_on_all_workspaces(true)
        .decorations(false)
        .resizable(false)
        .skip_taskbar(true)
        .build()
        .map_err(|e| PomoliceError::WindowControl(format!("Failed to create overlay: {e}")))?;

        Ok(())
    }

    /// Exit fullscreen mode on all monitors
    pub(crate) fn exit_fullscreen_all_monitors<R: Runtime>(
        &mut self,
        app: &AppHandle<R>,
    ) -> Result<()> {
        // Close all overlay windows
        let windows = app.webview_windows();
        for (label, window) in &windows {
            if label.starts_with("break-overlay-") {
                window.close().map_err(|e| {
                    PomoliceError::WindowControl(format!("Failed to close overlay: {e}"))
                })?;
            }
        }

        // Restore main window
        let main_window = app
            .get_webview_window("main")
            .ok_or_else(|| PomoliceError::WindowControl("Main window not found".to_string()))?;

        main_window.set_always_on_top(false).map_err(|e| {
            PomoliceError::WindowControl(format!("Failed to disable always on top: {e}"))
        })?;

        main_window
            .set_fullscreen(false)
            .map_err(|e| PomoliceError::WindowControl(format!("Failed to exit fullscreen: {e}")))?;

        let _ = main_window.set_visible_on_all_workspaces(false);
        let _ = main_window.set_skip_taskbar(false);

        // Re-enable decorations and resizing
        main_window.set_decorations(true).map_err(|e| {
            PomoliceError::WindowControl(format!("Failed to enable decorations: {e}"))
        })?;

        main_window.set_resizable(true).map_err(|e| {
            PomoliceError::WindowControl(format!("Failed to enable resizable: {e}"))
        })?;

        // Restore window to previous position and size
        if let Some(state) = &self.saved_state {
            main_window
                .set_size(tauri::Size::Physical(state.size))
                .map_err(|e| {
                    PomoliceError::WindowControl(format!("Failed to restore size: {e}"))
                })?;

            main_window
                .set_position(tauri::Position::Physical(state.position))
                .map_err(|e| {
                    PomoliceError::WindowControl(format!("Failed to restore position: {e}"))
                })?;
        } else {
            // Fallback: restore to default size and center
            main_window
                .set_size(tauri::Size::Logical(tauri::LogicalSize {
                    width: 800.0,
                    height: 600.0,
                }))
                .map_err(|e| {
                    PomoliceError::WindowControl(format!("Failed to restore size: {e}"))
                })?;

            main_window
                .center()
                .map_err(|e| PomoliceError::WindowControl(format!("Failed to center: {e}")))?;
        }

        Self::restore_main_window_visibility(
            || main_window.show().map_err(|e| e.to_string()),
            || main_window.set_focus().map_err(|e| e.to_string()),
        )?;

        Ok(())
    }

    fn restore_main_window_visibility(
        show: impl FnOnce() -> std::result::Result<(), String>,
        focus: impl FnOnce() -> std::result::Result<(), String>,
    ) -> Result<()> {
        show().map_err(|error| PomoliceError::WindowControl(format!("Failed to show: {error}")))?;
        let _ = focus();
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

    #[test]
    fn test_restore_main_window_visibility_calls_show_and_tolerates_focus_failure() {
        let mut show_called = false;
        let mut focus_called = false;

        WindowStateManager::restore_main_window_visibility(
            || {
                show_called = true;
                Ok(())
            },
            || {
                focus_called = true;
                Err("focus failed".to_string())
            },
        )
        .expect("show success should keep rollback restore successful");

        assert!(show_called);
        assert!(focus_called);
    }

    #[test]
    fn test_restore_main_window_visibility_propagates_show_failure() {
        let mut focus_called = false;

        let error = WindowStateManager::restore_main_window_visibility(
            || Err("show failed".to_string()),
            || {
                focus_called = true;
                Ok(())
            },
        )
        .expect_err("show failure should fail the rollback restore");

        assert!(matches!(
            error,
            PomoliceError::WindowControl(message) if message == "Failed to show: show failed"
        ));
        assert!(!focus_called);
    }

    // Note: enter_fullscreen_all_monitors and exit_fullscreen_all_monitors
    // require real Tauri AppHandle and cannot be unit tested without integration tests.
    // These functions are tested during manual integration testing.
}
