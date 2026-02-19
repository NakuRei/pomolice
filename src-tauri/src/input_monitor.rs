// Pomolice - Input Monitor Module
// Phase 4: Global keyboard and mouse input monitoring for break enforcement
// Now applies penalties directly instead of emitting events to frontend

use rdev::{listen, Event, EventType, Key};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, Once, OnceLock};
use std::thread;
use tauri::AppHandle;

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

static ENABLED: AtomicBool = AtomicBool::new(false);
static LISTENER_STARTED: Once = Once::new();
static APP_HANDLE: OnceLock<AppHandle> = OnceLock::new();
static MODIFIER_STATE: Mutex<ModifierState> = Mutex::new(ModifierState {
    ctrl: false,
    alt: false,
    shift: false,
});

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
/// and apply penalties directly to the timer state
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

        // Spawn monitoring thread
        thread::spawn(move || {
            let callback = move |event: Event| {
                if !ENABLED.load(Ordering::Relaxed) {
                    return;
                }

                // Track modifier keys for escape combo detection
                match event.event_type {
                    EventType::KeyPress(key) => {
                        if let Ok(mut mods) = MODIFIER_STATE.lock() {
                            update_modifiers(&mut mods, key, true);

                            // Escape hatch: detect Ctrl+Alt+Shift+E and exit fullscreen
                            if is_escape_combo(&mods, key) {
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
                    }
                    EventType::KeyRelease(key) => {
                        if let Ok(mut mods) = MODIFIER_STATE.lock() {
                            update_modifiers(&mut mods, key, false);
                        }
                    }
                    _ => {}
                }

                // Check if this is an input event that should trigger a penalty
                let should_penalize = matches!(
                    event.event_type,
                    EventType::KeyPress(_)
                        | EventType::KeyRelease(_)
                        | EventType::ButtonPress(_)
                        | EventType::ButtonRelease(_)
                        | EventType::MouseMove { .. }
                        | EventType::Wheel { .. }
                );

                if should_penalize {
                    // Apply penalty directly to timer state
                    // The timer_state module handles debouncing internally
                    crate::timer_state::apply_input_penalty(&app_handle);
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

    // Reset modifier state
    if let Ok(mut mods) = MODIFIER_STATE.lock() {
        *mods = ModifierState::default();
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_modifier_state_default() {
        let mods = ModifierState::default();
        assert!(!mods.ctrl);
        assert!(!mods.alt);
        assert!(!mods.shift);
    }

    #[test]
    fn test_escape_combo_detection() {
        let mut mods = ModifierState::default();

        // Not enough modifiers
        assert!(!is_escape_combo(&mods, Key::KeyE));

        // Add modifiers one by one
        update_modifiers(&mut mods, Key::ControlLeft, true);
        assert!(!is_escape_combo(&mods, Key::KeyE));

        update_modifiers(&mut mods, Key::Alt, true);
        assert!(!is_escape_combo(&mods, Key::KeyE));

        update_modifiers(&mut mods, Key::ShiftLeft, true);
        // Now all modifiers are pressed
        assert!(is_escape_combo(&mods, Key::KeyE));

        // Wrong key
        assert!(!is_escape_combo(&mods, Key::KeyA));
    }
}
