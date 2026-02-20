// Input Monitor Manager
// Manages global input monitoring without global statics

use rdev::{listen, Event, EventType, Key};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Once};
use std::thread;
use tauri::AppHandle;
use tracing::{error, info};

use crate::error::Result;

#[derive(Default, Clone)]
struct ModifierState {
    ctrl: bool,
    alt: bool,
    shift: bool,
}

impl ModifierState {
    fn update(&mut self, key: Key, pressed: bool) {
        match key {
            Key::ControlLeft | Key::ControlRight => self.ctrl = pressed,
            Key::Alt | Key::AltGr => self.alt = pressed,
            Key::ShiftLeft | Key::ShiftRight => self.shift = pressed,
            _ => {}
        }
    }

    fn is_escape_combo(&self, key: Key) -> bool {
        self.ctrl && self.alt && self.shift && matches!(key, Key::KeyE)
    }
}

/// Manager for global input monitoring
pub struct InputMonitorManager {
    app_handle: AppHandle,
    enabled: Arc<AtomicBool>,
    listener_started: Arc<Once>,
    modifier_state: Arc<parking_lot::Mutex<ModifierState>>,
}

impl InputMonitorManager {
    /// Create a new input monitor manager
    pub fn new(app_handle: AppHandle) -> Self {
        Self {
            app_handle,
            enabled: Arc::new(AtomicBool::new(false)),
            listener_started: Arc::new(Once::new()),
            modifier_state: Arc::new(parking_lot::Mutex::new(ModifierState::default())),
        }
    }

    /// Start global input monitoring
    pub fn start(&mut self) -> Result<()> {
        info!("Starting global input monitoring");

        // Check accessibility permissions (macOS only)
        #[cfg(target_os = "macos")]
        {
            // rdev handles permission prompts automatically on macOS
            // If permissions are denied, listen() will fail gracefully
        }

        // Prevent double-start
        if self.enabled.swap(true, Ordering::SeqCst) {
            info!("Input monitoring already running");
            return Ok(()); // Already started, not an error
        }

        let app_handle = self.app_handle.clone();
        let enabled = Arc::clone(&self.enabled);
        let listener_started = Arc::clone(&self.listener_started);
        let modifier_state = Arc::clone(&self.modifier_state);

        listener_started.call_once(move || {
            // Spawn monitoring thread
            thread::spawn(move || {
                let callback = move |event: Event| {
                    if !enabled.load(Ordering::Relaxed) {
                        return;
                    }

                    // Track modifier keys for escape combo detection
                    match event.event_type {
                        EventType::KeyPress(key) => {
                            let mut mods = modifier_state.lock();
                            mods.update(key, true);

                            // Escape hatch: detect Ctrl+Alt+Shift+E
                            if mods.is_escape_combo(key) {
                                enabled.store(false, Ordering::SeqCst);
                                let app = app_handle.clone();
                                tauri::async_runtime::spawn(async move {
                                    crate::lib_handle_escape_combo(app).await;
                                });
                                return;
                            }
                        }
                        EventType::KeyRelease(key) => {
                            let mut mods = modifier_state.lock();
                            mods.update(key, false);
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
                        // Handle input detection
                        let app = app_handle.clone();
                        tauri::async_runtime::spawn(async move {
                            crate::lib_handle_input_detected(app).await;
                        });
                    }
                };

                // Start listening (this blocks until process exit or error)
                info!("Starting input event listener");
                if let Err(e) = listen(callback) {
                    error!("Error in input monitoring: {:?}", e);
                }
            });
        });

        Ok(())
    }

    /// Stop global input monitoring
    pub fn stop(&mut self) -> Result<()> {
        if !self.enabled.swap(false, Ordering::SeqCst) {
            info!("Input monitoring already stopped");
            return Ok(()); // Already stopped, not an error
        }

        info!("Stopping global input monitoring");

        // Reset modifier state
        let mut mods = self.modifier_state.lock();
        *mods = ModifierState::default();

        Ok(())
    }

    /// Check if monitoring is currently enabled
    pub fn is_enabled(&self) -> bool {
        self.enabled.load(Ordering::SeqCst)
    }
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
        assert!(!mods.is_escape_combo(Key::KeyE));

        // Add modifiers one by one
        mods.update(Key::ControlLeft, true);
        assert!(!mods.is_escape_combo(Key::KeyE));

        mods.update(Key::Alt, true);
        assert!(!mods.is_escape_combo(Key::KeyE));

        mods.update(Key::ShiftLeft, true);
        // Now all modifiers are pressed
        assert!(mods.is_escape_combo(Key::KeyE));

        // Wrong key
        assert!(!mods.is_escape_combo(Key::KeyA));
    }

    #[test]
    fn test_modifier_state_update_press_and_release() {
        let mut mods = ModifierState::default();

        // Press left control
        mods.update(Key::ControlLeft, true);
        assert!(mods.ctrl);

        // Release left control
        mods.update(Key::ControlLeft, false);
        assert!(!mods.ctrl);

        // Press right control
        mods.update(Key::ControlRight, true);
        assert!(mods.ctrl);

        // Release right control
        mods.update(Key::ControlRight, false);
        assert!(!mods.ctrl);
    }

    #[test]
    fn test_modifier_state_all_modifier_keys() {
        let mut mods = ModifierState::default();

        // Test Alt
        mods.update(Key::Alt, true);
        assert!(mods.alt);
        mods.update(Key::Alt, false);
        assert!(!mods.alt);

        mods.update(Key::AltGr, true);
        assert!(mods.alt);
        mods.update(Key::AltGr, false);
        assert!(!mods.alt);

        // Test Shift
        mods.update(Key::ShiftLeft, true);
        assert!(mods.shift);
        mods.update(Key::ShiftLeft, false);
        assert!(!mods.shift);

        mods.update(Key::ShiftRight, true);
        assert!(mods.shift);
        mods.update(Key::ShiftRight, false);
        assert!(!mods.shift);

        // Test Ctrl
        mods.update(Key::ControlLeft, true);
        assert!(mods.ctrl);
        mods.update(Key::ControlLeft, false);
        assert!(!mods.ctrl);

        mods.update(Key::ControlRight, true);
        assert!(mods.ctrl);
        mods.update(Key::ControlRight, false);
        assert!(!mods.ctrl);
    }

    #[test]
    fn test_escape_combo_with_right_modifiers() {
        let mut mods = ModifierState::default();

        // Use right-side modifiers
        mods.update(Key::ControlRight, true);
        mods.update(Key::AltGr, true);
        mods.update(Key::ShiftRight, true);

        // Should detect combo with right modifiers
        assert!(mods.is_escape_combo(Key::KeyE));

        // Wrong key should still fail
        assert!(!mods.is_escape_combo(Key::KeyA));
    }

    #[test]
    fn test_escape_combo_mixed_modifiers() {
        let mut mods = ModifierState::default();

        // Mix left and right modifiers
        mods.update(Key::ControlLeft, true);
        mods.update(Key::AltGr, true);
        mods.update(Key::ShiftRight, true);

        // Should still work
        assert!(mods.is_escape_combo(Key::KeyE));
    }

    // Note: start() and stop() methods require real AppHandle and cannot be unit tested
    // without integration tests. These functions are tested during manual integration testing.
}
