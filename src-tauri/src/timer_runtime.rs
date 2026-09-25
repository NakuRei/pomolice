// The shell around `TimerCore`: the only place that applies events and runs
// their effects. One permit serializes apply and effects together, so the
// order the core returns is the order the outside world observes.

use std::collections::VecDeque;
use std::sync::{Arc, Mutex as StdMutex};

use chrono::{DateTime, Local};
use tokio::sync::Semaphore;

use crate::error::{PersistenceFailure, PomoliceError, Result};
use crate::external_adapters::clock::Clock;
use crate::external_adapters::persistence::{PersistenceManager, SaveError};
use crate::logging_event::{log_error_event, BackendLogEvent};
use crate::timer_core::{Effect, Event, TimerCore};
use crate::types::TimerTickPayload;

/// Where effects go. The application handle is the only production sink.
pub(crate) trait EffectSink {
    fn emit(&self, event_name: &str, payload: &(impl serde::Serialize + Clone));
    fn update_tray(&self, payload: &TimerTickPayload);
    async fn enter_break_lock(&self) -> Event;
    async fn release_break_lock(&self) -> Event;
}

/// Owns the core, the clock it is driven by, the store it persists to, and
/// the permit that serializes every change. The fields are private so a
/// transition can only be applied here, under the permit, with its effects
/// run before the next one starts.
pub(crate) struct TimerRuntime {
    /// Locked only inside synchronous critical sections; never held across an await.
    core: StdMutex<TimerCore>,
    /// Serializes every dispatch (apply plus effect execution). Held across awaits.
    dispatch_permit: Semaphore,
    clock: Arc<dyn Clock + Send + Sync>,
    persistence: PersistenceManager,
}

impl TimerRuntime {
    pub(crate) fn new(
        core: TimerCore,
        clock: Arc<dyn Clock + Send + Sync>,
        persistence: PersistenceManager,
    ) -> Self {
        Self {
            core: StdMutex::new(core),
            dispatch_permit: Semaphore::new(1),
            clock,
            persistence,
        }
    }

    pub(crate) async fn dispatch(&self, sink: &impl EffectSink, event: Event) -> Result<()> {
        self.dispatch_then_read(sink, event, |_, _| ()).await
    }

    /// Applies `event`, runs its effects, then reads from the core before the
    /// permit is released, so the read sees exactly the dispatched state.
    pub(crate) async fn dispatch_then_read<T>(
        &self,
        sink: &impl EffectSink,
        event: Event,
        read: impl FnOnce(&TimerCore, DateTime<Local>) -> T,
    ) -> Result<T> {
        let _permit = self.acquire_dispatch_permit().await;
        let now = self.clock.now_local();
        let effects = self.apply(event, now)?;
        let effect_result = self.run_effects(sink, effects, now).await;
        let value = self.read(|core| read(core, now));
        effect_result.map(|()| value)
    }

    /// Synchronous save for `RunEvent::Exit`, called on the main thread.
    ///
    /// This must not wait for the dispatch permit: effects running under it
    /// block on the main thread (window queries, tray updates), so waiting
    /// here would deadlock. The permit is not needed either. The core lock is
    /// never held across an await, so it is always available, and a dispatch
    /// waits for its own save to finish before the state moves on, so no save
    /// older than the snapshot read here can land after it.
    pub(crate) fn flush_sync_on_exit(&self) -> std::result::Result<(), SaveError> {
        let saved_at = self.clock.now_utc();
        let now = self.clock.now_local();
        let persisted_state = self.read(|core| core.persisted_state(now, saved_at));
        self.persistence.save_state_sync(&persisted_state)
    }

    fn apply(&self, event: Event, now: DateTime<Local>) -> Result<Vec<Effect>> {
        self.lock_core().apply(event, now)
    }

    fn read<T>(&self, read: impl FnOnce(&TimerCore) -> T) -> T {
        read(&self.lock_core())
    }

    fn lock_core(&self) -> std::sync::MutexGuard<'_, TimerCore> {
        self.core
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    async fn acquire_dispatch_permit(&self) -> tokio::sync::SemaphorePermit<'_> {
        let Ok(permit) = self.dispatch_permit.acquire().await else {
            panic!("dispatch permit should never be closed");
        };
        permit
    }

    async fn run_effects(
        &self,
        sink: &impl EffectSink,
        effects: Vec<Effect>,
        now: DateTime<Local>,
    ) -> Result<()> {
        let mut queue: VecDeque<Effect> = effects.into();
        let mut first_error = None;

        while let Some(effect) = queue.pop_front() {
            let follow_up = match effect {
                Effect::PublishTimerState(payload) => {
                    sink.emit("timer_tick", &payload);
                    sink.update_tray(&payload);
                    None
                }
                Effect::EmitSessionCompleted(payload) => {
                    sink.emit("session_completed", &payload);
                    None
                }
                Effect::EmitAutoRunStopPending => {
                    sink.emit("auto_run_stop_pending", &serde_json::json!({}));
                    None
                }
                Effect::EmitAutoRunStopped => {
                    sink.emit("auto_run_stopped", &serde_json::json!({}));
                    None
                }
                Effect::EmitActiveWindowChanged(payload) => {
                    sink.emit("active_window_changed", &payload);
                    None
                }
                Effect::Persist => {
                    if let Err(error) = self.persist(now).await {
                        first_error.get_or_insert(error);
                    }
                    None
                }
                Effect::EnterBreakLock => Some(sink.enter_break_lock().await),
                Effect::ReleaseBreakLock => Some(sink.release_break_lock().await),
            };

            if let Some(follow_up) = follow_up {
                match self.apply(follow_up, now) {
                    Ok(more) => queue.extend(more),
                    Err(error) => {
                        first_error.get_or_insert(error);
                    }
                }
            }
        }

        first_error.map_or(Ok(()), Err)
    }

    async fn persist(&self, now: DateTime<Local>) -> Result<()> {
        let saved_at = self.clock.now_utc();
        let persisted_state = self.read(|core| core.persisted_state(now, saved_at));
        let persistence = self.persistence.clone();

        let save_result =
            tokio::task::spawn_blocking(move || persistence.save_state_sync(&persisted_state))
                .await;
        match save_result {
            Ok(Ok(())) => Ok(()),
            Ok(Err(error)) => Err(PomoliceError::PersistenceSave(
                PersistenceFailure::from_error(error),
            )),
            Err(source) => {
                log_error_event(
                    BackendLogEvent::StoragePersistenceWriteFailed,
                    &[],
                    Some(&source),
                );
                Err(PomoliceError::PersistenceTaskJoin { source })
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::external_adapters::clock::testing::FixedClock;
    use crate::external_adapters::persistence::{PersistenceLoadOutcome, SaveError};
    use crate::types::{
        PersistedState, PostBreakDecisionAction, PostBreakTransitionPolicy, TimerSettings,
        TimerStatus, WeeklyAutoRunConfig,
    };
    use chrono::{Local, TimeZone, Utc};
    use std::path::PathBuf;
    use std::sync::Mutex;
    use std::time::{SystemTime, UNIX_EPOCH};

    /// Records every emitted event with its payload and answers a break-lock
    /// request with a configured outcome.
    struct RecordingSink {
        emitted: Mutex<Vec<(String, serde_json::Value)>>,
        lock_outcome: Event,
        lock_takes: Option<std::time::Duration>,
    }

    impl RecordingSink {
        fn with_lock_outcome(lock_outcome: Event) -> Self {
            Self {
                emitted: Mutex::new(Vec::new()),
                lock_outcome,
                lock_takes: None,
            }
        }

        /// Makes entering the lock take real time, which is where a second
        /// dispatch would interleave if dispatches were not serialized.
        fn with_slow_lock(mut self, lock_takes: std::time::Duration) -> Self {
            self.lock_takes = Some(lock_takes);
            self
        }

        fn timer_tick_statuses(&self) -> Vec<String> {
            self.emitted
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .iter()
                .filter(|(name, _)| name == "timer_tick")
                .map(|(_, payload)| payload["status"].as_str().unwrap_or("").to_string())
                .collect()
        }

        fn emitted_names(&self) -> Vec<String> {
            self.emitted
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .iter()
                .map(|(name, _)| name.clone())
                .collect()
        }

        fn last_timer_tick(&self) -> Option<serde_json::Value> {
            self.emitted
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .iter()
                .rev()
                .find(|(name, _)| name == "timer_tick")
                .map(|(_, payload)| payload.clone())
        }
    }

    impl EffectSink for RecordingSink {
        fn emit(&self, event_name: &str, payload: &(impl serde::Serialize + Clone)) {
            self.emitted
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .push((
                    event_name.to_string(),
                    serde_json::to_value(payload).expect("payload should serialize"),
                ));
        }

        fn update_tray(&self, _payload: &TimerTickPayload) {}

        async fn enter_break_lock(&self) -> Event {
            if let Some(lock_takes) = self.lock_takes {
                tokio::time::sleep(lock_takes).await;
            }
            self.lock_outcome.clone()
        }

        async fn release_break_lock(&self) -> Event {
            Event::BreakLockReleased
        }
    }

    fn create_temp_dir(test_name: &str) -> PathBuf {
        let timestamp_nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time should be after unix epoch")
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "pomolice-runtime-{test_name}-{}-{timestamp_nanos}",
            std::process::id(),
        ));
        std::fs::create_dir_all(&path).expect("temp directory should be created");
        path
    }

    fn fixed_clock() -> Arc<FixedClock> {
        let now_utc = Utc
            .with_ymd_and_hms(2025, 2, 3, 10, 0, 0)
            .single()
            .expect("valid UTC datetime");
        Arc::new(FixedClock::new(now_utc.with_timezone(&Local)))
    }

    fn idle_runtime(persistence: PersistenceManager, clock: &Arc<FixedClock>) -> TimerRuntime {
        let clock_trait: Arc<dyn Clock + Send + Sync> = Arc::<FixedClock>::clone(clock);
        TimerRuntime::new(
            TimerCore::restore(None, clock.now_local()),
            clock_trait,
            persistence,
        )
    }

    fn one_second_work_settings() -> TimerSettings {
        TimerSettings::try_from_raw(
            1,
            60,
            120,
            4,
            PostBreakTransitionPolicy::RequireManualContinue,
        )
        .expect("valid settings")
    }

    fn saved_state(runtime: &TimerRuntime) -> Option<PersistedState> {
        match runtime.persistence.load_state_sync() {
            PersistenceLoadOutcome::Restored(persisted) => Some(persisted),
            PersistenceLoadOutcome::NoState | PersistenceLoadOutcome::UnsupportedSchemaFallback => {
                None
            }
        }
    }

    async fn start_default_work(runtime: &TimerRuntime, sink: &RecordingSink) {
        runtime
            .dispatch(
                sink,
                Event::StartWorkRequested {
                    settings: TimerSettings::default(),
                },
            )
            .await
            .expect("start should succeed");
    }

    #[tokio::test]
    async fn a_persistence_failure_is_returned_after_the_state_changed() {
        let clock = fixed_clock();
        let persistence = PersistenceManager::new(Some(create_temp_dir("persist-failure")))
            .with_injected_save_error_hook(Arc::new(|| {
                Some(SaveError::Write {
                    path: PathBuf::from("timer_state.json"),
                    source: std::io::Error::other("injected"),
                })
            }));
        let runtime = idle_runtime(persistence, &clock);
        let sink = RecordingSink::with_lock_outcome(Event::BreakLockEntered);

        let result = runtime
            .dispatch(
                &sink,
                Event::StartWorkRequested {
                    settings: TimerSettings::default(),
                },
            )
            .await;

        assert!(matches!(result, Err(PomoliceError::PersistenceSave(_))));
        let last_tick = sink.last_timer_tick().expect("a tick should be emitted");
        assert_eq!(last_tick["status"], "working");
    }

    #[tokio::test]
    async fn a_rejected_event_runs_no_effects() {
        let clock = fixed_clock();
        let temp_dir = create_temp_dir("rejected-event");
        let runtime = idle_runtime(PersistenceManager::new(Some(temp_dir.clone())), &clock);
        let sink = RecordingSink::with_lock_outcome(Event::BreakLockEntered);

        let result = runtime
            .dispatch(
                &sink,
                Event::PostBreakDecisionResolved {
                    action: PostBreakDecisionAction::Continue,
                },
            )
            .await;

        assert!(matches!(
            result,
            Err(PomoliceError::InvalidStateTransition { .. })
        ));
        assert!(sink.emitted_names().is_empty());
        assert!(!temp_dir.join("timer_state.json").exists());
    }

    #[tokio::test]
    async fn the_frontend_receives_the_break_with_its_lock_outcome_and_it_is_persisted() {
        let clock = fixed_clock();
        let temp_dir = create_temp_dir("break-saved");
        let runtime = idle_runtime(PersistenceManager::new(Some(temp_dir.clone())), &clock);
        let sink = RecordingSink::with_lock_outcome(Event::BreakLockEntered);
        runtime
            .dispatch(
                &sink,
                Event::StartWorkRequested {
                    settings: one_second_work_settings(),
                },
            )
            .await
            .expect("start should succeed");
        clock.set_now_local(clock.now_local() + chrono::Duration::seconds(1));

        runtime
            .dispatch(&sink, Event::Tick)
            .await
            .expect("tick should succeed");

        assert!(sink
            .emitted_names()
            .contains(&"session_completed".to_string()));
        let last_tick = sink.last_timer_tick().expect("a tick should be emitted");
        assert_eq!(last_tick["status"], "on_break");
        assert_eq!(last_tick["is_fullscreen_break"], true);
        let saved = saved_state(&runtime)
            .expect("the break should have been saved by the time the tick returned");
        assert_eq!(saved.status, TimerStatus::OnBreak);
    }

    #[tokio::test]
    async fn starting_work_is_saved_before_the_command_returns() {
        let clock = fixed_clock();
        let runtime = idle_runtime(
            PersistenceManager::new(Some(create_temp_dir("start-saved"))),
            &clock,
        );
        let sink = RecordingSink::with_lock_outcome(Event::BreakLockEntered);

        start_default_work(&runtime, &sink).await;

        let saved = saved_state(&runtime).expect("the started session should be saved");
        assert_eq!(saved.status, TimerStatus::Working);
    }

    #[tokio::test]
    async fn an_emergency_stop_is_saved_before_the_command_returns() {
        let clock = fixed_clock();
        let runtime = idle_runtime(
            PersistenceManager::new(Some(create_temp_dir("stop-saved"))),
            &clock,
        );
        let sink = RecordingSink::with_lock_outcome(Event::BreakLockEntered);
        start_default_work(&runtime, &sink).await;

        runtime
            .dispatch(&sink, Event::EmergencyStopRequested)
            .await
            .expect("stop should succeed");

        let saved = saved_state(&runtime).expect("the stop should be saved");
        assert_eq!(saved.status, TimerStatus::Idle);
    }

    #[tokio::test]
    async fn a_schedule_change_is_saved_before_the_command_returns() {
        let clock = fixed_clock();
        let runtime = idle_runtime(
            PersistenceManager::new(Some(create_temp_dir("schedule-saved"))),
            &clock,
        );
        let sink = RecordingSink::with_lock_outcome(Event::BreakLockEntered);

        runtime
            .dispatch(
                &sink,
                Event::AutomationPoliciesChanged {
                    auto_run_feature_enabled: true,
                    weekly_auto_run_config: WeeklyAutoRunConfig::default(),
                    post_break_transition_policy:
                        PostBreakTransitionPolicy::StartNextWorkAutomatically,
                },
            )
            .await
            .expect("schedule change should succeed");

        let saved = saved_state(&runtime).expect("the schedule change should be saved");
        assert!(saved.auto_run_feature_enabled);
        assert_eq!(
            saved.settings.post_break_transition_policy,
            PostBreakTransitionPolicy::StartNextWorkAutomatically
        );
    }

    #[tokio::test]
    async fn idle_ticks_do_not_write_to_disk() {
        let clock = fixed_clock();
        let runtime = idle_runtime(
            PersistenceManager::new(Some(create_temp_dir("idle-no-write"))),
            &clock,
        );
        let sink = RecordingSink::with_lock_outcome(Event::BreakLockEntered);

        for _ in 0..3 {
            clock.set_now_local(clock.now_local() + chrono::Duration::seconds(1));
            runtime
                .dispatch(&sink, Event::Tick)
                .await
                .expect("idle tick should succeed");
        }

        assert!(saved_state(&runtime).is_none());
    }

    #[tokio::test]
    async fn dispatches_do_not_interleave_even_while_an_effect_is_still_running() {
        let clock = fixed_clock();
        let runtime = idle_runtime(
            PersistenceManager::new(Some(create_temp_dir("serialized"))),
            &clock,
        );
        let sink = RecordingSink::with_lock_outcome(Event::BreakLockEntered)
            .with_slow_lock(std::time::Duration::from_millis(50));
        runtime
            .dispatch(
                &sink,
                Event::StartWorkRequested {
                    settings: one_second_work_settings(),
                },
            )
            .await
            .expect("start should succeed");
        clock.set_now_local(clock.now_local() + chrono::Duration::seconds(1));

        // The tick is polled first and parks inside the slow break lock. If
        // the stop could run meanwhile, it would reset the state before the
        // lock outcome is applied, and the frontend would never be told about
        // the break, or would hear about it after the stop.
        let (tick, stop) = tokio::join!(
            runtime.dispatch(&sink, Event::Tick),
            runtime.dispatch(&sink, Event::EmergencyStopRequested),
        );

        tick.expect("tick should succeed");
        stop.expect("stop should succeed");
        let statuses = sink.timer_tick_statuses();
        let break_reported = statuses
            .iter()
            .rposition(|status| status == "on_break")
            .expect("the stop overtook the break: dispatches interleaved");
        let stop_reported = statuses
            .iter()
            .position(|status| status == "idle")
            .expect("the stop should reach the frontend");
        assert!(
            break_reported < stop_reported,
            "the stop was reported before the break: dispatches interleaved"
        );
    }

    #[tokio::test]
    async fn exit_flush_saves_while_a_dispatch_is_still_running_its_effects() {
        let clock = fixed_clock();
        let runtime = idle_runtime(
            PersistenceManager::new(Some(create_temp_dir("exit-during-dispatch"))),
            &clock,
        );
        let sink = RecordingSink::with_lock_outcome(Event::BreakLockEntered)
            .with_slow_lock(std::time::Duration::from_millis(50));
        runtime
            .dispatch(
                &sink,
                Event::StartWorkRequested {
                    settings: one_second_work_settings(),
                },
            )
            .await
            .expect("start should succeed");
        clock.set_now_local(clock.now_local() + chrono::Duration::seconds(1));

        // The tick is polled first and parks inside the slow break lock, so the
        // flush runs while that dispatch still holds the permit.
        let (tick, flushed) = tokio::join!(runtime.dispatch(&sink, Event::Tick), async {
            runtime.flush_sync_on_exit()
        });

        flushed.expect("exit flush should not wait for the running dispatch");
        tick.expect("tick should succeed");
        let saved = saved_state(&runtime).expect("state should be saved");
        assert_eq!(saved.status, TimerStatus::OnBreak);
    }
}
