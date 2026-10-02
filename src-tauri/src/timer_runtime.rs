pub(crate) mod observation;

// The shell around `TimerCore`: the only place that applies events and runs
// their effects. One permit keeps each published snapshot consistent with
// the completed effects before the next transition starts.

use std::sync::{Arc, Mutex as StdMutex};

use chrono::{DateTime, Local};
use tokio::sync::Semaphore;

use crate::error::{PersistenceFailure, PomoliceError, Result};
use crate::external_adapters::clock::Clock;
use crate::external_adapters::persistence::{PersistedDocument, PersistenceManager, SaveError};
use crate::logging_event::{log_error_event, log_warn_event, BackendLogEvent};
use crate::storage::PersistedState;
use crate::storage::{PersistedSettings, SETTINGS_SCHEMA_VERSION};
use crate::timer_runtime::observation::BreakLockAction;
use crate::timer_runtime::observation::BreakLockOutcome;
use crate::timer_runtime::observation::BreakLockState;
use crate::timer_runtime::observation::EffectStatus;
use crate::timer_runtime::observation::TimerEffectsState;
use crate::timer_runtime::observation::{FullStatePayload, SnapshotRevision, TimerSnapshot};
use pomolice_core::settings::input::{SettingsChanges, SettingsInput};
use pomolice_core::settings::UserSettings;
use pomolice_core::timer_core::{BreakLockTarget, Effect, Event, TimerCore};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum TimerNotification {
    SnapshotChanged(Box<TimerSnapshot>),
    SessionCompleted(pomolice_core::timer_core::observation::SessionCompletedPayload),
    AutoRunStopPending,
    AutoRunStopped,
}

/// Where effects go. The application handle is the only production sink.
pub(crate) trait EffectSink {
    fn notify(&self, notification: TimerNotification);
    async fn enter_break_lock(&self) -> Result<()>;
    async fn release_break_lock(&self) -> Result<()>;
}

/// Owns the core, the clock it is driven by, the files it persists to, and
/// the permit that serializes every change. The fields are private so a
/// transition can only be applied here, under the permit, with its effects
/// run before the next one starts.
pub(crate) struct TimerRuntime {
    /// Locked only inside synchronous critical sections; never held across an await.
    core: StdMutex<TimerCore<Local>>,
    settings_file_status: StdMutex<crate::storage::SettingsFileStatus>,
    published: StdMutex<TimerSnapshot>,
    /// Serializes every dispatch (apply plus effect execution). Held across awaits.
    dispatch_permit: Semaphore,
    clock: Arc<dyn Clock + Send + Sync>,
    timer_state_persistence: PersistenceManager<PersistedState>,
    settings_persistence: PersistenceManager<PersistedSettings>,
}

impl TimerRuntime {
    pub(crate) fn new(
        core: TimerCore<Local>,
        clock: Arc<dyn Clock + Send + Sync>,
        timer_state_persistence: PersistenceManager<PersistedState>,
        settings_persistence: PersistenceManager<PersistedSettings>,
        settings_file_status: crate::storage::SettingsFileStatus,
    ) -> Self {
        let published = TimerSnapshot {
            revision: SnapshotRevision::default(),
            timer: FullStatePayload {
                state: core.snapshot(clock.now_utc()),
                settings_file_status,
            },
            effects: TimerEffectsState::default(),
        };
        Self {
            core: StdMutex::new(core),
            settings_file_status: StdMutex::new(settings_file_status),
            published: StdMutex::new(published),
            dispatch_permit: Semaphore::new(1),
            clock,
            timer_state_persistence,
            settings_persistence,
        }
    }

    pub(crate) async fn run_initial_effects(&self, sink: &impl EffectSink, effects: Vec<Effect>) {
        let _permit = self.acquire_dispatch_permit().await;
        let revision = self.snapshot().revision;
        self.run_effects(sink, effects, self.clock.now_utc()).await;
        let snapshot = self.snapshot();
        if snapshot.revision == revision {
            sink.notify(TimerNotification::SnapshotChanged(Box::new(snapshot)));
        }
    }

    pub(crate) async fn dispatch(
        &self,
        sink: &impl EffectSink,
        event: Event,
    ) -> Result<TimerSnapshot> {
        let _permit = self.acquire_dispatch_permit().await;
        self.apply_and_run_effects(sink, event).await
    }

    /// Returns the last published observation, including completed effects.
    /// Reading does not advance the countdown or the revision.
    pub(crate) async fn read_full_state(&self) -> TimerSnapshot {
        let _permit = self.acquire_dispatch_permit().await;
        self.snapshot()
    }

    /// Writes `settings.json` first and changes the core only once that
    /// succeeded, so the file and the running timer never disagree. The new
    /// settings reach the frontend as an event, in order with every other
    /// state event. A save failure rejects the update before changing the core.
    ///
    /// The changes are laid over the settings in effect while the permit is
    /// held, so saves that overlap never undo each other.
    pub(crate) async fn update_settings(
        &self,
        sink: &impl EffectSink,
        changes: SettingsChanges,
    ) -> Result<TimerSnapshot> {
        let _permit = self.acquire_dispatch_permit().await;
        if changes.is_empty() {
            return Ok(self.snapshot());
        }

        let current = self.read(|core| SettingsInput::from(core.settings().clone()));
        let settings = UserSettings::try_from(changes.applied_to(current)).map_err(|errors| {
            log_warn_event(
                BackendLogEvent::SettingsUpdateRejected,
                &[("issue_count", errors.issues().len().to_string())],
            );
            PomoliceError::InvalidSettings(errors)
        })?;
        save_in_background(
            &self.settings_persistence,
            PersistedSettings {
                schema_version: SETTINGS_SCHEMA_VERSION,
                settings: settings.clone(),
            },
        )
        .await?;
        *self
            .settings_file_status
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) =
            crate::storage::SettingsFileStatus::Ok;
        self.apply_and_run_effects(sink, Event::SettingsChanged { settings })
            .await
    }

    /// Must be called while holding the dispatch permit.
    async fn apply_and_run_effects(
        &self,
        sink: &impl EffectSink,
        event: Event,
    ) -> Result<TimerSnapshot> {
        let now = self.clock.now_utc();
        let effects = self.apply(event, now)?;
        self.run_effects(sink, effects, now).await;
        Ok(self.snapshot())
    }

    pub(crate) async fn retry_failed_effects(&self, sink: &impl EffectSink) -> TimerSnapshot {
        let _permit = self.acquire_dispatch_permit().await;
        let failed = self.effect_state();
        let mut effects = Vec::new();
        if failed
            .break_lock
            .last_attempt
            .is_some_and(|attempt| attempt.status == EffectStatus::Failed)
        {
            effects.push(Effect::SetBreakLockTarget(failed.break_lock.target));
        }
        if failed.last_timer_save == Some(EffectStatus::Failed) {
            effects.push(Effect::Persist);
        }
        self.run_effects(sink, effects, self.clock.now_utc()).await;
        self.snapshot()
    }

    fn effect_state(&self) -> TimerEffectsState {
        self.published
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .effects
    }

    fn settings_file_status(&self) -> crate::storage::SettingsFileStatus {
        *self
            .settings_file_status
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    fn snapshot(&self) -> TimerSnapshot {
        self.published
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone()
    }

    fn publish_snapshot(
        &self,
        sink: &impl EffectSink,
        effects: TimerEffectsState,
        now: DateTime<chrono::Utc>,
    ) {
        let timer = FullStatePayload {
            state: self.read(|core| core.snapshot(now)),
            settings_file_status: self.settings_file_status(),
        };
        let snapshot = {
            let mut published = self
                .published
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            if published.timer == timer && published.effects == effects {
                return;
            }
            *published = TimerSnapshot {
                revision: SnapshotRevision(published.revision.0 + 1),
                timer,
                effects,
            };
            published.clone()
        };
        sink.notify(TimerNotification::SnapshotChanged(Box::new(snapshot)));
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
        let now = self.clock.now_utc();
        let persisted_state = self.read(|core| PersistedState::from(core.checkpoint(now)));
        self.timer_state_persistence.save_sync(&persisted_state)
    }

    fn apply(&self, event: Event, now: DateTime<chrono::Utc>) -> Result<Vec<Effect>> {
        self.lock_core().apply(event, now).map_err(Into::into)
    }

    fn read<T>(&self, read: impl FnOnce(&TimerCore<Local>) -> T) -> T {
        read(&self.lock_core())
    }

    fn lock_core(&self) -> std::sync::MutexGuard<'_, TimerCore<Local>> {
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
        now: DateTime<chrono::Utc>,
    ) {
        let mut outcomes = self.effect_state();
        let mut notifications = Vec::new();

        for effect in effects {
            match effect {
                Effect::EmitSessionCompleted(payload) => {
                    notifications.push(TimerNotification::SessionCompleted(payload));
                }
                Effect::EmitAutoRunStopPending => {
                    notifications.push(TimerNotification::AutoRunStopPending);
                }
                Effect::EmitAutoRunStopped => {
                    notifications.push(TimerNotification::AutoRunStopped);
                }
                Effect::Persist => {
                    outcomes.last_timer_save = Some(if self.persist(now).await.is_ok() {
                        EffectStatus::Succeeded
                    } else {
                        EffectStatus::Failed
                    });
                }
                Effect::SetBreakLockTarget(target) => {
                    outcomes.break_lock.target = target;
                    reconcile_break_lock(sink, &mut outcomes.break_lock).await;
                }
            }
        }

        self.publish_snapshot(sink, outcomes, now);
        for notification in notifications {
            sink.notify(notification);
        }
    }

    async fn persist(&self, now: DateTime<chrono::Utc>) -> Result<()> {
        let persisted_state = self.read(|core| PersistedState::from(core.checkpoint(now)));
        save_in_background(&self.timer_state_persistence, persisted_state).await
    }
}

async fn save_in_background<T: PersistedDocument + Send + 'static>(
    persistence: &PersistenceManager<T>,
    document: T,
) -> Result<()> {
    let persistence = persistence.clone();
    let save_result = tokio::task::spawn_blocking(move || persistence.save_sync(&document)).await;
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

async fn reconcile_break_lock(sink: &impl EffectSink, state: &mut BreakLockState) {
    match state.target {
        BreakLockTarget::Unlocked => {
            if state.last_attempt.is_some_and(|attempt| {
                attempt.action != BreakLockAction::Release || attempt.status == EffectStatus::Failed
            }) {
                state.last_attempt = Some(attempt_break_lock(sink, BreakLockAction::Release).await);
            }
        }
        BreakLockTarget::Locked => {
            if state
                .last_attempt
                .is_some_and(|attempt| attempt.status == EffectStatus::Failed)
            {
                let cleanup = attempt_break_lock(sink, BreakLockAction::Release).await;
                state.last_attempt = Some(cleanup);
                if cleanup.status == EffectStatus::Failed {
                    return;
                }
            }
            if !state.last_attempt.is_some_and(|attempt| {
                attempt.action == BreakLockAction::Enter
                    && attempt.status == EffectStatus::Succeeded
            }) {
                state.last_attempt = Some(attempt_break_lock(sink, BreakLockAction::Enter).await);
            }
        }
    }
}

async fn attempt_break_lock(sink: &impl EffectSink, action: BreakLockAction) -> BreakLockOutcome {
    let result = match action {
        BreakLockAction::Enter => sink.enter_break_lock().await,
        BreakLockAction::Release => sink.release_break_lock().await,
    };
    BreakLockOutcome {
        action,
        status: if result.is_ok() {
            EffectStatus::Succeeded
        } else {
            EffectStatus::Failed
        },
    }
}

#[cfg(test)]
mod tests {
    use pomolice_core::settings::input::SettingChange;
    mod full_state;
    mod outcomes;

    use super::*;
    use crate::external_adapters::clock::testing::FixedClock;
    use crate::external_adapters::persistence::{PersistenceLoadOutcome, SaveError};
    use crate::storage::SettingsFileStatus;
    use chrono::{Local, TimeZone, Utc};
    use pomolice_core::types::{PostBreakDecisionAction, PostBreakTransitionPolicy, TimerStatus};
    use std::path::PathBuf;
    use std::sync::Mutex;
    use std::time::{SystemTime, UNIX_EPOCH};

    /// Records every emitted event with its payload and answers a break-lock
    /// request with a configured outcome.
    struct RecordingSink {
        emitted: Mutex<Vec<(String, serde_json::Value)>>,
        lock_outcome: EffectStatus,
        lock_takes: Option<std::time::Duration>,
        release_fails: bool,
        lock_actions: Mutex<Vec<BreakLockAction>>,
    }

    impl RecordingSink {
        fn with_lock_outcome(lock_outcome: EffectStatus) -> Self {
            Self {
                emitted: Mutex::new(Vec::new()),
                lock_outcome,
                lock_takes: None,
                release_fails: false,
                lock_actions: Mutex::new(Vec::new()),
            }
        }

        /// Makes entering the lock take real time, which is where a second
        /// dispatch would interleave if dispatches were not serialized.
        fn with_slow_lock(mut self, lock_takes: std::time::Duration) -> Self {
            self.lock_takes = Some(lock_takes);
            self
        }

        fn snapshot_statuses(&self) -> Vec<String> {
            self.emitted
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .iter()
                .filter(|(name, _)| name == "timer_snapshot_changed")
                .map(|(_, payload)| {
                    payload["timer"]["status"]
                        .as_str()
                        .unwrap_or("")
                        .to_string()
                })
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

        fn emitted_payloads(&self, event_name: &str) -> Vec<serde_json::Value> {
            self.emitted
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .iter()
                .filter(|(name, _)| name == event_name)
                .map(|(_, payload)| payload.clone())
                .collect()
        }

        fn last_snapshot_timer(&self) -> Option<serde_json::Value> {
            self.emitted
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .iter()
                .rev()
                .find(|(name, _)| name == "timer_snapshot_changed")
                .map(|(_, payload)| payload["timer"].clone())
        }
    }

    impl EffectSink for RecordingSink {
        fn notify(&self, notification: TimerNotification) {
            let (name, payload) = match notification {
                TimerNotification::SnapshotChanged(value) => (
                    "timer_snapshot_changed",
                    serde_json::to_value(value).expect("notification should serialize"),
                ),
                TimerNotification::SessionCompleted(value) => (
                    "session_completed",
                    serde_json::to_value(value).expect("notification should serialize"),
                ),
                TimerNotification::AutoRunStopPending => {
                    ("auto_run_stop_pending", serde_json::Value::Null)
                }
                TimerNotification::AutoRunStopped => ("auto_run_stopped", serde_json::Value::Null),
            };
            self.emitted
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .push((name.to_owned(), payload));
        }

        async fn enter_break_lock(&self) -> Result<()> {
            self.lock_actions
                .lock()
                .expect("lock actions")
                .push(BreakLockAction::Enter);
            if let Some(lock_takes) = self.lock_takes {
                tokio::time::sleep(lock_takes).await;
            }
            if self.lock_outcome == EffectStatus::Succeeded {
                Ok(())
            } else {
                Err(PomoliceError::WindowControl(
                    "injected entry failure".into(),
                ))
            }
        }

        async fn release_break_lock(&self) -> Result<()> {
            self.lock_actions
                .lock()
                .expect("lock actions")
                .push(BreakLockAction::Release);
            if self.release_fails {
                Err(PomoliceError::WindowControl(
                    "injected release failure".into(),
                ))
            } else {
                Ok(())
            }
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

    fn runtime_with(
        timer_state_persistence: PersistenceManager<PersistedState>,
        settings_persistence: PersistenceManager<PersistedSettings>,
        clock: &Arc<FixedClock>,
    ) -> TimerRuntime {
        let clock_trait: Arc<dyn Clock + Send + Sync> = Arc::<FixedClock>::clone(clock);
        TimerRuntime::new(
            TimerCore::restore(None, UserSettings::default(), clock.now_utc(), Local).0,
            clock_trait,
            timer_state_persistence,
            settings_persistence,
            SettingsFileStatus::Ok,
        )
    }

    /// An idle runtime whose files live in `data_dir`.
    fn idle_runtime(data_dir: &std::path::Path, clock: &Arc<FixedClock>) -> TimerRuntime {
        runtime_with(
            PersistenceManager::new(Some(data_dir.to_path_buf())),
            PersistenceManager::new(Some(data_dir.to_path_buf())),
            clock,
        )
    }

    fn one_second_work() -> SettingsChanges {
        SettingsChanges {
            work_duration_secs: SettingChange::Set(1),
            ..SettingsChanges::default()
        }
    }

    fn daytime_slot(enabled: bool) -> pomolice_core::settings::input::DaySlotInput {
        pomolice_core::settings::input::DaySlotInput {
            enabled,
            start_time: "09:00".to_string(),
            end_time: "18:00".to_string(),
        }
    }

    fn saved_state(runtime: &TimerRuntime) -> Option<PersistedState> {
        match runtime.timer_state_persistence.load_sync() {
            PersistenceLoadOutcome::Restored(persisted) => Some(persisted),
            PersistenceLoadOutcome::NoState | PersistenceLoadOutcome::Discarded => None,
        }
    }

    fn saved_settings(runtime: &TimerRuntime) -> Option<UserSettings> {
        match runtime.settings_persistence.load_sync() {
            PersistenceLoadOutcome::Restored(persisted) => Some(persisted.settings),
            PersistenceLoadOutcome::NoState | PersistenceLoadOutcome::Discarded => None,
        }
    }

    async fn start_default_work(runtime: &TimerRuntime, sink: &RecordingSink) {
        runtime
            .dispatch(sink, Event::StartWorkRequested)
            .await
            .expect("start should succeed");
    }

    async fn start_one_second_work(runtime: &TimerRuntime, sink: &RecordingSink) {
        runtime
            .update_settings(sink, one_second_work())
            .await
            .expect("settings save should succeed");
        start_default_work(runtime, sink).await;
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
        let runtime = runtime_with(persistence, PersistenceManager::new(None), &clock);
        let sink = RecordingSink::with_lock_outcome(EffectStatus::Succeeded);

        let result = runtime.dispatch(&sink, Event::StartWorkRequested).await;

        let outcome = result.expect("the input was accepted");
        assert_eq!(outcome.effects.last_timer_save, Some(EffectStatus::Failed));
        let last_tick = sink
            .last_snapshot_timer()
            .expect("a tick should be emitted");
        assert_eq!(last_tick["status"], "working");
    }

    #[tokio::test]
    async fn a_rejected_event_runs_no_effects() {
        let clock = fixed_clock();
        let temp_dir = create_temp_dir("rejected-event");
        let runtime = idle_runtime(&temp_dir, &clock);
        let sink = RecordingSink::with_lock_outcome(EffectStatus::Succeeded);

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
        let runtime = idle_runtime(&temp_dir, &clock);
        let sink = RecordingSink::with_lock_outcome(EffectStatus::Succeeded);
        start_one_second_work(&runtime, &sink).await;
        clock.set_now_local(clock.now_local() + chrono::Duration::seconds(1));

        runtime
            .dispatch(
                &sink,
                Event::TimeObserved {
                    credited_break_seconds: 1,
                },
            )
            .await
            .expect("tick should succeed");

        assert!(sink
            .emitted_names()
            .contains(&"session_completed".to_string()));
        let last_tick = sink
            .last_snapshot_timer()
            .expect("a tick should be emitted");
        assert_eq!(last_tick["status"], "on_break");
        assert_eq!(
            runtime
                .read_full_state()
                .await
                .effects
                .break_lock
                .last_attempt,
            Some(BreakLockOutcome {
                action: BreakLockAction::Enter,
                status: EffectStatus::Succeeded
            })
        );
        let saved = saved_state(&runtime)
            .expect("the break should have been saved by the time the tick returned");
        assert_eq!(saved.timer.status, TimerStatus::OnBreak);
    }

    #[tokio::test]
    async fn starting_work_is_saved_before_the_command_returns() {
        let clock = fixed_clock();
        let runtime = idle_runtime(&create_temp_dir("start-saved"), &clock);
        let sink = RecordingSink::with_lock_outcome(EffectStatus::Succeeded);

        start_default_work(&runtime, &sink).await;

        let saved = saved_state(&runtime).expect("the started session should be saved");
        assert_eq!(saved.timer.status, TimerStatus::Working);
    }

    #[tokio::test]
    async fn an_emergency_stop_is_saved_before_the_command_returns() {
        let clock = fixed_clock();
        let runtime = idle_runtime(&create_temp_dir("stop-saved"), &clock);
        let sink = RecordingSink::with_lock_outcome(EffectStatus::Succeeded);
        start_default_work(&runtime, &sink).await;

        runtime
            .dispatch(&sink, Event::EmergencyStopRequested)
            .await
            .expect("stop should succeed");

        let saved = saved_state(&runtime).expect("the stop should be saved");
        assert_eq!(saved.timer.status, TimerStatus::Idle);
    }

    #[tokio::test]
    async fn saved_settings_are_written_to_their_file_and_announced_as_an_event() {
        let clock = fixed_clock();
        let runtime = idle_runtime(&create_temp_dir("settings-saved"), &clock);
        let sink = RecordingSink::with_lock_outcome(EffectStatus::Succeeded);
        let settings = UserSettings {
            post_break_transition_policy: PostBreakTransitionPolicy::StartNextWorkAutomatically,
            ..UserSettings::default()
        };

        runtime
            .update_settings(
                &sink,
                SettingsChanges {
                    post_break_transition_policy: SettingChange::Set(
                        PostBreakTransitionPolicy::StartNextWorkAutomatically,
                    ),
                    ..SettingsChanges::default()
                },
            )
            .await
            .expect("settings save should succeed");

        assert_eq!(
            sink.emitted_payloads("timer_snapshot_changed")
                .iter()
                .map(|snapshot| snapshot["timer"]["settings"].clone())
                .collect::<Vec<_>>(),
            vec![serde_json::to_value(&settings).expect("settings should serialize")]
        );
        assert_eq!(saved_settings(&runtime), Some(settings));
    }

    #[tokio::test]
    async fn saving_settings_does_not_depend_on_the_timer_state_file() {
        let clock = fixed_clock();
        let failing_timer_state =
            PersistenceManager::new(Some(create_temp_dir("settings-without-timer-state")))
                .with_injected_save_error_hook(Arc::new(|| {
                    Some(SaveError::Write {
                        path: PathBuf::from("timer_state.json"),
                        source: std::io::Error::other("injected"),
                    })
                }));
        let runtime = runtime_with(
            failing_timer_state,
            PersistenceManager::new(Some(create_temp_dir("settings-with-own-file"))),
            &clock,
        );
        let sink = RecordingSink::with_lock_outcome(EffectStatus::Succeeded);
        let settings = UserSettings {
            auto_run_feature_enabled: true,
            ..UserSettings::default()
        };

        runtime
            .update_settings(
                &sink,
                SettingsChanges {
                    auto_run_feature_enabled: SettingChange::Set(true),
                    ..SettingsChanges::default()
                },
            )
            .await
            .expect("settings save should not touch the timer state file");

        assert_eq!(
            runtime.read_full_state().await.timer.state.settings,
            settings.clone().into()
        );
        assert_eq!(saved_settings(&runtime), Some(settings));
    }

    #[tokio::test]
    async fn a_settings_save_that_cannot_be_written_changes_nothing() {
        let clock = fixed_clock();
        let settings_persistence =
            PersistenceManager::new(Some(create_temp_dir("settings-save-failure")))
                .with_injected_save_error_hook(Arc::new(|| {
                    Some(SaveError::Write {
                        path: PathBuf::from("settings.json"),
                        source: std::io::Error::other("injected"),
                    })
                }));
        let runtime = runtime_with(PersistenceManager::new(None), settings_persistence, &clock);
        let sink = RecordingSink::with_lock_outcome(EffectStatus::Succeeded);

        let result = runtime.update_settings(&sink, one_second_work()).await;

        assert!(matches!(result, Err(PomoliceError::PersistenceSave(_))));
        assert!(sink.emitted_names().is_empty());
        assert_eq!(
            runtime.read_full_state().await.timer.state.settings,
            UserSettings::default().into()
        );
    }

    #[tokio::test]
    async fn overlapping_saves_of_different_settings_keep_both_changes() {
        let clock = fixed_clock();
        let runtime = idle_runtime(&create_temp_dir("overlapping-saves"), &clock);
        let sink = RecordingSink::with_lock_outcome(EffectStatus::Succeeded);

        let (muted, policy_changed) = tokio::join!(
            runtime.update_settings(
                &sink,
                SettingsChanges {
                    notification_sound_muted: SettingChange::Set(true),
                    ..SettingsChanges::default()
                },
            ),
            runtime.update_settings(
                &sink,
                SettingsChanges {
                    post_break_transition_policy: SettingChange::Set(
                        PostBreakTransitionPolicy::StartNextWorkAutomatically,
                    ),
                    ..SettingsChanges::default()
                },
            ),
        );
        muted.expect("mute save should succeed");
        policy_changed.expect("policy save should succeed");

        let expected = UserSettings {
            notification_sound_muted: true,
            post_break_transition_policy: PostBreakTransitionPolicy::StartNextWorkAutomatically,
            ..UserSettings::default()
        };
        assert_eq!(
            runtime.read_full_state().await.timer.state.settings,
            expected.clone().into()
        );
        assert_eq!(saved_settings(&runtime), Some(expected));
    }

    #[tokio::test]
    async fn the_later_save_of_the_same_setting_wins() {
        let clock = fixed_clock();
        let runtime = idle_runtime(&create_temp_dir("same-setting-twice"), &clock);
        let sink = RecordingSink::with_lock_outcome(EffectStatus::Succeeded);
        let muted = |value: bool| SettingsChanges {
            notification_sound_muted: SettingChange::Set(value),
            ..SettingsChanges::default()
        };

        runtime
            .update_settings(&sink, muted(true))
            .await
            .expect("first save should succeed");
        runtime
            .update_settings(&sink, muted(false))
            .await
            .expect("second save should succeed");

        assert!(
            !runtime
                .read_full_state()
                .await
                .timer
                .state
                .settings
                .notification_sound_muted
        );
        assert_eq!(
            saved_settings(&runtime).map(|settings| settings.notification_sound_muted),
            Some(false)
        );
    }

    #[tokio::test]
    async fn a_change_is_validated_together_with_the_settings_it_joins() {
        let clock = fixed_clock();
        let runtime = idle_runtime(&create_temp_dir("validated-as-a-whole"), &clock);
        let sink = RecordingSink::with_lock_outcome(EffectStatus::Succeeded);
        let mut tuesday_only = SettingsInput::from(UserSettings::default()).weekly_auto_run_config;
        tuesday_only.tue = daytime_slot(true);
        runtime
            .update_settings(
                &sink,
                SettingsChanges {
                    weekly_auto_run_config: SettingChange::Set(tuesday_only.clone()),
                    ..SettingsChanges::default()
                },
            )
            .await
            .expect("tuesday save should succeed");
        let mut monday_overnight = tuesday_only;
        monday_overnight.mon = pomolice_core::settings::input::DaySlotInput {
            enabled: true,
            start_time: "23:00".to_string(),
            end_time: "10:00".to_string(),
        };

        let result = runtime
            .update_settings(
                &sink,
                SettingsChanges {
                    weekly_auto_run_config: SettingChange::Set(monday_overnight),
                    ..SettingsChanges::default()
                },
            )
            .await;

        match result {
            Err(PomoliceError::InvalidSettings(errors)) => assert_eq!(
                errors.issues(),
                [
                    pomolice_core::settings::input::SettingsValidationIssue::OverlappingTimeRanges {
                        primary_weekday: pomolice_core::types::WeekdayKey::Mon,
                        secondary_weekday: pomolice_core::types::WeekdayKey::Tue,
                    }
                ]
            ),
            other => panic!("expected an overlap rejection, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn saving_no_changes_writes_nothing_and_announces_nothing() {
        let clock = fixed_clock();
        let runtime = idle_runtime(&create_temp_dir("no-changes"), &clock);
        let sink = RecordingSink::with_lock_outcome(EffectStatus::Succeeded);

        runtime
            .update_settings(&sink, SettingsChanges::default())
            .await
            .expect("an empty save should succeed");

        assert_eq!(
            runtime.read_full_state().await.timer.state.settings,
            UserSettings::default().into()
        );
        assert_eq!(saved_settings(&runtime), None);
        assert!(sink.emitted_names().is_empty());
    }

    #[tokio::test]
    async fn idle_ticks_do_not_write_to_disk() {
        let clock = fixed_clock();
        let runtime = idle_runtime(&create_temp_dir("idle-no-write"), &clock);
        let sink = RecordingSink::with_lock_outcome(EffectStatus::Succeeded);

        for _ in 0..3 {
            clock.set_now_local(clock.now_local() + chrono::Duration::seconds(1));
            runtime
                .dispatch(
                    &sink,
                    Event::TimeObserved {
                        credited_break_seconds: 1,
                    },
                )
                .await
                .expect("idle tick should succeed");
        }

        assert!(saved_state(&runtime).is_none());
    }

    #[tokio::test]
    async fn dispatches_do_not_interleave_even_while_an_effect_is_still_running() {
        let clock = fixed_clock();
        let runtime = idle_runtime(&create_temp_dir("serialized"), &clock);
        let sink = RecordingSink::with_lock_outcome(EffectStatus::Succeeded)
            .with_slow_lock(std::time::Duration::from_millis(50));
        start_one_second_work(&runtime, &sink).await;
        clock.set_now_local(clock.now_local() + chrono::Duration::seconds(1));

        // The tick is polled first and parks inside the slow break lock. If
        // the stop could run meanwhile, it would reset the state before the
        // lock outcome is applied, and the frontend would never be told about
        // the break, or would hear about it after the stop.
        let (tick, stop) = tokio::join!(
            runtime.dispatch(
                &sink,
                Event::TimeObserved {
                    credited_break_seconds: 1
                }
            ),
            runtime.dispatch(&sink, Event::EmergencyStopRequested),
        );

        tick.expect("tick should succeed");
        stop.expect("stop should succeed");
        let statuses = sink.snapshot_statuses();
        let break_reported = statuses
            .iter()
            .rposition(|status| status == "on_break")
            .expect("the stop overtook the break: dispatches interleaved");
        let stop_reported = statuses
            .iter()
            .rposition(|status| status == "idle")
            .expect("the stop should reach the frontend");
        assert!(
            break_reported < stop_reported,
            "the stop was reported before the break: dispatches interleaved"
        );
    }

    #[tokio::test]
    async fn exit_flush_saves_while_a_dispatch_is_still_running_its_effects() {
        let clock = fixed_clock();
        let runtime = idle_runtime(&create_temp_dir("exit-during-dispatch"), &clock);
        let sink = RecordingSink::with_lock_outcome(EffectStatus::Succeeded)
            .with_slow_lock(std::time::Duration::from_millis(50));
        start_one_second_work(&runtime, &sink).await;
        clock.set_now_local(clock.now_local() + chrono::Duration::seconds(1));

        // The tick is polled first and parks inside the slow break lock, so the
        // flush runs while that dispatch still holds the permit.
        let (tick, flushed) = tokio::join!(
            runtime.dispatch(
                &sink,
                Event::TimeObserved {
                    credited_break_seconds: 1
                }
            ),
            async { runtime.flush_sync_on_exit() }
        );

        flushed.expect("exit flush should not wait for the running dispatch");
        tick.expect("tick should succeed");
        let saved = saved_state(&runtime).expect("state should be saved");
        assert_eq!(saved.timer.status, TimerStatus::OnBreak);
    }
}
