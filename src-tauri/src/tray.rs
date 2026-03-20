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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TrayGlyphMode {
    IdleSymbol,
    RemainingMinutes,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct TrayStatusPresentation {
    glyph_mode: TrayGlyphMode,
    status_label: &'static str,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct TrayVisualSpec {
    icon_rgba: Vec<u8>,
    tooltip: String,
}

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

    let initial_spec = compose_tray_visual_spec(TimerStatus::Idle, 0);
    let size = tray_icon_renderer::icon_size();
    let icon = Image::new_owned(initial_spec.icon_rgba, size, size);

    TrayIconBuilder::with_id("pomolice-tray")
        .icon(icon)
        .tooltip(initial_spec.tooltip)
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
    let visual_spec = compose_tray_visual_spec(payload.status, payload.remaining_seconds);
    let size = tray_icon_renderer::icon_size();
    let icon = Image::new_owned(visual_spec.icon_rgba, size, size);

    if let Some(tray) = app.tray_by_id("pomolice-tray") {
        if let Err(e) = tray.set_icon(Some(icon)) {
            warn!("Failed to update tray icon: {e}");
        }
        if let Err(e) = tray.set_tooltip(Some(&visual_spec.tooltip)) {
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

/// Map timer status to a centralized tray presentation definition.
///
/// This single mapping keeps icon semantics and tooltip status wording aligned.
fn status_presentation(status: TimerStatus) -> TrayStatusPresentation {
    match status {
        TimerStatus::Idle => TrayStatusPresentation {
            glyph_mode: TrayGlyphMode::IdleSymbol,
            status_label: "Idle",
        },
        TimerStatus::Working => TrayStatusPresentation {
            glyph_mode: TrayGlyphMode::RemainingMinutes,
            status_label: "Working",
        },
        TimerStatus::OnBreak => TrayStatusPresentation {
            glyph_mode: TrayGlyphMode::RemainingMinutes,
            status_label: "On Break",
        },
    }
}

/// Convert timer status and time data to icon pixels and tooltip text.
fn compose_tray_visual_spec(status: TimerStatus, remaining_seconds: u64) -> TrayVisualSpec {
    let presentation = status_presentation(status);
    let icon_rgba = render_icon_rgba(presentation.glyph_mode, status, remaining_seconds);
    let tooltip = compose_tooltip_text(presentation, remaining_seconds);
    TrayVisualSpec { icon_rgba, tooltip }
}

fn render_icon_rgba(
    glyph_mode: TrayGlyphMode,
    status: TimerStatus,
    remaining_seconds: u64,
) -> Vec<u8> {
    match glyph_mode {
        TrayGlyphMode::IdleSymbol => tray_icon_renderer::render_idle_icon(),
        TrayGlyphMode::RemainingMinutes => {
            let remaining_minutes = remaining_seconds.div_ceil(60); // Round up
            tray_icon_renderer::render_tray_icon(remaining_minutes as u32, status)
        }
    }
}

/// Format the tooltip string for the current timer state.
///
/// Examples:
/// - "Pomolice - Working - 18:32 remaining"
/// - "Pomolice - On Break - 4:05 remaining"
/// - "Pomolice - Idle"
fn compose_tooltip_text(presentation: TrayStatusPresentation, remaining_seconds: u64) -> String {
    match presentation.glyph_mode {
        TrayGlyphMode::IdleSymbol => "Pomolice - Idle".to_string(),
        TrayGlyphMode::RemainingMinutes => {
            let minutes = remaining_seconds / 60;
            let seconds = remaining_seconds % 60;
            format!(
                "Pomolice - {} - {minutes}:{seconds:02} remaining",
                presentation.status_label
            )
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_status_presentation_idle() {
        let presentation = status_presentation(TimerStatus::Idle);
        assert_eq!(presentation.glyph_mode, TrayGlyphMode::IdleSymbol);
        assert_eq!(presentation.status_label, "Idle");
    }

    #[test]
    fn test_status_presentation_working() {
        let presentation = status_presentation(TimerStatus::Working);
        assert_eq!(presentation.glyph_mode, TrayGlyphMode::RemainingMinutes);
        assert_eq!(presentation.status_label, "Working");
    }

    #[test]
    fn test_status_presentation_on_break() {
        let presentation = status_presentation(TimerStatus::OnBreak);
        assert_eq!(presentation.glyph_mode, TrayGlyphMode::RemainingMinutes);
        assert_eq!(presentation.status_label, "On Break");
    }

    #[test]
    fn test_compose_tooltip_text_idle() {
        let idle = status_presentation(TimerStatus::Idle);
        assert_eq!(compose_tooltip_text(idle, 0), "Pomolice - Idle");
    }

    #[test]
    fn test_compose_tooltip_text_working() {
        let working = status_presentation(TimerStatus::Working);
        let tip = compose_tooltip_text(working, 18 * 60 + 32);
        assert_eq!(tip, "Pomolice - Working - 18:32 remaining");
    }

    #[test]
    fn test_compose_tooltip_text_on_break() {
        let on_break = status_presentation(TimerStatus::OnBreak);
        let tip = compose_tooltip_text(on_break, 4 * 60 + 5);
        assert_eq!(tip, "Pomolice - On Break - 4:05 remaining");
    }

    #[test]
    fn test_compose_tooltip_text_zero_seconds() {
        let working = status_presentation(TimerStatus::Working);
        let tip = compose_tooltip_text(working, 0);
        assert_eq!(tip, "Pomolice - Working - 0:00 remaining");
    }

    #[test]
    fn test_compose_tray_visual_spec_idle_uses_idle_icon() {
        let spec = compose_tray_visual_spec(TimerStatus::Idle, 10);
        let expected = tray_icon_renderer::render_idle_icon();
        assert_eq!(spec.icon_rgba, expected);
        assert_eq!(spec.tooltip, "Pomolice - Idle");
    }

    #[test]
    fn test_compose_tray_visual_spec_working_uses_minutes_icon() {
        let spec = compose_tray_visual_spec(TimerStatus::Working, 65);
        let expected = tray_icon_renderer::render_tray_icon(2, TimerStatus::Working);
        assert_eq!(spec.icon_rgba, expected);
        assert_eq!(spec.tooltip, "Pomolice - Working - 1:05 remaining");
    }

    #[test]
    fn test_compose_tray_visual_spec_tooltip_matches_status_presentation() {
        let idle_spec = compose_tray_visual_spec(TimerStatus::Idle, 0);
        let idle_presentation = status_presentation(TimerStatus::Idle);
        assert_eq!(idle_presentation.glyph_mode, TrayGlyphMode::IdleSymbol);
        assert_eq!(idle_spec.tooltip, "Pomolice - Idle");

        let working_spec = compose_tray_visual_spec(TimerStatus::Working, 125);
        let working_presentation = status_presentation(TimerStatus::Working);
        assert_eq!(
            working_presentation.glyph_mode,
            TrayGlyphMode::RemainingMinutes
        );
        assert!(working_spec
            .tooltip
            .contains(working_presentation.status_label));

        let on_break_spec = compose_tray_visual_spec(TimerStatus::OnBreak, 125);
        let on_break_presentation = status_presentation(TimerStatus::OnBreak);
        assert_eq!(
            on_break_presentation.glyph_mode,
            TrayGlyphMode::RemainingMinutes
        );
        assert!(on_break_spec
            .tooltip
            .contains(on_break_presentation.status_label));
    }
}
