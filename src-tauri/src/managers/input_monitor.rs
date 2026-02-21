// Input Monitor Manager
// Manages global input monitoring without global statics

use rdev::{listen, Event, EventType, Key};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Once};
use std::thread;
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
///
/// Callbacks are injected at construction time to decouple this manager
/// from Tauri's `AppHandle`. The caller (e.g. `AppState::new`) is responsible
/// for capturing any `AppHandle` clones inside the closures.
pub struct InputMonitorManager {
    enabled: Arc<AtomicBool>,
    listener_started: Arc<Once>,
    modifier_state: Arc<parking_lot::Mutex<ModifierState>>,
    /// Called when the escape combo (Ctrl+Alt+Shift+E) is detected.
    on_escape_combo: Arc<dyn Fn() + Send + Sync>,
    /// Called when any input event (key, mouse, wheel) is detected during break.
    on_input_detected: Arc<dyn Fn() + Send + Sync>,
}

impl InputMonitorManager {
    /// Create a new input monitor manager
    ///
    /// - `on_escape_combo`: invoked when Ctrl+Alt+Shift+E is detected
    /// - `on_input_detected`: invoked on any input event while monitoring is active
    pub fn new(
        on_escape_combo: impl Fn() + Send + Sync + 'static,
        on_input_detected: impl Fn() + Send + Sync + 'static,
    ) -> Self {
        Self {
            enabled: Arc::new(AtomicBool::new(false)),
            listener_started: Arc::new(Once::new()),
            modifier_state: Arc::new(parking_lot::Mutex::new(ModifierState::default())),
            on_escape_combo: Arc::new(on_escape_combo),
            on_input_detected: Arc::new(on_input_detected),
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

        let enabled = Arc::clone(&self.enabled);
        let listener_started = Arc::clone(&self.listener_started);
        let modifier_state = Arc::clone(&self.modifier_state);
        let on_escape_combo = Arc::clone(&self.on_escape_combo);
        let on_input_detected = Arc::clone(&self.on_input_detected);

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
                                (on_escape_combo)();
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
                        (on_input_detected)();
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
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_manager() -> InputMonitorManager {
        InputMonitorManager::new(|| {}, || {})
    }

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

    #[test]
    fn test_stop_is_idempotent() {
        let mut mgr = make_manager();
        // stop on an already-stopped manager must not error
        assert!(mgr.stop().is_ok());
        assert!(mgr.stop().is_ok());
    }

    #[test]
    fn test_start_is_idempotent() {
        let mut mgr = make_manager();
        assert!(mgr.start().is_ok());
        // Second start must not error
        assert!(mgr.start().is_ok());
        // Cleanup
        let _ = mgr.stop();
    }

    #[test]
    fn test_stop_clears_enabled_flag() {
        let mut mgr = make_manager();
        mgr.start().unwrap();
        assert!(mgr.enabled.load(Ordering::SeqCst));

        mgr.stop().unwrap();
        assert!(!mgr.enabled.load(Ordering::SeqCst));
    }

    // Note: start() spawns a real OS thread via rdev::listen which blocks until
    // process exit. Callback invocation is therefore not tested at unit level;
    // it is covered by manual integration testing.
}
