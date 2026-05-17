// Timer commands using dependency injection

use std::future::Future;
use std::pin::Pin;
use std::sync::atomic::Ordering;

use chrono::{DateTime, Local};
use tauri::{Emitter, Manager, State, WebviewWindow};

use crate::commands::input_rejection::ObservedCommandArg;
use crate::error::{PomoliceError, Result};
use crate::logging_event::{log_info_event, log_warn_event, BackendLogEvent};
use crate::managers::weekly_schedule::{is_now_inside_schedule_window, ActiveWindow};
use crate::state::AppState;
use crate::types::{
    ActiveWindowSummaryPayload, BootPromptRuntimeState, FullStatePayload, LongBreakDurationSecs,
    LongBreakIntervalCount, PostBreakDecisionAction, PostBreakTransitionPolicy,
    ShortBreakDurationSecs, TimerSettings, TimerStartSource, TimerStatus, TimerTickPayload,
    WeeklyAutoRunConfig, WorkDurationSecs,
};

struct StartRuntimeSequence {
    old_task: Option<tokio::task::JoinHandle<()>>,
    tick_payload: TimerTickPayload,
    work_duration: u64,
}

type BootPromptStartAttempt = Option<StartRuntimeSequence>;

trait StartRuntimeEffectShell {
    fn emit_tick(&mut self, tick_payload: &TimerTickPayload);
    fn update_tray(&mut self, tick_payload: &TimerTickPayload);
    fn emit_work_started(&mut self, work_duration: u64);
    fn spawn_timer_task<'a>(
        &'a mut self,
        state: &'a AppState,
    ) -> Pin<Box<dyn Future<Output = ()> + Send + 'a>>;
    fn before_persist(&mut self) {}
}

struct AppHandleRuntimeEffectShell<'a> {
    app: &'a tauri::AppHandle,
}

impl StartRuntimeEffectShell for AppHandleRuntimeEffectShell<'_> {
    fn emit_tick(&mut self, tick_payload: &TimerTickPayload) {
        let _ = self.app.emit("timer_tick", tick_payload);
    }

    fn update_tray(&mut self, tick_payload: &TimerTickPayload) {
        crate::tray::update_tray_icon(self.app, tick_payload);
    }

    fn emit_work_started(&mut self, work_duration: u64) {
        let _ = self.app.emit(
            "work_started",
            serde_json::json!({ "duration": work_duration }),
        );
    }

    fn spawn_timer_task<'a>(
        &'a mut self,
        _state: &'a AppState,
    ) -> Pin<Box<dyn Future<Output = ()> + Send + 'a>> {
        Box::pin(async move {
            crate::session_pipeline::spawn_timer_task(self.app).await;
        })
    }
}

/// Start a work session with settings
#[tauri::command]
pub(crate) async fn start_work(
    window: WebviewWindow,
    state: State<'_, AppState>,
    work_duration_secs: ObservedCommandArg<WorkDurationSecs>,
    short_break_duration_secs: ObservedCommandArg<ShortBreakDurationSecs>,
    long_break_duration_secs: ObservedCommandArg<LongBreakDurationSecs>,
    long_break_interval: ObservedCommandArg<LongBreakIntervalCount>,
    post_break_transition_policy: ObservedCommandArg<PostBreakTransitionPolicy>,
) -> Result<()> {
    log_info_event(
        BackendLogEvent::TimerCommandInvoked,
        &[
            ("command_name", "start_work".to_string()),
            ("window_label", window.label().to_string()),
        ],
    );
    authorize_main_window(&window, "start_work")?;
    let work_duration_secs = work_duration_secs.into_inner();
    let short_break_duration_secs = short_break_duration_secs.into_inner();
    let long_break_duration_secs = long_break_duration_secs.into_inner();
    let long_break_interval = long_break_interval.into_inner();
    let post_break_transition_policy = post_break_transition_policy.into_inner();

    let settings = TimerSettings::new(
        work_duration_secs,
        short_break_duration_secs,
        long_break_duration_secs,
        long_break_interval,
        post_break_transition_policy,
    );
    let now_local = state.clock.now_local();

    let (auto_run_feature_enabled, weekly_auto_run_config) = {
        let timer = state.timer_manager.lock().await;
        let timer_state = timer.get_state();
        (
            timer_state.auto_run_feature_enabled,
            timer_state.weekly_auto_run_config.clone(),
        )
    };
    let manual_start_source =
        determine_manual_start_source(now_local, auto_run_feature_enabled, &weekly_auto_run_config);

    let app = window.app_handle();
    start_work_with_runtime_effects(&state, app, settings, manual_start_source).await
}

/// Resolve manual post-break decision from continue dialog.
///
/// This command is valid only while `post_break_decision_pending = true`.
#[tauri::command]
pub(crate) async fn resolve_post_break_decision(
    window: WebviewWindow,
    state: State<'_, AppState>,
    action: ObservedCommandArg<PostBreakDecisionAction>,
) -> Result<()> {
    authorize_main_window(&window, "resolve_post_break_decision")?;
    let action = action.into_inner();

    let (old_task, tick_payload, should_spawn_timer_task) = {
        let mut timer = state.timer_manager.lock().await;
        validate_post_break_decision_command_state(
            timer.status(),
            timer.get_state().post_break_decision_pending,
        )?;
        let tick_payload = timer.resolve_post_break_decision(action)?;
        let should_spawn_timer_task = action == PostBreakDecisionAction::Continue;
        let old_task = if should_spawn_timer_task {
            // Take/cancel under lock, but never await under lock.
            timer.cancel_and_take_task()
        } else {
            None
        };
        (old_task, tick_payload, should_spawn_timer_task)
    };

    let app = window.app_handle();
    let _ = app.emit("timer_tick", &tick_payload);
    crate::tray::update_tray_icon(app, &tick_payload);

    if let Some(handle) = old_task {
        // Await outside the lock to avoid lock-held await deadlocks.
        let _ = handle.await;
    }

    if should_spawn_timer_task {
        // Continue action always starts a fresh timer task.
        crate::session_pipeline::spawn_timer_task(app).await;
    }

    state.persist_current_state_async().await?;

    Ok(())
}

/// Emergency stop - reset all state
#[tauri::command]
pub(crate) async fn emergency_stop(
    window: WebviewWindow,
    state: State<'_, AppState>,
) -> Result<()> {
    log_info_event(
        BackendLogEvent::TimerCommandInvoked,
        &[
            ("command_name", "emergency_stop".to_string()),
            ("window_label", window.label().to_string()),
        ],
    );
    authorize_main_window(&window, "emergency_stop")?;

    let (was_fullscreen_break, tick_payload, old_task) = {
        let mut timer = state.timer_manager.lock().await;
        // Take/cancel under the lock, but never await under the lock.
        let old = timer.cancel_and_take_task();
        let (was_fullscreen, payload) = timer.emergency_stop();
        (was_fullscreen, payload, old)
    };

    let _ = window.app_handle().emit("timer_tick", &tick_payload);
    crate::tray::update_tray_icon(window.app_handle(), &tick_payload);

    if let Some(handle) = old_task {
        // Await outside the lock to avoid lock-held await deadlocks.
        let _ = handle.await;
    }

    // Exit fullscreen if needed
    if was_fullscreen_break {
        let app_handle = window.app_handle().clone();

        tauri::async_runtime::spawn(async move {
            crate::session_pipeline::lib_handle_external_break_exit(app_handle).await;
        });
    }

    state.persist_current_state_async().await?;

    Ok(())
}

/// Sync timer duration settings from frontend (idle only)
#[tauri::command]
pub(crate) async fn sync_timer_duration_settings(
    window: WebviewWindow,
    state: State<'_, AppState>,
    work_duration_secs: ObservedCommandArg<WorkDurationSecs>,
    short_break_duration_secs: ObservedCommandArg<ShortBreakDurationSecs>,
    long_break_duration_secs: ObservedCommandArg<LongBreakDurationSecs>,
    long_break_interval: ObservedCommandArg<LongBreakIntervalCount>,
) -> Result<()> {
    authorize_main_window(&window, "sync_timer_duration_settings")?;
    let work_duration_secs = work_duration_secs.into_inner();
    let short_break_duration_secs = short_break_duration_secs.into_inner();
    let long_break_duration_secs = long_break_duration_secs.into_inner();
    let long_break_interval = long_break_interval.into_inner();

    let tick_payload = {
        let mut timer = state.timer_manager.lock().await;
        timer.sync_timer_duration_settings(
            work_duration_secs,
            short_break_duration_secs,
            long_break_duration_secs,
            long_break_interval,
        )?
    };

    let _ = window.app_handle().emit("timer_tick", &tick_payload);
    crate::tray::update_tray_icon(window.app_handle(), &tick_payload);

    state.persist_current_state_async().await?;

    Ok(())
}

/// Sync automation policies from frontend
#[tauri::command]
pub(crate) async fn sync_automation_policies(
    window: WebviewWindow,
    state: State<'_, AppState>,
    auto_run_feature_enabled: ObservedCommandArg<bool>,
    weekly_auto_run_config: ObservedCommandArg<WeeklyAutoRunConfig>,
    post_break_transition_policy: ObservedCommandArg<PostBreakTransitionPolicy>,
) -> Result<()> {
    authorize_main_window(&window, "sync_automation_policies")?;
    let auto_run_feature_enabled = auto_run_feature_enabled.into_inner();
    let weekly_auto_run_config = weekly_auto_run_config.into_inner();
    let post_break_transition_policy = post_break_transition_policy.into_inner();

    let (tick_payload, should_bump_config_revision) = {
        let mut timer = state.timer_manager.lock().await;
        let previous_auto_run_feature_enabled = timer.get_state().auto_run_feature_enabled;
        let previous_weekly_auto_run_config = timer.get_state().weekly_auto_run_config.clone();
        let should_bump_config_revision = did_scheduler_topology_change(
            previous_auto_run_feature_enabled,
            &previous_weekly_auto_run_config,
            auto_run_feature_enabled,
            &weekly_auto_run_config,
        );

        let tick_payload = timer.sync_automation_policies(
            auto_run_feature_enabled,
            weekly_auto_run_config,
            post_break_transition_policy,
        );

        (tick_payload, should_bump_config_revision)
    };

    if should_bump_config_revision {
        state.config_revision.fetch_add(1, Ordering::AcqRel);
    }

    let _ = window.app_handle().emit("timer_tick", &tick_payload);
    crate::tray::update_tray_icon(window.app_handle(), &tick_payload);

    state.persist_current_state_async().await?;

    Ok(())
}

pub(crate) fn did_scheduler_topology_change(
    previous_auto_run_feature_enabled: bool,
    previous_weekly_auto_run_config: &WeeklyAutoRunConfig,
    next_auto_run_feature_enabled: bool,
    next_weekly_auto_run_config: &WeeklyAutoRunConfig,
) -> bool {
    previous_auto_run_feature_enabled != next_auto_run_feature_enabled
        || previous_weekly_auto_run_config != next_weekly_auto_run_config
}

fn authorize_main_window_label(window_label: &str) -> Result<()> {
    if window_label == "main" {
        return Ok(());
    }

    Err(PomoliceError::Unauthorized(window_label.to_string()))
}

fn authorize_main_window(window: &WebviewWindow, command_name: &'static str) -> Result<()> {
    let window_label = window.label();
    if let Err(error) = authorize_main_window_label(window_label) {
        log_warn_event(
            BackendLogEvent::TimerCommandUnauthorized,
            &[
                ("command_name", command_name.to_string()),
                ("window_label", window_label.to_string()),
            ],
        );
        return Err(error);
    }

    Ok(())
}

fn to_active_window_summary_payload(active_window: &ActiveWindow) -> ActiveWindowSummaryPayload {
    ActiveWindowSummaryPayload {
        weekday_key: active_window.slot_key.clone(),
        start_time: active_window.start_time.clone(),
        end_time: active_window.end_time.clone(),
        crosses_midnight: active_window.crosses_midnight,
    }
}

fn determine_manual_start_source(
    now_local: DateTime<Local>,
    auto_run_feature_enabled: bool,
    weekly_auto_run_config: &WeeklyAutoRunConfig,
) -> TimerStartSource {
    if auto_run_feature_enabled
        && is_now_inside_schedule_window(now_local, weekly_auto_run_config).is_some()
    {
        TimerStartSource::ManualInsideWindow
    } else {
        TimerStartSource::ManualOutsideWindow
    }
}

fn validate_post_break_decision_command_state(
    timer_status: TimerStatus,
    post_break_decision_pending: bool,
) -> Result<()> {
    if timer_status == TimerStatus::Idle && post_break_decision_pending {
        return Ok(());
    }

    Err(PomoliceError::InvalidStateTransition {
        action: "resolve post-break decision".to_string(),
        current_state: timer_status,
    })
}

async fn start_work_with_runtime_effects(
    state: &AppState,
    app: &tauri::AppHandle,
    settings: TimerSettings,
    manual_start_source: TimerStartSource,
) -> Result<()> {
    let start_sequence =
        prepare_manual_start_sequence(state, settings, manual_start_source).await?;

    let mut runtime_effect_shell = AppHandleRuntimeEffectShell { app };
    apply_start_runtime_effects(state, &mut runtime_effect_shell, start_sequence, |_| {}).await
}

async fn resolve_boot_auto_start_prompt_start(
    state: &AppState,
    app: &tauri::AppHandle,
    timer_settings: TimerSettings,
) -> Result<BootPromptRuntimeState> {
    let start_attempt = prepare_boot_prompt_start(state, timer_settings).await?;

    let Some(start_sequence) = start_attempt else {
        mark_boot_prompt_not_applicable(state);
        return Ok(BootPromptRuntimeState::NotApplicable);
    };

    let mut runtime_effect_shell = AppHandleRuntimeEffectShell { app };
    apply_start_runtime_effects(
        state,
        &mut runtime_effect_shell,
        start_sequence,
        mark_boot_prompt_resolved_start,
    )
    .await?;
    Ok(BootPromptRuntimeState::ResolvedStart)
}

async fn prepare_boot_prompt_start(
    state: &AppState,
    timer_settings: TimerSettings,
) -> Result<BootPromptStartAttempt> {
    let start_attempt = {
        let mut timer_manager = state.timer_manager.lock().await;
        // Boot-prompt resolution must attempt the state transition before cancelling any task.
        // If another path already started a replacement session, InvalidStateTransition degrades
        // to NotApplicable instead of tearing down that already-running task.
        match timer_manager
            .start_work_with_source(timer_settings, TimerStartSource::ManualInsideWindow)
        {
            Ok((payload, duration)) => Ok(Some(StartRuntimeSequence {
                old_task: timer_manager.cancel_and_take_task(),
                tick_payload: payload,
                work_duration: duration,
            })),
            Err(PomoliceError::InvalidStateTransition { .. }) => Ok(None),
            Err(err) => Err(err),
        }
    }?;

    Ok(start_attempt)
}

async fn prepare_manual_start_sequence(
    state: &AppState,
    settings: TimerSettings,
    manual_start_source: TimerStartSource,
) -> Result<StartRuntimeSequence> {
    // Manual start intentionally stays on the replacement flow: cancel first, then transition.
    // Boot-prompt resolution uses the opposite order because it must degrade an already-started
    // replacement session to NotApplicable without tearing that session back down.
    let start_sequence = {
        let mut timer = state.timer_manager.lock().await;
        let old_task = timer.cancel_and_take_task();
        let (tick_payload, work_duration) =
            timer.start_work_with_source(settings, manual_start_source)?;
        StartRuntimeSequence {
            old_task,
            tick_payload,
            work_duration,
        }
    };

    Ok(start_sequence)
}

async fn apply_start_runtime_effects<S, F>(
    state: &AppState,
    runtime_effect_shell: &mut S,
    start_sequence: StartRuntimeSequence,
    before_persist: F,
) -> Result<()>
where
    S: StartRuntimeEffectShell,
    F: FnOnce(&AppState),
{
    runtime_effect_shell.emit_tick(&start_sequence.tick_payload);
    runtime_effect_shell.update_tray(&start_sequence.tick_payload);
    runtime_effect_shell.emit_work_started(start_sequence.work_duration);

    // Await the old task outside the lock so the old tick loop cannot overlap
    // with the replacement session after runtime effects have been emitted.
    if let Some(handle) = start_sequence.old_task {
        let _ = handle.await;
    }

    runtime_effect_shell.spawn_timer_task(state).await;
    before_persist(state);
    runtime_effect_shell.before_persist();
    state.persist_current_state_async().await?;
    Ok(())
}

fn mark_boot_prompt_not_applicable(state: &AppState) {
    if let Ok(mut runtime_state) = state.boot_prompt_runtime_state.lock() {
        if *runtime_state == BootPromptRuntimeState::AwaitingDecision {
            *runtime_state = BootPromptRuntimeState::NotApplicable;
        }
    }
    state
        .auto_start_boundary_enabled
        .store(true, Ordering::Release);
}

fn mark_boot_prompt_resolved_start(state: &AppState) {
    if let Ok(mut runtime_state) = state.boot_prompt_runtime_state.lock() {
        *runtime_state = BootPromptRuntimeState::ResolvedStart;
    }
    state
        .auto_start_boundary_enabled
        .store(true, Ordering::Release);
}

#[cfg(test)]
async fn start_work_for_tests(
    state: &AppState,
    settings: TimerSettings,
    manual_start_source: TimerStartSource,
    runtime_effect_shell: &mut impl StartRuntimeEffectShell,
) -> Result<()> {
    let start_sequence =
        prepare_manual_start_sequence(state, settings, manual_start_source).await?;

    apply_start_runtime_effects(state, runtime_effect_shell, start_sequence, |_| {}).await
}

#[cfg(test)]
async fn resolve_boot_auto_start_prompt_start_for_tests(
    state: &AppState,
    timer_settings: TimerSettings,
    runtime_effect_shell: &mut impl StartRuntimeEffectShell,
) -> Result<BootPromptRuntimeState> {
    let start_attempt = prepare_boot_prompt_start(state, timer_settings).await?;

    let Some(start_sequence) = start_attempt else {
        mark_boot_prompt_not_applicable(state);
        return Ok(BootPromptRuntimeState::NotApplicable);
    };

    apply_start_runtime_effects(
        state,
        runtime_effect_shell,
        start_sequence,
        mark_boot_prompt_resolved_start,
    )
    .await?;
    Ok(BootPromptRuntimeState::ResolvedStart)
}

fn evaluate_initial_boot_prompt_decision(
    is_first_get_full_state: bool,
    timer_status: TimerStatus,
    is_inside_schedule_window: bool,
) -> (bool, Option<BootPromptRuntimeState>, bool) {
    if !is_first_get_full_state {
        return (false, None, false);
    }

    if timer_status != TimerStatus::Idle {
        return (false, Some(BootPromptRuntimeState::NotApplicable), true);
    }

    if is_inside_schedule_window {
        return (true, Some(BootPromptRuntimeState::AwaitingDecision), false);
    }

    (false, Some(BootPromptRuntimeState::NotApplicable), true)
}

/// Get full timer state for initial sync
#[tauri::command]
pub(crate) async fn get_full_state(
    window: WebviewWindow,
    state: State<'_, AppState>,
) -> Result<FullStatePayload> {
    authorize_main_window(&window, "get_full_state")?;

    let now_local = state.clock.now_local();
    let is_first_get_full_state = state
        .boot_prompt_check_pending
        .swap(false, Ordering::AcqRel);

    let (mut full_state_payload, timer_status, auto_run_feature_enabled, weekly_auto_run_config) = {
        let timer_manager = state.timer_manager.lock().await;
        let timer_state = timer_manager.get_state();
        (
            timer_manager.get_full_state(),
            timer_manager.status(),
            timer_state.auto_run_feature_enabled,
            timer_state.weekly_auto_run_config.clone(),
        )
    };

    let active_window = if auto_run_feature_enabled {
        is_now_inside_schedule_window(now_local, &weekly_auto_run_config)
    } else {
        None
    };

    full_state_payload.active_window_summary =
        active_window.as_ref().map(to_active_window_summary_payload);

    let (should_prompt_start_on_boot, runtime_state_transition, should_enable_auto_start) =
        evaluate_initial_boot_prompt_decision(
            is_first_get_full_state,
            timer_status,
            active_window.is_some(),
        );

    if let Some(next_runtime_state) = runtime_state_transition {
        if let Ok(mut boot_prompt_runtime_state) = state.boot_prompt_runtime_state.lock() {
            *boot_prompt_runtime_state = next_runtime_state;
        }
    }

    if should_enable_auto_start {
        state
            .auto_start_boundary_enabled
            .store(true, Ordering::Release);
    }

    full_state_payload.should_prompt_start_on_boot = should_prompt_start_on_boot;
    Ok(full_state_payload)
}

/// Resolve startup auto-start confirmation prompt.
#[tauri::command]
pub(crate) async fn resolve_boot_auto_start_prompt(
    window: WebviewWindow,
    state: State<'_, AppState>,
    start_now: ObservedCommandArg<bool>,
) -> Result<BootPromptRuntimeState> {
    authorize_main_window(&window, "resolve_boot_auto_start_prompt")?;
    let start_now = start_now.into_inner();

    let current_runtime_state = match state.boot_prompt_runtime_state.lock() {
        Ok(runtime_state) => *runtime_state,
        Err(_) => BootPromptRuntimeState::NotApplicable,
    };

    let can_resolve_prompt = {
        let timer_manager = state.timer_manager.lock().await;
        timer_manager.status() == TimerStatus::Idle
            && current_runtime_state == BootPromptRuntimeState::AwaitingDecision
    };

    if !can_resolve_prompt {
        if current_runtime_state == BootPromptRuntimeState::AwaitingDecision {
            if let Ok(mut runtime_state) = state.boot_prompt_runtime_state.lock() {
                *runtime_state = BootPromptRuntimeState::NotApplicable;
            }
            state
                .auto_start_boundary_enabled
                .store(true, Ordering::Release);
        }
        return Ok(BootPromptRuntimeState::NotApplicable);
    }

    let timer_settings = {
        let timer_manager = state.timer_manager.lock().await;
        timer_manager.get_state().settings.clone()
    };

    if start_now {
        resolve_boot_auto_start_prompt_start(&state, window.app_handle(), timer_settings).await
    } else {
        if let Ok(mut runtime_state) = state.boot_prompt_runtime_state.lock() {
            *runtime_state = BootPromptRuntimeState::ResolvedLater;
        }
        state
            .auto_start_boundary_enabled
            .store(true, Ordering::Release);
        Ok(BootPromptRuntimeState::ResolvedLater)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::managers::clock::testing::FixedClock;
    use crate::managers::clock::Clock;
    use crate::managers::persistence::PersistenceManager;
    use chrono::Utc;
    use std::path::{Path, PathBuf};
    use std::sync::atomic::AtomicBool as StdAtomicBool;
    use std::sync::{Arc, Mutex as StdMutex};
    use std::time::Duration as StdDuration;
    use std::time::{SystemTime, UNIX_EPOCH};

    use crate::types::{DayAutoRunSlot, HhMm};
    use chrono::TimeZone;
    use serde_json::json;

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    enum RuntimeEffectEvent {
        EmitTick,
        UpdateTray,
        EmitWorkStarted,
        SpawnTimerTask,
        Persist,
    }

    #[derive(Default)]
    struct RecordingRuntimeEffectShell {
        events: Vec<RuntimeEffectEvent>,
    }

    impl RecordingRuntimeEffectShell {
        fn events(&self) -> &[RuntimeEffectEvent] {
            &self.events
        }
    }

    impl StartRuntimeEffectShell for RecordingRuntimeEffectShell {
        fn emit_tick(&mut self, _tick_payload: &TimerTickPayload) {
            self.events.push(RuntimeEffectEvent::EmitTick);
        }

        fn update_tray(&mut self, _tick_payload: &TimerTickPayload) {
            self.events.push(RuntimeEffectEvent::UpdateTray);
        }

        fn emit_work_started(&mut self, _work_duration: u64) {
            self.events.push(RuntimeEffectEvent::EmitWorkStarted);
        }

        fn spawn_timer_task<'a>(
            &'a mut self,
            _state: &'a AppState,
        ) -> Pin<Box<dyn Future<Output = ()> + Send + 'a>> {
            self.events.push(RuntimeEffectEvent::SpawnTimerTask);
            Box::pin(async {})
        }

        fn before_persist(&mut self) {
            self.events.push(RuntimeEffectEvent::Persist);
        }
    }

    fn create_temp_dir(test_name: &str) -> PathBuf {
        let timestamp_nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time should be after unix epoch")
            .as_nanos();

        let path = std::env::temp_dir().join(format!(
            "pomolice-timer-command-{}-{}-{}",
            test_name,
            std::process::id(),
            timestamp_nanos
        ));
        std::fs::create_dir_all(&path).expect("temp directory should be created");
        path
    }

    fn cleanup_temp_dir(path: &Path) {
        let _ = std::fs::remove_dir_all(path);
    }

    fn create_utc_datetime(
        year: i32,
        month: u32,
        day: u32,
        hour: u32,
        minute: u32,
        second: u32,
    ) -> chrono::DateTime<Utc> {
        Utc.with_ymd_and_hms(year, month, day, hour, minute, second)
            .single()
            .expect("valid UTC datetime")
    }

    fn fixed_clock(now_utc: chrono::DateTime<Utc>) -> Arc<FixedClock> {
        Arc::new(FixedClock::new(now_utc.with_timezone(&Local)))
    }

    fn as_clock_trait(clock: &Arc<FixedClock>) -> Arc<dyn Clock + Send + Sync> {
        Arc::<FixedClock>::clone(clock) as Arc<dyn Clock + Send + Sync>
    }

    async fn build_test_state_with_persistence_dir(app_data_dir: PathBuf) -> AppState {
        let clock = fixed_clock(create_utc_datetime(2025, 2, 1, 9, 0, 0));
        let persistence = PersistenceManager::new(as_clock_trait(&clock), Some(app_data_dir));
        let (app_state, _) =
            AppState::build_startup_state_for_tests(as_clock_trait(&clock), persistence).await;
        app_state
    }

    async fn build_test_state_without_persistence_path() -> AppState {
        let clock = fixed_clock(create_utc_datetime(2025, 2, 1, 9, 0, 0));
        let persistence = PersistenceManager::new(as_clock_trait(&clock), None);
        let (app_state, _) =
            AppState::build_startup_state_for_tests(as_clock_trait(&clock), persistence).await;
        app_state
    }

    async fn wait_for_transition_event_count(
        events: &StdMutex<Vec<&'static str>>,
        expected_count: usize,
        context: &str,
    ) {
        for _ in 0..100 {
            let current_count = events
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .len();
            if current_count >= expected_count {
                return;
            }
            tokio::time::sleep(StdDuration::from_millis(10)).await;
        }

        panic!("timed out waiting for {context}");
    }

    async fn run_emergency_stop_exit_uses_break_mode_transition_coordinator_for_tests(
        state: Arc<AppState>,
        events: Arc<StdMutex<Vec<&'static str>>>,
        started_label: &'static str,
        completed_label: &'static str,
        release: Arc<StdAtomicBool>,
    ) {
        state
            .run_break_mode_transition(async move {
                {
                    let mut events = events
                        .lock()
                        .unwrap_or_else(std::sync::PoisonError::into_inner);
                    events.push(started_label);
                }

                while !release.load(Ordering::Acquire) {
                    tokio::time::sleep(StdDuration::from_millis(10)).await;
                }

                let mut events = events
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner);
                events.push(completed_label);
            })
            .await;
    }

    fn enabled_slot(start_time: &str, end_time: &str) -> DayAutoRunSlot {
        DayAutoRunSlot::try_new(
            true,
            HhMm::try_from(start_time.to_string()).expect("start time should be valid"),
            HhMm::try_from(end_time.to_string()).expect("end time should be valid"),
        )
        .expect("slot should be valid")
    }

    fn monday_config(start_time: &str, end_time: &str) -> WeeklyAutoRunConfig {
        WeeklyAutoRunConfig::try_new(
            enabled_slot(start_time, end_time),
            DayAutoRunSlot::default(),
            DayAutoRunSlot::default(),
            DayAutoRunSlot::default(),
            DayAutoRunSlot::default(),
            DayAutoRunSlot::default(),
            DayAutoRunSlot::default(),
        )
        .expect("weekly config should be valid")
    }

    fn make_enabled_daytime_monday_config() -> WeeklyAutoRunConfig {
        monday_config("09:00", "18:00")
    }

    #[test]
    fn authorize_main_window_label_accepts_main() {
        assert!(authorize_main_window_label("main").is_ok());
    }

    #[test]
    fn authorize_main_window_label_rejects_break_overlay_window() {
        let result = authorize_main_window_label("break-overlay-1");

        assert!(matches!(
            result,
            Err(PomoliceError::Unauthorized(window_label)) if window_label == "break-overlay-1"
        ));
    }

    #[test]
    fn authorize_main_window_label_rejects_non_main_window() {
        let result = authorize_main_window_label("settings");

        assert!(matches!(
            result,
            Err(PomoliceError::Unauthorized(window_label)) if window_label == "settings"
        ));
    }

    #[test]
    fn test_determine_manual_start_source_returns_inside_when_enabled_and_in_window() {
        let now_local = Local
            .with_ymd_and_hms(2025, 1, 6, 10, 0, 0)
            .single()
            .expect("local datetime should be valid");
        let weekly_auto_run_config = make_enabled_daytime_monday_config();

        let source = determine_manual_start_source(now_local, true, &weekly_auto_run_config);

        assert_eq!(source, TimerStartSource::ManualInsideWindow);
    }

    #[test]
    fn test_determine_manual_start_source_returns_outside_when_enabled_and_out_of_window() {
        let now_local = Local
            .with_ymd_and_hms(2025, 1, 6, 8, 0, 0)
            .single()
            .expect("local datetime should be valid");
        let weekly_auto_run_config = make_enabled_daytime_monday_config();

        let source = determine_manual_start_source(now_local, true, &weekly_auto_run_config);

        assert_eq!(source, TimerStartSource::ManualOutsideWindow);
    }

    #[test]
    fn test_determine_manual_start_source_returns_outside_when_feature_disabled() {
        let now_local = Local
            .with_ymd_and_hms(2025, 1, 6, 10, 0, 0)
            .single()
            .expect("local datetime should be valid");
        let weekly_auto_run_config = make_enabled_daytime_monday_config();

        let source = determine_manual_start_source(now_local, false, &weekly_auto_run_config);

        assert_eq!(source, TimerStartSource::ManualOutsideWindow);
    }

    #[test]
    fn test_evaluate_initial_boot_prompt_decision_returns_awaiting_on_first_idle_inside_window() {
        let result = evaluate_initial_boot_prompt_decision(true, TimerStatus::Idle, true);
        assert_eq!(
            result,
            (true, Some(BootPromptRuntimeState::AwaitingDecision), false)
        );
    }

    #[test]
    fn test_evaluate_initial_boot_prompt_decision_returns_not_applicable_on_first_non_idle() {
        let result = evaluate_initial_boot_prompt_decision(true, TimerStatus::Working, true);
        assert_eq!(
            result,
            (false, Some(BootPromptRuntimeState::NotApplicable), true)
        );
    }

    #[test]
    fn test_evaluate_initial_boot_prompt_decision_returns_not_applicable_on_first_idle_outside_window(
    ) {
        let result = evaluate_initial_boot_prompt_decision(true, TimerStatus::Idle, false);
        assert_eq!(
            result,
            (false, Some(BootPromptRuntimeState::NotApplicable), true)
        );
    }

    #[test]
    fn test_evaluate_initial_boot_prompt_decision_returns_no_transition_after_first_call() {
        let result = evaluate_initial_boot_prompt_decision(false, TimerStatus::Idle, true);
        assert_eq!(result, (false, None, false));
    }

    #[test]
    fn test_evaluate_initial_boot_prompt_decision_prompts_only_on_first_call() {
        let first_result = evaluate_initial_boot_prompt_decision(true, TimerStatus::Idle, true);
        let second_result = evaluate_initial_boot_prompt_decision(false, TimerStatus::Idle, true);

        assert_eq!(
            first_result,
            (true, Some(BootPromptRuntimeState::AwaitingDecision), false)
        );
        assert_eq!(second_result, (false, None, false));
    }

    #[test]
    fn test_work_duration_secs_boundary_deserialize_rejects_zero() {
        let result = serde_json::from_value::<WorkDurationSecs>(json!(0));
        assert!(result.is_err());
    }

    #[test]
    fn test_long_break_interval_boundary_deserialize_rejects_zero() {
        let result = serde_json::from_value::<LongBreakIntervalCount>(json!(0));
        assert!(result.is_err());
    }

    #[test]
    fn test_day_auto_run_slot_boundary_deserialize_rejects_equal_start_and_end() {
        let result = serde_json::from_value::<DayAutoRunSlot>(json!({
            "enabled": true,
            "startTime": "09:00",
            "endTime": "09:00"
        }));
        assert!(result.is_err());
    }

    #[test]
    fn test_weekly_auto_run_config_boundary_deserialize_rejects_overlap() {
        let result = serde_json::from_value::<WeeklyAutoRunConfig>(json!({
            "mon": {"enabled": true, "startTime": "22:00", "endTime": "02:00"},
            "tue": {"enabled": true, "startTime": "01:00", "endTime": "05:00"},
            "wed": {"enabled": false, "startTime": "09:00", "endTime": "18:00"},
            "thu": {"enabled": false, "startTime": "09:00", "endTime": "18:00"},
            "fri": {"enabled": false, "startTime": "09:00", "endTime": "18:00"},
            "sat": {"enabled": false, "startTime": "09:00", "endTime": "18:00"},
            "sun": {"enabled": false, "startTime": "09:00", "endTime": "18:00"}
        }));
        assert!(result.is_err());
    }

    #[test]
    fn test_weekly_auto_run_config_boundary_deserialize_rejects_invalid_start_time() {
        let result = serde_json::from_value::<WeeklyAutoRunConfig>(json!({
            "mon": {"enabled": true, "startTime": "24:00", "endTime": "02:00"},
            "tue": {"enabled": false, "startTime": "09:00", "endTime": "18:00"},
            "wed": {"enabled": false, "startTime": "09:00", "endTime": "18:00"},
            "thu": {"enabled": false, "startTime": "09:00", "endTime": "18:00"},
            "fri": {"enabled": false, "startTime": "09:00", "endTime": "18:00"},
            "sat": {"enabled": false, "startTime": "09:00", "endTime": "18:00"},
            "sun": {"enabled": false, "startTime": "09:00", "endTime": "18:00"}
        }));
        assert!(result.is_err());
    }

    #[test]
    fn test_validate_post_break_decision_command_state_accepts_idle_pending_state() {
        let result = validate_post_break_decision_command_state(TimerStatus::Idle, true);
        assert!(result.is_ok());
    }

    #[test]
    fn test_validate_post_break_decision_command_state_rejects_idle_without_pending() {
        let result = validate_post_break_decision_command_state(TimerStatus::Idle, false);
        assert!(matches!(
            result,
            Err(PomoliceError::InvalidStateTransition { .. })
        ));
    }

    #[test]
    fn test_validate_post_break_decision_command_state_rejects_non_idle_even_when_pending() {
        let result = validate_post_break_decision_command_state(TimerStatus::Working, true);
        assert!(matches!(
            result,
            Err(PomoliceError::InvalidStateTransition { .. })
        ));
    }

    #[test]
    fn test_did_scheduler_topology_change_returns_false_when_schedule_fields_are_identical() {
        let previous_config = make_enabled_daytime_monday_config();
        let next_config = previous_config.clone();

        let changed = did_scheduler_topology_change(true, &previous_config, true, &next_config);

        assert!(!changed);
    }

    #[test]
    fn test_did_scheduler_topology_change_returns_true_when_auto_run_feature_enabled_changes() {
        let previous_config = make_enabled_daytime_monday_config();
        let next_config = previous_config.clone();

        let changed = did_scheduler_topology_change(false, &previous_config, true, &next_config);

        assert!(changed);
    }

    #[test]
    fn test_did_scheduler_topology_change_returns_true_when_weekly_auto_run_config_changes() {
        let previous_config = make_enabled_daytime_monday_config();
        let next_config = monday_config("10:00", "18:00");

        let changed = did_scheduler_topology_change(true, &previous_config, true, &next_config);

        assert!(changed);
    }

    #[tokio::test]
    async fn test_start_work_returns_persistence_error_when_runtime_save_fails() {
        let temp_dir = create_temp_dir("start-work-save-fails");
        let invalid_app_data_path = temp_dir.join("app-data-file");
        std::fs::write(&invalid_app_data_path, "not a directory")
            .expect("invalid app data file should be created");

        let state = build_test_state_with_persistence_dir(invalid_app_data_path).await;
        let settings = TimerSettings::new(
            WorkDurationSecs::try_new(1500).expect("work duration should be valid"),
            ShortBreakDurationSecs::try_new(300).expect("short break duration should be valid"),
            LongBreakDurationSecs::try_new(900).expect("long break duration should be valid"),
            LongBreakIntervalCount::try_new(4).expect("long break interval should be valid"),
            PostBreakTransitionPolicy::RequireManualContinue,
        );
        let mut runtime_effect_shell = RecordingRuntimeEffectShell::default();

        let result = start_work_for_tests(
            &state,
            settings,
            TimerStartSource::ManualOutsideWindow,
            &mut runtime_effect_shell,
        )
        .await;

        assert!(matches!(result, Err(PomoliceError::PersistenceSave(_))));

        cleanup_temp_dir(&temp_dir);
    }

    #[tokio::test]
    async fn test_boot_prompt_start_updates_runtime_flags_even_when_runtime_save_fails() {
        let temp_dir = create_temp_dir("boot-prompt-start-save-fails");
        let invalid_app_data_path = temp_dir.join("app-data-file");
        std::fs::write(&invalid_app_data_path, "not a directory")
            .expect("invalid app data file should be created");

        let state = build_test_state_with_persistence_dir(invalid_app_data_path).await;
        {
            let mut runtime_state = state
                .boot_prompt_runtime_state
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            *runtime_state = BootPromptRuntimeState::AwaitingDecision;
        }

        let timer_settings = {
            let timer_manager = state.timer_manager.lock().await;
            timer_manager.get_state().settings.clone()
        };
        let mut runtime_effect_shell = RecordingRuntimeEffectShell::default();

        let result = resolve_boot_auto_start_prompt_start_for_tests(
            &state,
            timer_settings,
            &mut runtime_effect_shell,
        )
        .await;

        assert!(matches!(result, Err(PomoliceError::PersistenceSave(_))));
        assert_eq!(
            *state
                .boot_prompt_runtime_state
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner),
            BootPromptRuntimeState::ResolvedStart
        );
        assert!(state.auto_start_boundary_enabled.load(Ordering::Acquire));
        assert_eq!(
            state.timer_manager.lock().await.status(),
            TimerStatus::Working
        );

        cleanup_temp_dir(&temp_dir);
    }

    #[tokio::test]
    async fn test_start_work_persistence_failure_records_production_runtime_effect_order() {
        let temp_dir = create_temp_dir("start-work-ordering-save-fails");
        let invalid_app_data_path = temp_dir.join("app-data-file");
        std::fs::write(&invalid_app_data_path, "not a directory")
            .expect("invalid app data file should be created");

        let state = build_test_state_with_persistence_dir(invalid_app_data_path).await;
        let settings = TimerSettings::new(
            WorkDurationSecs::try_new(1500).expect("work duration should be valid"),
            ShortBreakDurationSecs::try_new(300).expect("short break duration should be valid"),
            LongBreakDurationSecs::try_new(900).expect("long break duration should be valid"),
            LongBreakIntervalCount::try_new(4).expect("long break interval should be valid"),
            PostBreakTransitionPolicy::RequireManualContinue,
        );
        let mut runtime_effect_shell = RecordingRuntimeEffectShell::default();

        let result = start_work_for_tests(
            &state,
            settings,
            TimerStartSource::ManualOutsideWindow,
            &mut runtime_effect_shell,
        )
        .await;

        assert!(matches!(result, Err(PomoliceError::PersistenceSave(_))));
        assert_eq!(
            runtime_effect_shell.events(),
            &[
                RuntimeEffectEvent::EmitTick,
                RuntimeEffectEvent::UpdateTray,
                RuntimeEffectEvent::EmitWorkStarted,
                RuntimeEffectEvent::SpawnTimerTask,
                RuntimeEffectEvent::Persist,
            ]
        );

        cleanup_temp_dir(&temp_dir);
    }

    #[tokio::test]
    async fn test_boot_prompt_start_persistence_failure_records_production_runtime_effect_order() {
        let temp_dir = create_temp_dir("boot-prompt-ordering-save-fails");
        let invalid_app_data_path = temp_dir.join("app-data-file");
        std::fs::write(&invalid_app_data_path, "not a directory")
            .expect("invalid app data file should be created");

        let state = build_test_state_with_persistence_dir(invalid_app_data_path).await;
        {
            let mut runtime_state = state
                .boot_prompt_runtime_state
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            *runtime_state = BootPromptRuntimeState::AwaitingDecision;
        }

        let timer_settings = {
            let timer_manager = state.timer_manager.lock().await;
            timer_manager.get_state().settings.clone()
        };
        let mut runtime_effect_shell = RecordingRuntimeEffectShell::default();

        let result = resolve_boot_auto_start_prompt_start_for_tests(
            &state,
            timer_settings,
            &mut runtime_effect_shell,
        )
        .await;

        assert!(matches!(result, Err(PomoliceError::PersistenceSave(_))));
        assert_eq!(
            runtime_effect_shell.events(),
            &[
                RuntimeEffectEvent::EmitTick,
                RuntimeEffectEvent::UpdateTray,
                RuntimeEffectEvent::EmitWorkStarted,
                RuntimeEffectEvent::SpawnTimerTask,
                RuntimeEffectEvent::Persist,
            ]
        );

        cleanup_temp_dir(&temp_dir);
    }

    #[tokio::test]
    async fn test_emergency_stop_exit_uses_break_mode_transition_coordinator() {
        let state = Arc::new(build_test_state_without_persistence_path().await);
        let events = Arc::new(StdMutex::new(Vec::new()));
        let release_first = Arc::new(StdAtomicBool::new(false));

        let first_transition = {
            let state = Arc::clone(&state);
            let events = Arc::clone(&events);
            let release_first = Arc::clone(&release_first);
            tokio::spawn(async move {
                run_emergency_stop_exit_uses_break_mode_transition_coordinator_for_tests(
                    state,
                    events,
                    "first-start",
                    "first-end",
                    release_first,
                )
                .await;
            })
        };

        wait_for_transition_event_count(&events, 1, "the first emergency-stop exit transition")
            .await;

        let second_transition = {
            let state = Arc::clone(&state);
            let events = Arc::clone(&events);
            tokio::spawn(async move {
                run_emergency_stop_exit_uses_break_mode_transition_coordinator_for_tests(
                    state,
                    events,
                    "second-start",
                    "second-end",
                    Arc::new(StdAtomicBool::new(true)),
                )
                .await;
            })
        };

        tokio::time::sleep(StdDuration::from_millis(50)).await;
        assert_eq!(
            events
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .as_slice(),
            ["first-start"]
        );

        release_first.store(true, Ordering::Release);

        first_transition
            .await
            .expect("first emergency-stop transition should complete");
        second_transition
            .await
            .expect("second emergency-stop transition should complete");

        assert_eq!(
            events
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .as_slice(),
            ["first-start", "first-end", "second-start", "second-end"]
        );
    }
}
