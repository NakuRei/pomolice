// Pomolice - Input Monitor Module
// Phase 4: Global keyboard and mouse input monitoring for break enforcement

use rdev::{listen, Event, EventType};
use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex};
use std::thread;
use tauri::{AppHandle, Emitter};

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

/// Global state to control monitoring lifecycle
struct MonitorState {
    is_monitoring: bool,
    should_stop: bool,
}

static MONITOR_STATE: Mutex<Option<Arc<Mutex<MonitorState>>>> = Mutex::new(None);

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
#[tauri::command]
pub fn start_monitoring(app_handle: AppHandle) -> Result<(), String> {
    // Check accessibility permissions
    if !check_accessibility_permission()? {
        return Err(
            "Accessibility permissions required. Please grant permissions in System Preferences."
                .to_string(),
        );
    }

    // Check if already monitoring
    let mut state_lock = MONITOR_STATE.lock().map_err(|e| e.to_string())?;
    if let Some(state) = &*state_lock {
        let monitoring = state.lock().map_err(|e| e.to_string())?;
        if monitoring.is_monitoring {
            return Err("Monitoring already started".to_string());
        }
    }

    // Initialize state
    let state = Arc::new(Mutex::new(MonitorState {
        is_monitoring: true,
        should_stop: false,
    }));
    *state_lock = Some(state.clone());
    drop(state_lock);

    // Spawn monitoring thread
    thread::spawn(move || {
        let callback = move |event: Event| {
            // Check if we should stop
            if let Ok(state) = state.lock() {
                if state.should_stop {
                    // Signal rdev to stop listening
                    // Note: rdev doesn't have a clean way to stop, so we just return
                    return;
                }
            }

            // Determine event type and emit to frontend
            let event_type = match event.event_type {
                EventType::KeyPress(_) | EventType::KeyRelease(_) => Some(InputEventType::Keyboard),
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

        // Start listening (this blocks until stopped or error)
        if let Err(e) = listen(callback) {
            eprintln!("Error in input monitoring: {:?}", e);
        }

        // Clean up state when stopped
        if let Ok(state_lock) = MONITOR_STATE.lock() {
            if let Some(state) = &*state_lock {
                if let Ok(mut monitoring) = state.lock() {
                    monitoring.is_monitoring = false;
                }
            }
        }
    });

    Ok(())
}

/// Stop global input monitoring
#[tauri::command]
pub fn stop_monitoring() -> Result<(), String> {
    let mut state_lock = MONITOR_STATE.lock().map_err(|e| e.to_string())?;

    if let Some(state) = &*state_lock {
        let mut monitoring = state.lock().map_err(|e| e.to_string())?;
        if !monitoring.is_monitoring {
            return Err("Monitoring not started".to_string());
        }
        monitoring.should_stop = true;
    } else {
        return Err("Monitoring not started".to_string());
    }

    // Clean up state
    *state_lock = None;

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
