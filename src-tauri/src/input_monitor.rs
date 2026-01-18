// Pomolice - Input Monitor Module
// Phase 4: Global keyboard and mouse input monitoring for break enforcement

use rdev::{listen, Event, EventType, Key};
use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, Once, OnceLock};
use std::thread;
use tauri::{AppHandle, Emitter};

#[derive(Default)]
struct ModifierState {
    ctrl: bool,
    alt: bool,
    shift: bool,
}

fn update_modifiers(mods: &mut ModifierState, key: Key, pressed: bool) {
    match key {
        Key::ControlLeft | Key::ControlRight => mods.ctrl = pressed,
        Key::Alt | Key::AltGr => mods.alt = pressed,
        Key::ShiftLeft | Key::ShiftRight => mods.shift = pressed,
        _ => {}
    }
}

fn is_escape_combo(mods: &ModifierState, key: Key) -> bool {
    mods.ctrl && mods.alt && mods.shift && matches!(key, Key::KeyE)
}

/// Input event types detected during break time
/// Note: We do NOT record the actual key pressed or mouse position for privacy
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum InputEventType {
    /// Any keyboard key press detected
    Keyboard,
    /// Any mouse button press detected
    MouseClick,
    /// Any mouse movement detected
    MouseMove,
}

/// Payload sent to frontend when input is detected
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InputDetectedPayload {
    pub event_type: InputEventType,
    pub timestamp: u64,
}

static ENABLED: AtomicBool = AtomicBool::new(false);
static LISTENER_STARTED: Once = Once::new();
static APP_HANDLE: OnceLock<AppHandle> = OnceLock::new();

/// Check if we have accessibility permissions (macOS only)
#[cfg(target_os = "macos")]
fn check_accessibility_permission() -> Result<bool, String> {
    // Note: rdev handles permission prompts automatically on macOS
    // If permissions are denied, listen() will fail gracefully
    // For now, we return Ok(true) and let rdev handle the permissions
    Ok(true)
}

#[cfg(not(target_os = "macos"))]
fn check_accessibility_permission() -> Result<bool, String> {
    // No special permissions needed on Windows/Linux
    Ok(true)
}

/// Start global input monitoring
/// This will listen to ALL keyboard and mouse events system-wide
/// and emit "input-detected" events to the frontend
pub fn start_monitoring(app_handle: AppHandle) -> Result<(), String> {
    // Check accessibility permissions
    if !check_accessibility_permission()? {
        return Err(
            "Accessibility permissions required. Please grant permissions in System Preferences."
                .to_string(),
        );
    }

    // Prevent double-start
    if ENABLED.swap(true, Ordering::SeqCst) {
        return Err("Monitoring already started".to_string());
    }

    // Preserve app handle for the listener thread
    let _ = APP_HANDLE.get_or_init(|| app_handle.clone());

    LISTENER_STARTED.call_once(|| {
        let app_handle = APP_HANDLE
            .get()
            .expect("App handle should be set before starting listener")
            .clone();
        let mods = Arc::new(Mutex::new(ModifierState::default()));

        // Spawn monitoring thread
        thread::spawn(move || {
            let callback = move |event: Event| {
                if !ENABLED.load(Ordering::Relaxed) {
                    return;
                }

                // Escape hatch: detect Ctrl+Alt+Shift+E and exit fullscreen
                if let EventType::KeyPress(key) = event.event_type {
                    if let Ok(mut m) = mods.lock() {
                        update_modifiers(&mut m, key, true);
                        if is_escape_combo(&m, key) {
                            let app = app_handle.clone();
                            // Best-effort exit; ignore errors
                            let _ = tauri::async_runtime::block_on(async {
                                crate::window_control::exit_fullscreen_all_monitors(app.clone())
                                    .await
                            });
                            // Disable monitoring after escape
                            ENABLED.store(false, Ordering::SeqCst);
                            return;
                        }
                    }
                } else if let EventType::KeyRelease(key) = event.event_type {
                    if let Ok(mut m) = mods.lock() {
                        update_modifiers(&mut m, key, false);
                    }
                }

                // Determine event type and emit to frontend
                let event_type = match event.event_type {
                    EventType::KeyPress(_) | EventType::KeyRelease(_) => {
                        Some(InputEventType::Keyboard)
                    }
                    EventType::ButtonPress(_) | EventType::ButtonRelease(_) => {
                        Some(InputEventType::MouseClick)
                    }
                    EventType::MouseMove { .. } => Some(InputEventType::MouseMove),
                    EventType::Wheel { .. } => Some(InputEventType::MouseMove), // Treat wheel as mouse activity
                };

                if let Some(event_type) = event_type {
                    let payload = InputDetectedPayload {
                        event_type,
                        timestamp: std::time::SystemTime::now()
                            .duration_since(std::time::UNIX_EPOCH)
                            .unwrap_or_default()
                            .as_millis() as u64,
                    };

                    // Emit event to frontend
                    // Note: We ignore errors here to avoid blocking the event loop
                    let _ = app_handle.emit("input-detected", payload);
                }
            };

            // Start listening (this blocks until process exit or error)
            if let Err(e) = listen(callback) {
                eprintln!("Error in input monitoring: {:?}", e);
            }
        });
    });

    Ok(())
}

/// Stop global input monitoring
pub fn stop_monitoring() -> Result<(), String> {
    if !ENABLED.swap(false, Ordering::SeqCst) {
        return Err("Monitoring not started".to_string());
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_input_event_type_serialization() {
        let event = InputEventType::Keyboard;
        let serialized = serde_json::to_string(&event).unwrap();
        assert!(serialized.contains("Keyboard"));

        let event = InputEventType::MouseClick;
        let serialized = serde_json::to_string(&event).unwrap();
        assert!(serialized.contains("MouseClick"));

        let event = InputEventType::MouseMove;
        let serialized = serde_json::to_string(&event).unwrap();
        assert!(serialized.contains("MouseMove"));
    }

    #[test]
    fn test_input_detected_payload() {
        let payload = InputDetectedPayload {
            event_type: InputEventType::Keyboard,
            timestamp: 1234567890,
        };

        let serialized = serde_json::to_string(&payload).unwrap();
        assert!(serialized.contains("Keyboard"));
        assert!(serialized.contains("1234567890"));
    }
}
