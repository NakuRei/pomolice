// Window State Manager
// Manages window state and fullscreen mode without global statics

use std::collections::BTreeSet;
use std::time::Duration;

use tauri::{
    AppHandle, Manager, Monitor, PhysicalPosition, PhysicalSize, Runtime, WebviewUrl,
    WebviewWindow, WebviewWindowBuilder,
};

use crate::error::{PomoliceError, Result};

const UNRESTORABLE_MAIN_WINDOW_MIN_X: i32 = -30000;
const DEFAULT_RESTORE_WIDTH: f64 = 800.0;
const DEFAULT_RESTORE_HEIGHT: f64 = 600.0;
const BREAK_OVERLAY_LABEL_PREFIX: &str = "break-overlay-";

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct WindowState {
    position: PhysicalPosition<i32>,
    size: PhysicalSize<u32>,
}

impl WindowState {
    pub(crate) fn from_restorable_main_window_geometry(
        position: PhysicalPosition<i32>,
        size: PhysicalSize<u32>,
    ) -> Option<Self> {
        if position.x > UNRESTORABLE_MAIN_WINDOW_MIN_X && size.width > 0 {
            return Some(Self { position, size });
        }

        None
    }
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) enum MainWindowRestorePlan {
    Saved(WindowState),
    DefaultCentered { width: f64, height: f64 },
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct MonitorSnapshot {
    name: Option<String>,
    position: PhysicalPosition<i32>,
    size: PhysicalSize<u32>,
}

impl MonitorSnapshot {
    fn from_monitor(monitor: &Monitor) -> Self {
        Self {
            name: monitor.name().map(std::string::ToString::to_string),
            position: *monitor.position(),
            size: *monitor.size(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct BreakOverlayWindowPlan {
    label: String,
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

        Ok(WindowState::from_restorable_main_window_geometry(
            current_position,
            current_size,
        ))
    }

    pub(crate) fn set_saved_state(&mut self, saved_state: Option<WindowState>) {
        self.saved_state = saved_state;
    }

    pub(crate) fn main_window_restore_plan(&self) -> MainWindowRestorePlan {
        match &self.saved_state {
            Some(state) => MainWindowRestorePlan::Saved(state.clone()),
            None => MainWindowRestorePlan::DefaultCentered {
                width: DEFAULT_RESTORE_WIDTH,
                height: DEFAULT_RESTORE_HEIGHT,
            },
        }
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
        let monitor_snapshots: Vec<MonitorSnapshot> =
            monitors.iter().map(MonitorSnapshot::from_monitor).collect();
        let main_monitor_snapshot = main_monitor.map(MonitorSnapshot::from_monitor);
        let existing_overlay_labels: BTreeSet<String> = app
            .webview_windows()
            .keys()
            .filter(|label| label.starts_with(BREAK_OVERLAY_LABEL_PREFIX))
            .cloned()
            .collect();
        let overlay_window_plans = Self::build_break_overlay_window_plans(
            main_monitor_snapshot.as_ref(),
            &monitor_snapshots,
            &existing_overlay_labels,
        );

        for overlay_window_plan in overlay_window_plans {
            Self::create_overlay_window(app, &overlay_window_plan)?;
        }

        Ok(())
    }

    fn build_break_overlay_window_plans(
        main_monitor: Option<&MonitorSnapshot>,
        monitors: &[MonitorSnapshot],
        existing_overlay_labels: &BTreeSet<String>,
    ) -> Vec<BreakOverlayWindowPlan> {
        monitors
            .iter()
            .enumerate()
            .filter_map(|(index, monitor)| {
                let label = format!("{BREAK_OVERLAY_LABEL_PREFIX}{index}");

                if Self::is_confirmed_main_monitor(main_monitor, monitor)
                    || existing_overlay_labels.contains(&label)
                {
                    return None;
                }

                Some(BreakOverlayWindowPlan {
                    label,
                    position: monitor.position,
                    size: monitor.size,
                })
            })
            .collect()
    }

    fn is_confirmed_main_monitor(
        main_monitor: Option<&MonitorSnapshot>,
        monitor: &MonitorSnapshot,
    ) -> bool {
        let Some(main_monitor) = main_monitor else {
            return false;
        };

        matches!(
            (&main_monitor.name, &monitor.name),
            (Some(main_name), Some(monitor_name)) if main_name == monitor_name
        )
    }

    fn create_overlay_window<R: Runtime>(
        app: &AppHandle<R>,
        overlay_window_plan: &BreakOverlayWindowPlan,
    ) -> Result<()> {
        WebviewWindowBuilder::new(
            app,
            &overlay_window_plan.label,
            WebviewUrl::App("break-overlay.html".into()),
        )
        .title("Break Overlay")
        .position(
            f64::from(overlay_window_plan.position.x),
            f64::from(overlay_window_plan.position.y),
        )
        .inner_size(
            f64::from(overlay_window_plan.size.width),
            f64::from(overlay_window_plan.size.height),
        )
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
            if label.starts_with(BREAK_OVERLAY_LABEL_PREFIX) {
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

        match self.main_window_restore_plan() {
            MainWindowRestorePlan::Saved(state) => {
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
            }
            MainWindowRestorePlan::DefaultCentered { width, height } => {
                main_window
                    .set_size(tauri::Size::Logical(tauri::LogicalSize { width, height }))
                    .map_err(|e| {
                        PomoliceError::WindowControl(format!("Failed to restore size: {e}"))
                    })?;

                main_window
                    .center()
                    .map_err(|e| PomoliceError::WindowControl(format!("Failed to center: {e}")))?;
            }
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

    fn physical_position(x: i32, y: i32) -> PhysicalPosition<i32> {
        PhysicalPosition { x, y }
    }

    fn physical_size(width: u32, height: u32) -> PhysicalSize<u32> {
        PhysicalSize { width, height }
    }

    fn restorable_window_state() -> WindowState {
        WindowState::from_restorable_main_window_geometry(
            physical_position(120, 80),
            physical_size(1024, 768),
        )
        .expect("window geometry should be restorable")
    }

    fn monitor_snapshot(
        name: Option<&str>,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> MonitorSnapshot {
        MonitorSnapshot {
            name: name.map(str::to_string),
            position: physical_position(x, y),
            size: physical_size(width, height),
        }
    }

    #[test]
    fn test_default_window_state_manager_selects_default_centered_restore_plan() {
        let manager = WindowStateManager::default();

        assert_eq!(
            manager.main_window_restore_plan(),
            MainWindowRestorePlan::DefaultCentered {
                width: 800.0,
                height: 600.0,
            }
        );
    }

    #[test]
    fn test_restorable_main_window_geometry_is_saved() {
        let position = physical_position(40, 60);
        let size = physical_size(1280, 720);

        let saved_state = WindowState::from_restorable_main_window_geometry(position, size);

        assert!(saved_state.is_some());
    }

    #[test]
    fn test_unrestorable_offscreen_main_window_geometry_is_not_saved() {
        let saved_state = WindowState::from_restorable_main_window_geometry(
            physical_position(-32000, 60),
            physical_size(1280, 720),
        );

        assert_eq!(saved_state, None);
    }

    #[test]
    fn test_zero_width_main_window_geometry_is_not_saved() {
        let saved_state = WindowState::from_restorable_main_window_geometry(
            physical_position(40, 60),
            physical_size(0, 720),
        );

        assert_eq!(saved_state, None);
    }

    #[test]
    fn test_saved_main_window_state_selects_saved_restore_plan() {
        let saved_state = restorable_window_state();
        let mut manager = WindowStateManager::new();

        manager.set_saved_state(Some(saved_state.clone()));

        assert_eq!(
            manager.main_window_restore_plan(),
            MainWindowRestorePlan::Saved(saved_state)
        );
    }

    #[test]
    fn test_missing_main_window_state_selects_default_centered_restore_plan() {
        let manager = WindowStateManager::new();

        assert_eq!(
            manager.main_window_restore_plan(),
            MainWindowRestorePlan::DefaultCentered {
                width: 800.0,
                height: 600.0,
            }
        );
    }

    #[test]
    fn test_overlay_plan_covers_only_the_other_monitor_at_its_geometry() {
        let main_monitor = monitor_snapshot(Some("built-in"), 0, 0, 1920, 1080);
        let external_monitor = monitor_snapshot(Some("external"), -1280, 120, 1280, 1024);
        let monitors = vec![main_monitor.clone(), external_monitor.clone()];

        let overlay_window_plans = WindowStateManager::build_break_overlay_window_plans(
            Some(&main_monitor),
            &monitors,
            &BTreeSet::new(),
        );

        assert_eq!(overlay_window_plans.len(), 1);
        let overlay = &overlay_window_plans[0];
        assert_eq!(overlay.position, external_monitor.position);
        assert_eq!(overlay.size, external_monitor.size);
        assert!(overlay.label.starts_with(BREAK_OVERLAY_LABEL_PREFIX));
    }

    #[test]
    fn test_overlay_plan_keeps_monitor_when_identity_is_unknown() {
        let main_monitor = monitor_snapshot(None, 0, 0, 1920, 1080);
        let candidate_monitor = monitor_snapshot(None, 1920, 0, 1920, 1080);
        let monitors = vec![candidate_monitor.clone()];

        let overlay_window_plans = WindowStateManager::build_break_overlay_window_plans(
            Some(&main_monitor),
            &monitors,
            &BTreeSet::new(),
        );

        assert_eq!(overlay_window_plans.len(), 1);
        let overlay = &overlay_window_plans[0];
        assert_eq!(overlay.position, candidate_monitor.position);
        assert_eq!(overlay.size, candidate_monitor.size);
    }

    #[test]
    fn test_overlay_plan_does_not_duplicate_existing_overlay_label() {
        let main_monitor = monitor_snapshot(Some("built-in"), 0, 0, 1920, 1080);
        let external_monitor = monitor_snapshot(Some("external"), 1920, 0, 1920, 1080);
        let monitors = vec![main_monitor.clone(), external_monitor];
        let initial_plans = WindowStateManager::build_break_overlay_window_plans(
            Some(&main_monitor),
            &monitors,
            &BTreeSet::new(),
        );
        assert_eq!(initial_plans.len(), 1);
        let existing_overlay_labels = BTreeSet::from([initial_plans[0].label.clone()]);

        let overlay_window_plans = WindowStateManager::build_break_overlay_window_plans(
            Some(&main_monitor),
            &monitors,
            &existing_overlay_labels,
        );

        assert_eq!(overlay_window_plans, Vec::<BreakOverlayWindowPlan>::new());
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
        let show_error = "platform denied visibility change";

        let error = WindowStateManager::restore_main_window_visibility(
            || Err(show_error.to_string()),
            || {
                focus_called = true;
                Ok(())
            },
        )
        .expect_err("show failure should fail the rollback restore");

        match error {
            PomoliceError::WindowControl(message) => {
                assert!(
                    message.contains("show"),
                    "window control error should identify the failed show operation: {message:?}"
                );
                assert!(
                    message.contains(show_error),
                    "window control error should retain the underlying show error: {message:?}"
                );
            }
            other => {
                panic!("expected show failure to return a window control error, got {other:?}")
            }
        }
        assert!(!focus_called);
    }

    // Note: enter_fullscreen_all_monitors and exit_fullscreen_all_monitors
    // require real Tauri AppHandle and cannot be unit tested without integration tests.
    // These functions are tested during manual integration testing.
}
