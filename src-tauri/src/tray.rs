// System tray icon setup, event handling, and dynamic updates.
// The tray icon is output-only and does not participate in lock ordering.

use std::sync::atomic::{AtomicBool, Ordering};

use tauri::image::Image;
use tauri::menu::{MenuBuilder, MenuItemBuilder};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Manager};
use tracing::{info, warn};

use crate::tray_icon_renderer;
use crate::types::{TimerStatus, TimerTickPayload};

/// Tracks whether the "minimized to tray" notification has been shown.
/// Once shown, it is never shown again for the lifetime of the process.
static TRAY_NOTIFICATION_SHOWN: AtomicBool = AtomicBool::new(false);

/// Guards against phantom menu events on Windows.
///
/// On Windows, when WebView2 overlay windows are destroyed (e.g. during
/// exit_fullscreen_all_monitors), the Win32 message pump may dispatch stale
/// WM_COMMAND messages that `muda` misinterprets as menu item clicks.
/// This flag is set to `true` only when the user actually right-clicks
/// the tray icon, and reset to `false` after a menu event is processed.
/// Menu events that arrive without this flag set are ignored as phantom events.
static MENU_OPENED_BY_USER: AtomicBool = AtomicBool::new(false);

/// Build and register the system tray icon with menu and click handlers.
///
/// Must be called after `app.manage(app_state)` during setup.
pub fn setup_tray(app: &AppHandle) -> tauri::Result<()> {
    let exit_item = MenuItemBuilder::with_id("exit", "Exit").build(app)?;
    let menu = MenuBuilder::new(app).item(&exit_item).build()?;

    let icon_rgba = tray_icon_renderer::render_idle_icon();
    let size = tray_icon_renderer::icon_size();
    let icon = Image::new_owned(icon_rgba, size, size);

    TrayIconBuilder::with_id("pomolice-tray")
        .icon(icon)
        .tooltip("Pomolice - Idle")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| {
            if event.id() == "exit" {
                // Only process if the user actually opened the menu via right-click.
                // Phantom WM_COMMAND messages from overlay window destruction are
                // ignored because MENU_OPENED_BY_USER will be false.
                if MENU_OPENED_BY_USER.swap(false, Ordering::SeqCst) {
                    info!("Tray menu: Exit selected");
                    app.exit(0);
                } else {
                    warn!("Ignored phantom tray menu event (not triggered by user)");
                }
            }
        })
        .on_tray_icon_event(|tray, event| match event {
            TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } => {
                toggle_main_window_visibility(tray.app_handle());
            }
            TrayIconEvent::Click {
                button: MouseButton::Right,
                button_state: MouseButtonState::Up,
                ..
            } => {
                // Mark that the user intentionally opened the context menu.
                // The menu itself is shown automatically by the OS since it
                // was attached via TrayIconBuilder::menu().
                MENU_OPENED_BY_USER.store(true, Ordering::SeqCst);
            }
            _ => {}
        })
        .build(app)?;

    info!("System tray icon initialized");
    Ok(())
}

/// Update the tray icon image and tooltip based on the current timer tick.
pub fn update_tray_icon(app: &AppHandle, payload: &TimerTickPayload) {
    let remaining_minutes = (payload.remaining_seconds + 59) / 60; // Round up

    let icon_rgba = if payload.status == TimerStatus::Idle {
        tray_icon_renderer::render_idle_icon()
    } else {
        tray_icon_renderer::render_tray_icon(remaining_minutes as u32, payload.status)
    };

    let size = tray_icon_renderer::icon_size();
    let icon = Image::new_owned(icon_rgba, size, size);

    let tooltip = format_tooltip(payload.status, payload.remaining_seconds);

    if let Some(tray) = app.tray_by_id("pomolice-tray") {
        if let Err(e) = tray.set_icon(Some(icon)) {
            warn!("Failed to update tray icon: {e}");
        }
        if let Err(e) = tray.set_tooltip(Some(&tooltip)) {
            warn!("Failed to update tray tooltip: {e}");
        }
    }
}

/// Toggle the main window between visible and hidden.
fn toggle_main_window_visibility(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        if window.is_visible().unwrap_or(false) {
            let _ = window.hide();
        } else {
            let _ = window.show();
            let _ = window.set_focus();
        }
    }
}

/// Returns `true` if the tray notification has not yet been shown, and marks it as shown.
/// Subsequent calls always return `false`.
pub fn should_show_tray_notification() -> bool {
    // swap returns the previous value; if it was false, this is the first call
    !TRAY_NOTIFICATION_SHOWN.swap(true, Ordering::Relaxed)
}

/// Format the tooltip string for the current timer state.
///
/// Examples:
/// - "Pomolice - Working - 18:32 remaining"
/// - "Pomolice - On Break - 4:05 remaining"
/// - "Pomolice - Idle"
fn format_tooltip(status: TimerStatus, remaining_seconds: u64) -> String {
    match status {
        TimerStatus::Idle => "Pomolice - Idle".to_string(),
        _ => {
            let minutes = remaining_seconds / 60;
            let seconds = remaining_seconds % 60;
            let status_label = match status {
                TimerStatus::Working => "Working",
                TimerStatus::OnBreak => "On Break",
                TimerStatus::Idle => unreachable!(),
            };
            format!("Pomolice - {status_label} - {minutes}:{seconds:02} remaining")
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_tooltip_idle() {
        assert_eq!(format_tooltip(TimerStatus::Idle, 0), "Pomolice - Idle");
    }

    #[test]
    fn test_format_tooltip_working() {
        let tip = format_tooltip(TimerStatus::Working, 18 * 60 + 32);
        assert_eq!(tip, "Pomolice - Working - 18:32 remaining");
    }

    #[test]
    fn test_format_tooltip_on_break() {
        let tip = format_tooltip(TimerStatus::OnBreak, 4 * 60 + 5);
        assert_eq!(tip, "Pomolice - On Break - 4:05 remaining");
    }

    #[test]
    fn test_format_tooltip_zero_seconds() {
        let tip = format_tooltip(TimerStatus::Working, 0);
        assert_eq!(tip, "Pomolice - Working - 0:00 remaining");
    }
}
