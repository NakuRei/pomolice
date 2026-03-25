// Input Monitor Manager
// Manages global input monitoring without global statics

use rdev::{listen, Event, EventType, Key};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::Duration;
use tracing::{error, info};

const INPUT_MONITOR_MAX_BACKOFF: Duration = Duration::from_secs(30);

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
pub(crate) struct InputMonitorManager {
    /// Whether callbacks should fire. Set to true by `start()`, false by `stop()`.
    enabled: Arc<AtomicBool>,
    /// True while the supervisor thread is alive. Cleared by the thread itself on exit.
    /// Never written by `start()` or `stop()` after the initial spawn — only the thread
    /// clears it, preventing a race between a dying thread and a new spawn.
    listener_alive: Arc<AtomicBool>,
    /// Set to true by `stop()` to tell the supervisor loop to exit cleanly instead of
    /// retrying. Reset to false by `start()` before each spawn.
    shutdown_requested: Arc<AtomicBool>,
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
    pub(crate) fn new(
        on_escape_combo: impl Fn() + Send + Sync + 'static,
        on_input_detected: impl Fn() + Send + Sync + 'static,
    ) -> Self {
        Self {
            enabled: Arc::new(AtomicBool::new(false)),
            listener_alive: Arc::new(AtomicBool::new(false)),
            shutdown_requested: Arc::new(AtomicBool::new(false)),
            modifier_state: Arc::new(parking_lot::Mutex::new(ModifierState::default())),
            on_escape_combo: Arc::new(on_escape_combo),
            on_input_detected: Arc::new(on_input_detected),
        }
    }

    /// Start global input monitoring.
    ///
    /// If a supervisor thread is already running (`listener_alive` is true), this
    /// call re-enables callbacks without spawning a new thread. Otherwise it resets
    /// the shutdown signal, marks the thread as alive, and spawns a new supervisor.
    pub(crate) fn start(&mut self) {
        info!("Starting global input monitoring");

        // If the supervisor thread is already running, just re-enable callbacks.
        if self.listener_alive.load(Ordering::SeqCst) {
            self.enabled.store(true, Ordering::SeqCst);
            info!("Input monitoring already running; callbacks re-enabled");
            return;
        }

        // Reset signals before spawning so the supervisor loop starts fresh.
        self.shutdown_requested.store(false, Ordering::SeqCst);
        self.enabled.store(true, Ordering::SeqCst);
        self.listener_alive.store(true, Ordering::SeqCst);

        let enabled = Arc::clone(&self.enabled);
        let listener_alive = Arc::clone(&self.listener_alive);
        let shutdown_requested = Arc::clone(&self.shutdown_requested);
        let modifier_state = Arc::clone(&self.modifier_state);
        let on_escape_combo = Arc::clone(&self.on_escape_combo);
        let on_input_detected = Arc::clone(&self.on_input_detected);

        thread::spawn(move || {
            let mut backoff = Duration::from_secs(1);

            loop {
                let callback = build_event_callback(
                    Arc::clone(&enabled),
                    Arc::clone(&modifier_state),
                    Arc::clone(&on_escape_combo),
                    Arc::clone(&on_input_detected),
                );

                info!("Starting input event listener");
                match listen(callback) {
                    Ok(()) => {
                        // rdev returned cleanly. Treat as a stop signal.
                        info!("Input event listener stopped cleanly");
                        break;
                    }
                    Err(e) => {
                        if shutdown_requested.load(Ordering::SeqCst) {
                            info!("Input monitoring shut down on request after listener error");
                            break;
                        }
                        error!(
                            "Input monitoring listener failed: {:?}; retrying in {:?}",
                            e, backoff
                        );
                        thread::sleep(backoff);
                        backoff = (backoff * 2).min(INPUT_MONITOR_MAX_BACKOFF);
                    }
                }
            }

            // Always clear the alive flag so start() can spawn a new thread later.
            listener_alive.store(false, Ordering::SeqCst);
        });
    }

    /// Stop global input monitoring
    pub(crate) fn stop(&mut self) {
        if !self.enabled.swap(false, Ordering::SeqCst) {
            info!("Input monitoring already stopped");
            return; // Already stopped, not an error
        }

        info!("Stopping global input monitoring");

        // Signal the supervisor loop to exit on its next iteration rather than retry.
        self.shutdown_requested.store(true, Ordering::SeqCst);

        // Reset modifier state so no stale key flags remain for the next session.
        let mut mods = self.modifier_state.lock();
        *mods = ModifierState::default();
    }
}

/// Construct one rdev event callback for a single `listen()` invocation.
///
/// All Arc arguments must be freshly cloned from the supervisor's Arcs before
/// each call, because closures cannot be cloned. Each `listen()` invocation
/// therefore gets independent captures with the same underlying shared state.
///
/// # Example
/// ```ignore
/// let callback = build_event_callback(
///     Arc::clone(&enabled),
///     Arc::clone(&modifier_state),
///     Arc::clone(&on_escape_combo),
///     Arc::clone(&on_input_detected),
/// );
/// rdev::listen(callback)?;
/// ```
fn build_event_callback(
    enabled: Arc<AtomicBool>,
    modifier_state: Arc<parking_lot::Mutex<ModifierState>>,
    on_escape_combo: Arc<dyn Fn() + Send + Sync>,
    on_input_detected: Arc<dyn Fn() + Send + Sync>,
) -> impl Fn(Event) {
    move |event: Event| {
        if !enabled.load(Ordering::Relaxed) {
            return;
        }

        // Track modifier keys for escape combo detection.
        match event.event_type {
            EventType::KeyPress(key) => {
                let mut mods = modifier_state.lock();
                mods.update(key, true);

                // Escape hatch: Ctrl+Alt+Shift+E exits break mode immediately.
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

        // Penalise any input event that occurs during an active break.
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
        mgr.stop();
        mgr.stop();
    }

    #[test]
    fn test_start_is_idempotent() {
        let mut mgr = make_manager();
        mgr.start();
        // listener_alive is true after the first start (thread was spawned)
        assert!(mgr.listener_alive.load(Ordering::SeqCst));
        // Second start must not spawn a second thread
        mgr.start();
        // Cleanup
        mgr.stop();
    }

    #[test]
    fn test_stop_clears_enabled_flag_and_sets_shutdown_requested() {
        let mut mgr = make_manager();
        mgr.start();
        assert!(mgr.enabled.load(Ordering::SeqCst));

        mgr.stop();
        assert!(!mgr.enabled.load(Ordering::SeqCst));
        // stop() must signal the supervisor loop to exit.
        assert!(mgr.shutdown_requested.load(Ordering::SeqCst));
    }

    #[test]
    fn test_stop_on_already_stopped_does_not_set_shutdown_requested() {
        let mut mgr = make_manager();
        // stop() on a never-started manager: enabled was already false, so the
        // early-return guard fires and shutdown_requested must stay false.
        mgr.stop();
        assert!(!mgr.shutdown_requested.load(Ordering::SeqCst));
    }

    #[test]
    fn test_start_after_stop_resets_shutdown_requested() {
        let mut mgr = make_manager();
        // Simulate a start → stop cycle.
        mgr.start();
        mgr.stop();
        assert!(mgr.shutdown_requested.load(Ordering::SeqCst));

        // Manually clear listener_alive so start() treats the thread as dead
        // (in production the thread sets this; here the rdev thread is blocked
        // so we simulate thread exit to test the start() reset path).
        mgr.listener_alive.store(false, Ordering::SeqCst);

        mgr.start();
        // start() must reset shutdown_requested before spawning.
        assert!(!mgr.shutdown_requested.load(Ordering::SeqCst));
        mgr.stop();
    }

    // Note: start() spawns a real OS thread via rdev::listen which blocks until
    // process exit. Callback invocation is therefore not tested at unit level;
    // it is covered by manual integration testing.
}
