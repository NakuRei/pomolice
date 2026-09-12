// Input Monitor Manager
// Manages global input monitoring without global statics

use rdev::{listen, Event, EventType, Key};
use std::collections::HashSet;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::Duration;

use crate::logging_event::{log_error_event, log_info_event, BackendLogEvent};

const INPUT_MONITOR_MAX_BACKOFF: Duration = Duration::from_secs(30);

type InputEventCallback = Box<dyn FnMut(Event) + Send + 'static>;

trait InputMonitorRuntime: Send + Sync {
    fn listen(&self, callback: InputEventCallback) -> Result<(), String>;
    fn sleep(&self, duration: Duration);
}

struct RdevInputMonitorRuntime;

impl InputMonitorRuntime for RdevInputMonitorRuntime {
    fn listen(&self, callback: InputEventCallback) -> Result<(), String> {
        listen(callback).map_err(|error| format!("{error:?}"))
    }

    fn sleep(&self, duration: Duration) {
        thread::sleep(duration);
    }
}

#[derive(Default, Clone)]
struct ModifierState {
    pressed_modifier_keys: HashSet<Key>,
}

impl ModifierState {
    fn update(&mut self, key: Key, pressed: bool) {
        if !is_modifier_key(key) {
            return;
        }

        if pressed {
            let _ = self.pressed_modifier_keys.insert(key);
        } else {
            let _ = self.pressed_modifier_keys.remove(&key);
        }
    }

    fn is_escape_combo(&self, key: Key) -> bool {
        matches!(key, Key::KeyE)
            && self.any_pressed([Key::ControlLeft, Key::ControlRight])
            && self.any_pressed([Key::Alt, Key::AltGr])
            && self.any_pressed([Key::ShiftLeft, Key::ShiftRight])
    }

    fn any_pressed(&self, keys: [Key; 2]) -> bool {
        keys.iter()
            .any(|key| self.pressed_modifier_keys.contains(key))
    }
}

fn is_modifier_key(key: Key) -> bool {
    matches!(
        key,
        Key::ControlLeft
            | Key::ControlRight
            | Key::Alt
            | Key::AltGr
            | Key::ShiftLeft
            | Key::ShiftRight
    )
}

/// Manager for global input monitoring
///
/// Callbacks are injected at construction time to decouple this manager
/// from Tauri's `AppHandle`. The caller (e.g. `AppState::new`) is responsible
/// for capturing any `AppHandle` clones inside the closures.
#[derive(Clone)]
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
    /// Callback-only modifier tracking; keep the guard around state updates only.
    modifier_state: Arc<parking_lot::Mutex<ModifierState>>,
    /// Called when the escape combo (Ctrl+Alt+Shift+E) is detected.
    on_escape_combo: Arc<dyn Fn() + Send + Sync>,
    /// Called when any input event (key, mouse, wheel) is detected during break.
    on_input_detected: Arc<dyn Fn() + Send + Sync>,
    on_failure: Arc<dyn Fn() + Send + Sync>,
    failure_observed: Arc<AtomicBool>,
    runtime: Arc<dyn InputMonitorRuntime>,
}

impl InputMonitorManager {
    /// Create a new input monitor manager
    ///
    /// - `on_escape_combo`: invoked when Ctrl+Alt+Shift+E is detected
    /// - `on_input_detected`: invoked on any input event while monitoring is active
    pub(crate) fn new(
        on_escape_combo: impl Fn() + Send + Sync + 'static,
        on_input_detected: impl Fn() + Send + Sync + 'static,
        on_failure: impl Fn() + Send + Sync + 'static,
    ) -> Self {
        Self::new_with_runtime(
            on_escape_combo,
            on_input_detected,
            on_failure,
            Arc::new(RdevInputMonitorRuntime),
        )
    }

    fn new_with_runtime(
        on_escape_combo: impl Fn() + Send + Sync + 'static,
        on_input_detected: impl Fn() + Send + Sync + 'static,
        on_failure: impl Fn() + Send + Sync + 'static,
        runtime: Arc<dyn InputMonitorRuntime>,
    ) -> Self {
        Self {
            enabled: Arc::new(AtomicBool::new(false)),
            listener_alive: Arc::new(AtomicBool::new(false)),
            shutdown_requested: Arc::new(AtomicBool::new(false)),
            modifier_state: Arc::new(parking_lot::Mutex::new(ModifierState::default())),
            on_escape_combo: Arc::new(on_escape_combo),
            on_input_detected: Arc::new(on_input_detected),
            on_failure: Arc::new(on_failure),
            failure_observed: Arc::new(AtomicBool::new(false)),
            runtime,
        }
    }

    pub(crate) fn has_observed_failure(&self) -> bool {
        self.failure_observed.load(Ordering::SeqCst)
    }

    /// Start global input monitoring.
    ///
    /// If a supervisor thread is already running (`listener_alive` is true), this
    /// call re-enables callbacks without spawning a new thread. Otherwise it resets
    /// the shutdown signal, marks the thread as alive, and spawns a new supervisor.
    pub(crate) fn start(&mut self) {
        log_info_event(BackendLogEvent::InputMonitoringStarted, &[]);

        // If the supervisor thread is already running, just re-enable callbacks.
        if self.listener_alive.load(Ordering::SeqCst) {
            self.shutdown_requested.store(false, Ordering::SeqCst);
            self.enabled.store(true, Ordering::SeqCst);
            log_info_event(BackendLogEvent::InputMonitoringAlreadyRunning, &[]);
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
        let on_failure = Arc::clone(&self.on_failure);
        let failure_observed = Arc::clone(&self.failure_observed);
        let runtime = Arc::clone(&self.runtime);

        thread::spawn(move || {
            let mut backoff = Duration::from_secs(1);

            loop {
                if shutdown_requested.load(Ordering::SeqCst) {
                    break;
                }

                let callback: InputEventCallback = Box::new(build_event_callback(
                    Arc::clone(&enabled),
                    Arc::clone(&modifier_state),
                    Arc::clone(&on_escape_combo),
                    Arc::clone(&on_input_detected),
                ));

                log_info_event(BackendLogEvent::InputMonitoringListenerStarted, &[]);
                match runtime.listen(callback) {
                    Ok(()) => {
                        // rdev returned cleanly. Treat as a stop signal.
                        log_info_event(BackendLogEvent::InputMonitoringListenerStoppedCleanly, &[]);
                        break;
                    }
                    Err(e) => {
                        if shutdown_requested.load(Ordering::SeqCst) {
                            log_info_event(
                                BackendLogEvent::InputMonitoringShutdownRequestedAfterError,
                                &[],
                            );
                            break;
                        }
                        log_error_event(
                            BackendLogEvent::InputMonitoringListenerFailed,
                            &[("backoff", format!("{backoff:?}")), ("error_message", e)],
                            None,
                        );
                        if !failure_observed.swap(true, Ordering::SeqCst) {
                            on_failure();
                        }
                        runtime.sleep(backoff);
                        if shutdown_requested.load(Ordering::SeqCst) {
                            log_info_event(
                                BackendLogEvent::InputMonitoringShutdownRequestedAfterError,
                                &[],
                            );
                            break;
                        }
                        backoff = (backoff * 2).min(INPUT_MONITOR_MAX_BACKOFF);
                    }
                }
            }

            // Always clear the alive flag so start() can spawn a new thread later.
            listener_alive.store(false, Ordering::SeqCst);
        });
    }

    pub(crate) fn rollback_break_enter_failure(&self) {
        self.stop_internal();
    }

    /// Stop global input monitoring
    pub(crate) fn stop(&mut self) {
        self.stop_internal();
    }

    fn stop_internal(&self) {
        if !self.enabled.swap(false, Ordering::SeqCst) {
            log_info_event(BackendLogEvent::InputMonitoringAlreadyStopped, &[]);
            return; // Already stopped, not an error
        }

        log_info_event(BackendLogEvent::InputMonitoringStopping, &[]);

        // Signal the supervisor loop to exit on its next iteration rather than retry.
        self.shutdown_requested.store(true, Ordering::SeqCst);

        // Reset modifier state so no stale key flags remain for the next session.
        let mut mods = self.modifier_state.lock();
        *mods = ModifierState::default();
    }

    #[cfg(test)]
    pub(crate) fn is_enabled_for_tests(&self) -> bool {
        self.enabled.load(Ordering::SeqCst)
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
                let is_escape_combo = {
                    let mut mods = modifier_state.lock();
                    mods.update(key, true);
                    mods.is_escape_combo(key)
                };

                // Escape hatch: Ctrl+Alt+Shift+E exits break mode immediately.
                if is_escape_combo {
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
    use rdev::Button;
    use std::collections::VecDeque;
    use std::sync::atomic::AtomicUsize;
    use std::sync::{Arc as StdArc, Mutex as StdMutex};

    fn make_manager() -> InputMonitorManager {
        InputMonitorManager::new(|| {}, || {}, || {})
    }

    fn make_manager_with_runtime(
        runtime: RecordingInputMonitorRuntime,
        on_input_detected: impl Fn() + Send + Sync + 'static,
    ) -> InputMonitorManager {
        make_manager_with_callbacks(runtime, || {}, on_input_detected)
    }

    fn make_manager_with_callbacks(
        runtime: RecordingInputMonitorRuntime,
        on_escape_combo: impl Fn() + Send + Sync + 'static,
        on_input_detected: impl Fn() + Send + Sync + 'static,
    ) -> InputMonitorManager {
        InputMonitorManager::new_with_runtime(
            on_escape_combo,
            on_input_detected,
            || {},
            StdArc::new(runtime),
        )
    }

    enum ListenStep {
        BlockThenOk,
        BlockThenErr(&'static str),
        ReturnOk,
    }

    #[derive(Clone)]
    struct RecordingInputMonitorRuntime {
        inner: StdArc<StdMutex<RecordingInputMonitorRuntimeState>>,
    }

    #[derive(Default)]
    struct RecordingInputMonitorRuntimeState {
        listen_steps: VecDeque<ListenStep>,
        listen_call_count: usize,
        completed_listen_call_count: usize,
        blocked_listen_call_count: usize,
        released_blocking_listen_call_count: usize,
        retry_wait_count: usize,
        completed_retry_wait_count: usize,
        released_retry_wait_count: usize,
        active_callback: Option<InputEventCallback>,
    }

    impl RecordingInputMonitorRuntime {
        fn with_steps(steps: impl IntoIterator<Item = ListenStep>) -> Self {
            Self {
                inner: StdArc::new(StdMutex::new(RecordingInputMonitorRuntimeState {
                    listen_steps: steps.into_iter().collect(),
                    ..RecordingInputMonitorRuntimeState::default()
                })),
            }
        }

        fn listen_call_count(&self) -> usize {
            self.inner
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .listen_call_count
        }

        fn retry_wait_count(&self) -> usize {
            self.inner
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .retry_wait_count
        }

        fn wait_for_retry_wait_count(&self, expected_count: usize, context: &str) {
            for _ in 0..100 {
                if self.retry_wait_count() >= expected_count {
                    return;
                }
                std::thread::sleep(Duration::from_millis(10));
            }

            panic!("timed out waiting for {context}");
        }

        fn wait_for_completed_retry_wait_count(&self, expected_count: usize, context: &str) {
            for _ in 0..100 {
                let completed_count = self
                    .inner
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner)
                    .completed_retry_wait_count;
                if completed_count >= expected_count {
                    return;
                }
                std::thread::sleep(Duration::from_millis(10));
            }

            panic!("timed out waiting for {context}");
        }

        fn wait_for_listen_call_count(&self, expected_count: usize, context: &str) {
            for _ in 0..100 {
                if self.listen_call_count() >= expected_count {
                    return;
                }
                std::thread::sleep(Duration::from_millis(10));
            }

            panic!("timed out waiting for {context}");
        }

        fn wait_for_completed_listen_call_count(&self, expected_count: usize, context: &str) {
            for _ in 0..100 {
                let completed_count = self
                    .inner
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner)
                    .completed_listen_call_count;
                if completed_count >= expected_count {
                    return;
                }
                std::thread::sleep(Duration::from_millis(10));
            }

            panic!("timed out waiting for {context}");
        }

        fn release_next_listener(&self) {
            let mut runtime_state = self
                .inner
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            runtime_state.released_blocking_listen_call_count += 1;
        }

        fn release_next_retry_wait(&self) {
            let mut runtime_state = self
                .inner
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            runtime_state.released_retry_wait_count += 1;
        }

        fn dispatch_event(&self, event: Event) {
            let mut runtime_state = self
                .inner
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            let Some(callback) = runtime_state.active_callback.as_mut() else {
                panic!("listener callback should be active");
            };
            callback(event);
        }

        fn wait_until_released(&self, block_index: usize) {
            loop {
                let is_released = self
                    .inner
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner)
                    .released_blocking_listen_call_count
                    >= block_index;
                if is_released {
                    return;
                }
                std::thread::sleep(Duration::from_millis(10));
            }
        }

        fn wait_until_retry_wait_released(&self, wait_index: usize) {
            loop {
                let is_released = self
                    .inner
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner)
                    .released_retry_wait_count
                    >= wait_index;
                if is_released {
                    return;
                }
                std::thread::sleep(Duration::from_millis(10));
            }
        }

        fn complete_listen_call(&self) {
            let mut runtime_state = self
                .inner
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            runtime_state.active_callback = None;
            runtime_state.completed_listen_call_count += 1;
        }
    }

    impl InputMonitorRuntime for RecordingInputMonitorRuntime {
        fn listen(&self, callback: InputEventCallback) -> Result<(), String> {
            let (block_index, step) = {
                let mut runtime_state = self
                    .inner
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner);
                let step = runtime_state
                    .listen_steps
                    .pop_front()
                    .unwrap_or(ListenStep::ReturnOk);
                runtime_state.listen_call_count += 1;
                runtime_state.active_callback = Some(callback);
                let block_index = match step {
                    ListenStep::BlockThenOk | ListenStep::BlockThenErr(_) => {
                        runtime_state.blocked_listen_call_count += 1;
                        Some(runtime_state.blocked_listen_call_count)
                    }
                    ListenStep::ReturnOk => None,
                };
                (block_index, step)
            };

            match step {
                ListenStep::BlockThenOk => {
                    let Some(block_index) = block_index else {
                        panic!("blocking listen should have index");
                    };
                    self.wait_until_released(block_index);
                    self.complete_listen_call();
                    Ok(())
                }
                ListenStep::BlockThenErr(error_message) => {
                    let Some(block_index) = block_index else {
                        panic!("blocking listen should have index");
                    };
                    self.wait_until_released(block_index);
                    self.complete_listen_call();
                    Err(error_message.to_string())
                }
                ListenStep::ReturnOk => {
                    self.complete_listen_call();
                    Ok(())
                }
            }
        }

        fn sleep(&self, _duration: Duration) {
            let wait_index = {
                let mut runtime_state = self
                    .inner
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner);
                runtime_state.retry_wait_count += 1;
                runtime_state.retry_wait_count
            };

            self.wait_until_retry_wait_released(wait_index);

            let mut runtime_state = self
                .inner
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            runtime_state.completed_retry_wait_count += 1;
        }
    }

    fn mouse_move_event() -> Event {
        input_event(EventType::MouseMove { x: 1.0, y: 1.0 })
    }

    fn key_press_event(key: Key) -> Event {
        input_event(EventType::KeyPress(key))
    }

    fn input_event(event_type: EventType) -> Event {
        Event {
            time: std::time::SystemTime::now(),
            name: None,
            event_type,
        }
    }

    fn press_keys(mods: &mut ModifierState, keys: impl IntoIterator<Item = Key>) {
        for key in keys {
            mods.update(key, true);
        }
    }

    fn release_keys(mods: &mut ModifierState, keys: impl IntoIterator<Item = Key>) {
        for key in keys {
            mods.update(key, false);
        }
    }

    #[test]
    fn test_escape_combo_requires_all_modifier_groups_and_e_key() {
        let mut mods = ModifierState::default();

        assert!(!mods.is_escape_combo(Key::KeyE));

        mods.update(Key::ControlLeft, true);
        assert!(!mods.is_escape_combo(Key::KeyE));

        mods.update(Key::Alt, true);
        assert!(!mods.is_escape_combo(Key::KeyE));

        mods.update(Key::ShiftLeft, true);
        assert!(mods.is_escape_combo(Key::KeyE));

        assert!(!mods.is_escape_combo(Key::KeyA));
    }

    #[test]
    fn test_escape_combo_accepts_supported_modifier_key_variants() {
        for (ctrl, alt, shift) in [
            (Key::ControlLeft, Key::Alt, Key::ShiftLeft),
            (Key::ControlRight, Key::Alt, Key::ShiftRight),
            (Key::ControlLeft, Key::AltGr, Key::ShiftRight),
            (Key::ControlRight, Key::AltGr, Key::ShiftLeft),
        ] {
            let mut mods = ModifierState::default();

            press_keys(&mut mods, [ctrl, alt, shift]);

            assert!(mods.is_escape_combo(Key::KeyE));
            assert!(!mods.is_escape_combo(Key::KeyA));
        }
    }

    #[test]
    fn test_escape_combo_stays_active_until_all_keys_in_modifier_group_are_released() {
        for (modifier_group_keys, other_required_keys) in [
            (
                [Key::ControlLeft, Key::ControlRight],
                [Key::Alt, Key::ShiftLeft],
            ),
            ([Key::Alt, Key::AltGr], [Key::ControlLeft, Key::ShiftLeft]),
            (
                [Key::ShiftLeft, Key::ShiftRight],
                [Key::ControlLeft, Key::Alt],
            ),
        ] {
            let mut mods = ModifierState::default();

            press_keys(&mut mods, modifier_group_keys);
            press_keys(&mut mods, other_required_keys);
            assert!(mods.is_escape_combo(Key::KeyE));

            mods.update(modifier_group_keys[0], false);
            assert!(mods.is_escape_combo(Key::KeyE));

            mods.update(modifier_group_keys[1], false);
            assert!(!mods.is_escape_combo(Key::KeyE));
        }
    }

    #[test]
    fn test_escape_combo_ignores_non_modifier_state_updates() {
        let mut mods = ModifierState::default();

        press_keys(&mut mods, [Key::ControlLeft, Key::Alt, Key::ShiftLeft]);
        assert!(mods.is_escape_combo(Key::KeyE));

        press_keys(&mut mods, [Key::KeyA, Key::KeyE, Key::Space]);
        release_keys(&mut mods, [Key::KeyA, Key::KeyE, Key::Space]);

        assert!(mods.is_escape_combo(Key::KeyE));
    }

    #[test]
    fn test_escape_combo_penalizes_setup_keys_but_not_completion_key() {
        let enabled = Arc::new(AtomicBool::new(true));
        let modifier_state = Arc::new(parking_lot::Mutex::new(ModifierState::default()));
        let escape_combo_count = Arc::new(AtomicUsize::new(0));
        let input_event_count = Arc::new(AtomicUsize::new(0));

        let escape_combo_count_for_callback = Arc::clone(&escape_combo_count);
        let input_event_count_for_callback = Arc::clone(&input_event_count);
        let callback = build_event_callback(
            Arc::clone(&enabled),
            Arc::clone(&modifier_state),
            Arc::new(move || {
                escape_combo_count_for_callback.fetch_add(1, Ordering::SeqCst);
            }),
            Arc::new(move || {
                input_event_count_for_callback.fetch_add(1, Ordering::SeqCst);
            }),
        );

        callback(key_press_event(Key::ControlLeft));
        assert_eq!(input_event_count.load(Ordering::SeqCst), 1);

        callback(key_press_event(Key::Alt));
        assert_eq!(input_event_count.load(Ordering::SeqCst), 2);

        callback(key_press_event(Key::ShiftLeft));
        assert_eq!(input_event_count.load(Ordering::SeqCst), 3);
        assert_eq!(escape_combo_count.load(Ordering::SeqCst), 0);

        callback(key_press_event(Key::KeyE));

        assert_eq!(escape_combo_count.load(Ordering::SeqCst), 1);
        assert!(!enabled.load(Ordering::SeqCst));
        assert_eq!(
            input_event_count.load(Ordering::SeqCst),
            3,
            "escape combo completion key must not report input detection"
        );

        callback(mouse_move_event());
        callback(key_press_event(Key::KeyA));

        assert_eq!(escape_combo_count.load(Ordering::SeqCst), 1);
        assert_eq!(
            input_event_count.load(Ordering::SeqCst),
            3,
            "input after escape combo must be ignored"
        );
    }

    #[test]
    fn test_stop_is_idempotent() {
        let mut mgr = make_manager();
        mgr.stop();
        mgr.stop();
    }

    #[test]
    fn test_start_is_idempotent() {
        let runtime = RecordingInputMonitorRuntime::with_steps([ListenStep::BlockThenOk]);
        let mut mgr = make_manager_with_runtime(runtime.clone(), || {});

        mgr.start();
        runtime.wait_for_listen_call_count(1, "the first input listener start");

        mgr.start();
        std::thread::sleep(Duration::from_millis(50));

        assert_eq!(
            runtime.listen_call_count(),
            1,
            "repeated start must not spawn another listener while one is active"
        );

        mgr.stop();
        runtime.release_next_listener();
        runtime.wait_for_completed_listen_call_count(1, "the active input listener to stop");
    }

    #[test]
    fn test_active_monitor_reports_all_penalized_input_event_types() {
        for (event_name, event_type) in [
            ("key press", EventType::KeyPress(Key::KeyA)),
            ("key release", EventType::KeyRelease(Key::KeyA)),
            ("button press", EventType::ButtonPress(Button::Left)),
            ("button release", EventType::ButtonRelease(Button::Left)),
            ("mouse move", EventType::MouseMove { x: 1.0, y: 1.0 }),
            (
                "wheel",
                EventType::Wheel {
                    delta_x: 0,
                    delta_y: 1,
                },
            ),
        ] {
            let runtime = RecordingInputMonitorRuntime::with_steps([ListenStep::BlockThenOk]);
            let input_event_count = StdArc::new(AtomicUsize::new(0));
            let input_event_count_for_callback = StdArc::clone(&input_event_count);
            let mut mgr = make_manager_with_runtime(runtime.clone(), move || {
                input_event_count_for_callback.fetch_add(1, Ordering::SeqCst);
            });

            mgr.start();
            runtime.wait_for_listen_call_count(1, "the input listener callback to become active");
            runtime.dispatch_event(input_event(event_type));
            let observed_input_event_count = input_event_count.load(Ordering::SeqCst);

            mgr.stop();
            runtime.release_next_listener();
            runtime.wait_for_completed_listen_call_count(1, "the stopped input listener to return");

            assert_eq!(
                observed_input_event_count, 1,
                "{event_name} must report input detection once while monitoring is active"
            );
        }
    }

    #[test]
    fn test_stop_disables_input_callbacks() {
        let runtime = RecordingInputMonitorRuntime::with_steps([ListenStep::BlockThenOk]);
        let input_event_count = StdArc::new(AtomicUsize::new(0));
        let input_event_count_for_callback = StdArc::clone(&input_event_count);
        let mut mgr = make_manager_with_runtime(runtime.clone(), move || {
            input_event_count_for_callback.fetch_add(1, Ordering::SeqCst);
        });

        mgr.start();
        runtime.wait_for_listen_call_count(1, "the input listener callback to become active");
        runtime.dispatch_event(mouse_move_event());
        assert_eq!(input_event_count.load(Ordering::SeqCst), 1);

        mgr.stop();
        runtime.dispatch_event(mouse_move_event());
        assert_eq!(
            input_event_count.load(Ordering::SeqCst),
            1,
            "stop must disable input callbacks while the listener is still alive"
        );

        runtime.release_next_listener();
        runtime.wait_for_completed_listen_call_count(1, "the stopped input listener to return");
    }

    #[test]
    fn test_stop_on_already_stopped_does_not_start_listener() {
        let runtime = RecordingInputMonitorRuntime::with_steps([]);
        let mut mgr = make_manager_with_runtime(runtime.clone(), || {});

        mgr.stop();
        mgr.stop();

        assert_eq!(
            runtime.listen_call_count(),
            0,
            "stopping an inactive manager must not start a listener"
        );
    }

    #[test]
    fn test_stop_before_listener_error_prevents_retry() {
        let runtime =
            RecordingInputMonitorRuntime::with_steps([ListenStep::BlockThenErr("listen failed")]);
        let mut mgr = make_manager_with_runtime(runtime.clone(), || {});

        mgr.start();
        runtime.wait_for_listen_call_count(1, "the first input listener start");
        mgr.stop();
        runtime.release_next_listener();
        runtime.wait_for_completed_listen_call_count(1, "the failed input listener to return");
        std::thread::sleep(Duration::from_millis(50));

        assert_eq!(
            runtime.listen_call_count(),
            1,
            "stopped listener errors must not retry listening"
        );
    }

    #[test]
    fn test_listener_error_waits_before_restart() {
        let runtime = RecordingInputMonitorRuntime::with_steps([
            ListenStep::BlockThenErr("listen failed"),
            ListenStep::BlockThenOk,
        ]);
        let mut mgr = make_manager_with_runtime(runtime.clone(), || {});

        mgr.start();
        runtime.wait_for_listen_call_count(1, "the first input listener start");
        runtime.release_next_listener();
        runtime.wait_for_completed_listen_call_count(1, "the failed input listener to return");
        runtime.wait_for_retry_wait_count(1, "retry wait after listener error");
        std::thread::sleep(Duration::from_millis(50));

        assert_eq!(
            runtime.listen_call_count(),
            1,
            "listener errors must wait before retrying"
        );

        runtime.release_next_retry_wait();
        runtime.wait_for_listen_call_count(2, "the restarted input listener");

        mgr.stop();
        runtime.release_next_listener();
        runtime.wait_for_completed_listen_call_count(2, "the restarted input listener to stop");
    }

    #[test]
    fn detected_failure_is_reported_and_remains_available_after_retry_and_stop() {
        struct ImmediateRetryRuntime(RecordingInputMonitorRuntime);

        impl InputMonitorRuntime for ImmediateRetryRuntime {
            fn listen(&self, callback: InputEventCallback) -> Result<(), String> {
                self.0.listen(callback)
            }

            fn sleep(&self, _duration: Duration) {}
        }

        let runtime = RecordingInputMonitorRuntime::with_steps([
            ListenStep::BlockThenErr("listen failed"),
            ListenStep::BlockThenOk,
        ]);
        let (sender, receiver) = std::sync::mpsc::channel();
        let mut manager = InputMonitorManager::new_with_runtime(
            || {},
            || {},
            move || {
                let _ = sender.send(());
            },
            StdArc::new(ImmediateRetryRuntime(runtime.clone())),
        );
        assert!(!manager.has_observed_failure());
        manager.start();
        runtime.wait_for_listen_call_count(1, "initial listener");
        runtime.release_next_listener();
        let notification = receiver.recv_timeout(Duration::from_secs(2));
        let failure_was_recorded = manager.has_observed_failure();
        runtime.wait_for_listen_call_count(2, "retry listener");
        manager.stop();
        runtime.release_next_listener();
        assert!(
            notification.is_ok(),
            "detected failures must notify subscribers"
        );
        assert!(
            failure_was_recorded,
            "late subscribers must be able to retrieve the failure"
        );
        assert!(
            manager.has_observed_failure(),
            "retrying or stopping is not proof of recovery"
        );
    }

    #[test]
    fn test_restarted_listener_reports_input_events() {
        let runtime = RecordingInputMonitorRuntime::with_steps([
            ListenStep::BlockThenErr("listen failed"),
            ListenStep::BlockThenOk,
        ]);
        let input_event_count = StdArc::new(AtomicUsize::new(0));
        let input_event_count_for_callback = StdArc::clone(&input_event_count);
        let mut mgr = make_manager_with_runtime(runtime.clone(), move || {
            input_event_count_for_callback.fetch_add(1, Ordering::SeqCst);
        });

        mgr.start();
        runtime.wait_for_listen_call_count(1, "the first input listener start");
        runtime.release_next_listener();
        runtime.wait_for_completed_listen_call_count(1, "the failed input listener to return");
        runtime.wait_for_retry_wait_count(1, "retry wait after listener error");
        runtime.release_next_retry_wait();
        runtime.wait_for_listen_call_count(2, "the restarted input listener");

        runtime.dispatch_event(mouse_move_event());

        assert_eq!(
            input_event_count.load(Ordering::SeqCst),
            1,
            "restarted listener must report input detection"
        );

        mgr.stop();
        runtime.release_next_listener();
        runtime.wait_for_completed_listen_call_count(2, "the restarted input listener to stop");
    }

    #[test]
    fn test_restarted_listener_preserves_escape_combo() {
        let runtime = RecordingInputMonitorRuntime::with_steps([
            ListenStep::BlockThenErr("listen failed"),
            ListenStep::BlockThenOk,
        ]);
        let escape_combo_count = StdArc::new(AtomicUsize::new(0));
        let input_event_count = StdArc::new(AtomicUsize::new(0));
        let escape_combo_count_for_callback = StdArc::clone(&escape_combo_count);
        let input_event_count_for_callback = StdArc::clone(&input_event_count);
        let mut mgr = make_manager_with_callbacks(
            runtime.clone(),
            move || {
                escape_combo_count_for_callback.fetch_add(1, Ordering::SeqCst);
            },
            move || {
                input_event_count_for_callback.fetch_add(1, Ordering::SeqCst);
            },
        );

        mgr.start();
        runtime.wait_for_listen_call_count(1, "the first input listener start");
        runtime.release_next_listener();
        runtime.wait_for_completed_listen_call_count(1, "the failed input listener to return");
        runtime.wait_for_retry_wait_count(1, "retry wait after listener error");
        runtime.release_next_retry_wait();
        runtime.wait_for_listen_call_count(2, "the restarted input listener");

        for key in [Key::ControlLeft, Key::Alt, Key::ShiftLeft, Key::KeyE] {
            runtime.dispatch_event(key_press_event(key));
        }

        assert_eq!(escape_combo_count.load(Ordering::SeqCst), 1);
        assert!(!mgr.is_enabled_for_tests());
        assert_eq!(
            input_event_count.load(Ordering::SeqCst),
            3,
            "escape combo completion key must not report input detection after restart"
        );

        runtime.dispatch_event(mouse_move_event());
        assert_eq!(
            input_event_count.load(Ordering::SeqCst),
            3,
            "input after restarted escape combo must be ignored"
        );

        runtime.release_next_listener();
        runtime.wait_for_completed_listen_call_count(2, "the restarted input listener to stop");
    }

    #[test]
    fn test_stop_during_retry_wait_prevents_restart() {
        let runtime = RecordingInputMonitorRuntime::with_steps([
            ListenStep::BlockThenErr("listen failed"),
            ListenStep::BlockThenOk,
        ]);
        let mut mgr = make_manager_with_runtime(runtime.clone(), || {});

        mgr.start();
        runtime.wait_for_listen_call_count(1, "the first input listener start");
        runtime.release_next_listener();
        runtime.wait_for_completed_listen_call_count(1, "the failed input listener to return");
        runtime.wait_for_retry_wait_count(1, "retry wait after listener error");

        mgr.stop();
        runtime.release_next_retry_wait();
        runtime.wait_for_completed_retry_wait_count(1, "retry wait to finish");
        std::thread::sleep(Duration::from_millis(50));

        assert_eq!(
            runtime.listen_call_count(),
            1,
            "stopping during retry wait must prevent restarting the listener"
        );
    }

    #[test]
    fn test_start_during_retry_wait_reenables_restart() {
        let runtime = RecordingInputMonitorRuntime::with_steps([
            ListenStep::BlockThenErr("listen failed"),
            ListenStep::BlockThenOk,
        ]);
        let mut mgr = make_manager_with_runtime(runtime.clone(), || {});

        mgr.start();
        runtime.wait_for_listen_call_count(1, "the first input listener start");
        runtime.release_next_listener();
        runtime.wait_for_completed_listen_call_count(1, "the failed input listener to return");
        runtime.wait_for_retry_wait_count(1, "retry wait after listener error");

        mgr.stop();
        mgr.start();
        runtime.release_next_retry_wait();
        runtime.wait_for_listen_call_count(2, "the restarted input listener");

        mgr.stop();
        runtime.release_next_listener();
        runtime.wait_for_completed_listen_call_count(2, "the restarted input listener to stop");
    }

    #[test]
    fn test_start_after_listener_exit_starts_new_listener() {
        let runtime = RecordingInputMonitorRuntime::with_steps([
            ListenStep::ReturnOk,
            ListenStep::BlockThenOk,
        ]);
        let mut mgr = make_manager_with_runtime(runtime.clone(), || {});

        mgr.start();
        runtime.wait_for_completed_listen_call_count(1, "the first input listener to exit");

        for _ in 0..100 {
            mgr.start();
            if runtime.listen_call_count() >= 2 {
                break;
            }
            std::thread::sleep(Duration::from_millis(10));
        }

        assert_eq!(
            runtime.listen_call_count(),
            2,
            "start must launch a new listener after the previous listener exits"
        );

        mgr.stop();
        runtime.release_next_listener();
        runtime.wait_for_completed_listen_call_count(2, "the second input listener to stop");
    }
}
