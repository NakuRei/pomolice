// System tray icon setup, event handling, and dynamic updates.
// The tray icon is output-only and does not participate in lock ordering.

use std::io::Cursor;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    OnceLock,
};

use tauri::image::Image;
use tauri::menu::{MenuBuilder, MenuItemBuilder};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Manager};

use crate::logging_event::{log_info_event, log_warn_event, BackendLogEvent};
use crate::tray_icon_renderer;
use crate::types::{TimerStatus, TimerTickPayload};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TrayGlyphMode {
    IdleSymbol,
    RemainingMinutes,
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum TrayIconContent {
    IdleAppIcon,
    RemainingMinutes { minutes: u32, status: TimerStatus },
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum TrayIconSource {
    EmbeddedIdleAppIcon,
    GeneratedIdleFallback,
    GeneratedRemainingMinutes { minutes: u32, status: TimerStatus },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct TrayStatusPresentation {
    glyph_mode: TrayGlyphMode,
    status_label: &'static str,
}

#[derive(Debug, Clone, PartialEq)]
struct TrayVisualSpec {
    icon_content: TrayIconContent,
    tooltip: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct TrayIconPixels {
    rgba: Vec<u8>,
    width: u32,
    height: u32,
}

/// Tracks whether the "minimized to tray" notification has been shown.
/// Once shown, it is never shown again for the lifetime of the process.
static TRAY_NOTIFICATION_SHOWN: AtomicBool = AtomicBool::new(false);

static IDLE_APP_ICON: OnceLock<TrayIconPixels> = OnceLock::new();

/// Guards against phantom menu events on Windows.
///
/// On `Windows`, when `WebView2` overlay windows are destroyed (e.g. during
/// `exit_fullscreen_all_monitors`), the Win32 message pump may dispatch stale
/// `WM_COMMAND` messages that `muda` misinterprets as menu item clicks.
/// This flag is set to `true` only when the user actually right-clicks
/// the tray icon, and reset to `false` after a menu event is processed.
/// Menu events that arrive without this flag set are ignored as phantom events.
static MENU_OPENED_BY_USER: AtomicBool = AtomicBool::new(false);

/// Build and register the system tray icon with menu and click handlers.
///
/// Must be called after `app.manage(app_state)` during setup.
pub(crate) fn setup_tray(
    app: &AppHandle,
) -> std::result::Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let decoded_icon = decode_idle_app_icon()?;
    let _ = IDLE_APP_ICON.set(decoded_icon);

    let exit_item = MenuItemBuilder::with_id("exit", "Exit").build(app)?;
    let menu = MenuBuilder::new(app).item(&exit_item).build()?;

    let initial_spec = compose_tray_visual_spec(TimerStatus::Idle, 0);
    let initial_icon = render_icon_content(initial_spec.icon_content);
    let icon = Image::new_owned(initial_icon.rgba, initial_icon.width, initial_icon.height);

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
                    log_info_event(BackendLogEvent::AppTrayMenuExitSelected, &[]);
                    app.exit(0);
                } else {
                    log_warn_event(BackendLogEvent::AppTrayPhantomMenuEventIgnored, &[]);
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

    log_info_event(BackendLogEvent::AppTrayInitialized, &[]);
    Ok(())
}

/// Update the tray icon image and tooltip based on the current timer tick.
pub(crate) fn update_tray_icon(app: &AppHandle, payload: &TimerTickPayload) {
    let visual_spec = compose_tray_visual_spec(payload.status, payload.remaining_seconds);
    let icon_pixels = render_icon_content(visual_spec.icon_content);
    let icon = Image::new_owned(icon_pixels.rgba, icon_pixels.width, icon_pixels.height);

    if let Some(tray) = app.tray_by_id("pomolice-tray") {
        if let Err(e) = tray.set_icon(Some(icon)) {
            log_warn_event(
                BackendLogEvent::AppTrayIconUpdateFailed,
                &[("error_message", e.to_string())],
            );
        }
        if let Err(e) = tray.set_tooltip(Some(&visual_spec.tooltip)) {
            log_warn_event(
                BackendLogEvent::AppTrayTooltipUpdateFailed,
                &[("error_message", e.to_string())],
            );
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
pub(crate) fn should_show_tray_notification() -> bool {
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

/// Convert timer status and time data to tray icon content and tooltip text.
fn compose_tray_visual_spec(status: TimerStatus, remaining_seconds: u64) -> TrayVisualSpec {
    let presentation = status_presentation(status);
    let icon_content = tray_icon_content(status, remaining_seconds);
    let tooltip = compose_tooltip_text(presentation, remaining_seconds);
    TrayVisualSpec {
        icon_content,
        tooltip,
    }
}

fn tray_icon_content(status: TimerStatus, remaining_seconds: u64) -> TrayIconContent {
    match status_presentation(status).glyph_mode {
        TrayGlyphMode::IdleSymbol => TrayIconContent::IdleAppIcon,
        TrayGlyphMode::RemainingMinutes => TrayIconContent::RemainingMinutes {
            minutes: tray_icon_minutes(remaining_seconds),
            status,
        },
    }
}

fn render_icon_content(icon_content: TrayIconContent) -> TrayIconPixels {
    let embedded_idle_icon = idle_app_icon();
    let icon_source = select_tray_icon_source(icon_content, embedded_idle_icon.is_some());
    render_tray_icon_source(icon_source, embedded_idle_icon)
}

fn select_tray_icon_source(
    icon_content: TrayIconContent,
    embedded_idle_icon_available: bool,
) -> TrayIconSource {
    match icon_content {
        TrayIconContent::IdleAppIcon if embedded_idle_icon_available => {
            TrayIconSource::EmbeddedIdleAppIcon
        }
        TrayIconContent::IdleAppIcon => TrayIconSource::GeneratedIdleFallback,
        TrayIconContent::RemainingMinutes { minutes, status } => {
            TrayIconSource::GeneratedRemainingMinutes { minutes, status }
        }
    }
}

fn render_tray_icon_source(
    icon_source: TrayIconSource,
    embedded_idle_icon: Option<&TrayIconPixels>,
) -> TrayIconPixels {
    match icon_source {
        TrayIconSource::EmbeddedIdleAppIcon => {
            if let Some(icon) = embedded_idle_icon {
                TrayIconPixels {
                    rgba: icon.rgba.clone(),
                    width: icon.width,
                    height: icon.height,
                }
            } else {
                render_generated_idle_fallback_icon()
            }
        }
        TrayIconSource::GeneratedIdleFallback => render_generated_idle_fallback_icon(),
        TrayIconSource::GeneratedRemainingMinutes { minutes, status } => {
            let size = tray_icon_renderer::icon_size();
            TrayIconPixels {
                rgba: tray_icon_renderer::render_tray_icon(minutes, status),
                width: size,
                height: size,
            }
        }
    }
}

fn render_generated_idle_fallback_icon() -> TrayIconPixels {
    log_warn_event(BackendLogEvent::AppTrayIdleAppIconMissing, &[]);
    let size = tray_icon_renderer::icon_size();
    TrayIconPixels {
        rgba: tray_icon_renderer::render_idle_icon(),
        width: size,
        height: size,
    }
}

fn tray_icon_minutes(remaining_seconds: u64) -> u32 {
    let remaining_minutes = remaining_seconds.div_ceil(60); // Round up
    u32::try_from(remaining_minutes).unwrap_or(u32::MAX)
}

fn idle_app_icon() -> Option<&'static TrayIconPixels> {
    IDLE_APP_ICON.get()
}

fn decode_idle_app_icon(
) -> std::result::Result<TrayIconPixels, Box<dyn std::error::Error + Send + Sync>> {
    let decoder = png::Decoder::new(Cursor::new(&include_bytes!("../icons/32x32.png")[..]));
    let mut reader = decoder.read_info()?;

    if reader.output_color_type().0 != png::ColorType::Rgba {
        return Err("embedded idle app icon PNG must decode to RGBA".into());
    }

    let mut rgba = Vec::with_capacity(reader.output_buffer_size());
    while let Ok(Some(row)) = reader.next_row() {
        rgba.extend(row.data());
    }

    Ok(TrayIconPixels {
        rgba,
        width: reader.info().width,
        height: reader.info().height,
    })
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
    fn test_idle_icon_content_selects_embedded_app_icon_when_available() {
        assert_eq!(
            select_tray_icon_source(TrayIconContent::IdleAppIcon, true),
            TrayIconSource::EmbeddedIdleAppIcon
        );
    }

    #[test]
    fn test_idle_icon_content_selects_generated_fallback_when_app_icon_missing() {
        assert_eq!(
            select_tray_icon_source(TrayIconContent::IdleAppIcon, false),
            TrayIconSource::GeneratedIdleFallback
        );
    }

    #[test]
    fn test_remaining_minutes_content_selects_generated_minutes_icon() {
        assert_eq!(
            select_tray_icon_source(
                TrayIconContent::RemainingMinutes {
                    minutes: 5,
                    status: TimerStatus::Working
                },
                true
            ),
            TrayIconSource::GeneratedRemainingMinutes {
                minutes: 5,
                status: TimerStatus::Working
            }
        );
    }

    #[test]
    fn test_decode_idle_app_icon_returns_valid_rgba_pixels() {
        let icon = decode_idle_app_icon().expect("embedded idle app icon must decode");

        assert!(icon.width > 0);
        assert!(icon.height > 0);
        assert_eq!(icon.rgba.len(), (icon.width * icon.height * 4) as usize);
    }

    #[test]
    fn test_compose_tray_visual_spec_idle_uses_app_icon_content() {
        let spec = compose_tray_visual_spec(TimerStatus::Idle, 65);

        assert_eq!(spec.icon_content, TrayIconContent::IdleAppIcon);
        assert_eq!(spec.tooltip, "Pomolice - Idle");
    }

    #[test]
    fn test_compose_tray_visual_spec_working_shows_rounded_up_remaining_minutes() {
        let spec = compose_tray_visual_spec(TimerStatus::Working, 65);

        assert_eq!(
            spec.icon_content,
            TrayIconContent::RemainingMinutes {
                minutes: 2,
                status: TimerStatus::Working
            }
        );
        assert_eq!(spec.tooltip, "Pomolice - Working - 1:05 remaining");
    }

    #[test]
    fn test_compose_tray_visual_spec_on_break_shows_rounded_up_remaining_minutes() {
        let spec = compose_tray_visual_spec(TimerStatus::OnBreak, 65);

        assert_eq!(
            spec.icon_content,
            TrayIconContent::RemainingMinutes {
                minutes: 2,
                status: TimerStatus::OnBreak
            }
        );
        assert_eq!(spec.tooltip, "Pomolice - On Break - 1:05 remaining");
    }

    #[test]
    fn test_compose_tray_visual_spec_zero_seconds_keeps_zero_icon_minutes() {
        let spec = compose_tray_visual_spec(TimerStatus::Working, 0);

        assert_eq!(
            spec.icon_content,
            TrayIconContent::RemainingMinutes {
                minutes: 0,
                status: TimerStatus::Working
            }
        );
        assert_eq!(spec.tooltip, "Pomolice - Working - 0:00 remaining");
    }

    #[test]
    fn test_compose_tray_visual_spec_exact_minute_keeps_same_icon_minutes() {
        let spec = compose_tray_visual_spec(TimerStatus::Working, 60);

        assert_eq!(
            spec.icon_content,
            TrayIconContent::RemainingMinutes {
                minutes: 1,
                status: TimerStatus::Working
            }
        );
    }

    #[test]
    fn test_compose_tray_visual_spec_partial_minute_rounds_icon_minutes_up() {
        let spec = compose_tray_visual_spec(TimerStatus::Working, 61);

        assert_eq!(
            spec.icon_content,
            TrayIconContent::RemainingMinutes {
                minutes: 2,
                status: TimerStatus::Working
            }
        );
    }

    #[test]
    fn test_compose_tray_visual_spec_large_remaining_seconds_stays_active() {
        let spec = compose_tray_visual_spec(TimerStatus::Working, u64::MAX);

        assert!(matches!(
            spec.icon_content,
            TrayIconContent::RemainingMinutes {
                minutes: _,
                status: TimerStatus::Working
            }
        ));
    }

    #[test]
    fn test_compose_tray_visual_spec_tooltips_match_timer_status() {
        let idle_spec = compose_tray_visual_spec(TimerStatus::Idle, 0);
        assert_eq!(idle_spec.tooltip, "Pomolice - Idle");

        let working_spec = compose_tray_visual_spec(TimerStatus::Working, 125);
        assert_eq!(working_spec.tooltip, "Pomolice - Working - 2:05 remaining");

        let on_break_spec = compose_tray_visual_spec(TimerStatus::OnBreak, 125);
        assert_eq!(
            on_break_spec.tooltip,
            "Pomolice - On Break - 2:05 remaining"
        );
    }
}
