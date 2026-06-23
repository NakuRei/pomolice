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

trait TimerCommandRuntimeEffectShell {
    fn emit_tick(&mut self, tick_payload: &TimerTickPayload);
    fn update_tray(&mut self, tick_payload: &TimerTickPayload);
    fn spawn_timer_task<'a>(
        &'a mut self,
        state: &'a AppState,
    ) -> Pin<Box<dyn Future<Output = ()> + Send + 'a>>;
    fn request_fullscreen_break_exit(&mut self);
    fn before_persist(&mut self) {}
}

struct AppHandleTimerCommandRuntimeEffectShell<'a> {
    app: &'a tauri::AppHandle,
}

impl TimerCommandRuntimeEffectShell for AppHandleTimerCommandRuntimeEffectShell<'_> {
    fn emit_tick(&mut self, tick_payload: &TimerTickPayload) {
        let _ = self.app.emit("timer_tick", tick_payload);
    }

    fn update_tray(&mut self, tick_payload: &TimerTickPayload) {
        crate::tray::update_tray_icon(self.app, tick_payload);
    }

    fn spawn_timer_task<'a>(
        &'a mut self,
        _state: &'a AppState,
    ) -> Pin<Box<dyn Future<Output = ()> + Send + 'a>> {
        Box::pin(async move {
            crate::session_pipeline::spawn_timer_task(self.app).await;
        })
    }

    fn request_fullscreen_break_exit(&mut self) {
        let app_handle = self.app.clone();
        tauri::async_runtime::spawn(async move {
            crate::session_pipeline::lib_handle_external_break_exit(app_handle).await;
        });
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

    let app = window.app_handle();
    let mut runtime_effect_shell = AppHandleTimerCommandRuntimeEffectShell { app };
    resolve_post_break_decision_with_runtime_effects(&state, action, &mut runtime_effect_shell)
        .await
}

async fn resolve_post_break_decision_with_runtime_effects<S>(
    state: &AppState,
    action: PostBreakDecisionAction,
    runtime_effect_shell: &mut S,
) -> Result<()>
where
    S: TimerCommandRuntimeEffectShell,
{
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

    emit_tick_and_update_tray(runtime_effect_shell, &tick_payload);

    if let Some(handle) = old_task {
        // Await outside the lock to avoid lock-held await deadlocks.
        let _ = handle.await;
    }

    if should_spawn_timer_task {
        // Continue action always starts a fresh timer task.
        runtime_effect_shell.spawn_timer_task(state).await;
    }

    persist_current_state_after_command_effects(state, runtime_effect_shell).await
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

    let app = window.app_handle();
    let mut runtime_effect_shell = AppHandleTimerCommandRuntimeEffectShell { app };
    emergency_stop_with_runtime_effects(&state, &mut runtime_effect_shell).await
}

async fn emergency_stop_with_runtime_effects<S>(
    state: &AppState,
    runtime_effect_shell: &mut S,
) -> Result<()>
where
    S: TimerCommandRuntimeEffectShell,
{
    let (was_fullscreen_break, tick_payload, old_task) = {
        let mut timer = state.timer_manager.lock().await;
        // Take/cancel under the lock, but never await under the lock.
        let old = timer.cancel_and_take_task();
        let (was_fullscreen, payload) = timer.emergency_stop();
        (was_fullscreen, payload, old)
    };

    emit_tick_and_update_tray(runtime_effect_shell, &tick_payload);

    if let Some(handle) = old_task {
        // Await outside the lock to avoid lock-held await deadlocks.
        let _ = handle.await;
    }

    // Exit fullscreen if needed
    if was_fullscreen_break {
        runtime_effect_shell.request_fullscreen_break_exit();
    }

    persist_current_state_after_command_effects(state, runtime_effect_shell).await
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

    let app = window.app_handle();
    let mut runtime_effect_shell = AppHandleTimerCommandRuntimeEffectShell { app };
    sync_timer_duration_settings_with_runtime_effects(
        &state,
        work_duration_secs,
        short_break_duration_secs,
        long_break_duration_secs,
        long_break_interval,
        &mut runtime_effect_shell,
    )
    .await
}

async fn sync_timer_duration_settings_with_runtime_effects<S>(
    state: &AppState,
    work_duration_secs: WorkDurationSecs,
    short_break_duration_secs: ShortBreakDurationSecs,
    long_break_duration_secs: LongBreakDurationSecs,
    long_break_interval: LongBreakIntervalCount,
    runtime_effect_shell: &mut S,
) -> Result<()>
where
    S: TimerCommandRuntimeEffectShell,
{
    let tick_payload = {
        let mut timer = state.timer_manager.lock().await;
        timer.sync_timer_duration_settings(
            work_duration_secs,
            short_break_duration_secs,
            long_break_duration_secs,
            long_break_interval,
        )?
    };

    emit_tick_and_update_tray(runtime_effect_shell, &tick_payload);
    persist_current_state_after_command_effects(state, runtime_effect_shell).await
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

    let app = window.app_handle();
    let mut runtime_effect_shell = AppHandleTimerCommandRuntimeEffectShell { app };
    sync_automation_policies_with_runtime_effects(
        &state,
        auto_run_feature_enabled,
        weekly_auto_run_config,
        post_break_transition_policy,
        &mut runtime_effect_shell,
    )
    .await
}

async fn sync_automation_policies_with_runtime_effects<S>(
    state: &AppState,
    auto_run_feature_enabled: bool,
    weekly_auto_run_config: WeeklyAutoRunConfig,
    post_break_transition_policy: PostBreakTransitionPolicy,
    runtime_effect_shell: &mut S,
) -> Result<()>
where
    S: TimerCommandRuntimeEffectShell,
{
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

    emit_tick_and_update_tray(runtime_effect_shell, &tick_payload);
    persist_current_state_after_command_effects(state, runtime_effect_shell).await
}

fn emit_tick_and_update_tray<S>(runtime_effect_shell: &mut S, tick_payload: &TimerTickPayload)
where
    S: TimerCommandRuntimeEffectShell,
{
    runtime_effect_shell.emit_tick(tick_payload);
    runtime_effect_shell.update_tray(tick_payload);
}

async fn persist_current_state_after_command_effects<S>(
    state: &AppState,
    runtime_effect_shell: &mut S,
) -> Result<()>
where
    S: TimerCommandRuntimeEffectShell,
{
    runtime_effect_shell.before_persist();
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

async fn resolve_boot_auto_start_prompt_start_contract(
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
    let start_sequence = {
        let mut timer = state.timer_manager.lock().await;
        let (tick_payload, work_duration) =
            timer.start_work_with_source(settings, manual_start_source)?;
        let old_task = timer.cancel_and_take_task();
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

fn mark_boot_prompt_resolved_later(state: &AppState) {
    if let Ok(mut runtime_state) = state.boot_prompt_runtime_state.lock() {
        *runtime_state = BootPromptRuntimeState::ResolvedLater;
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

async fn get_full_state_contract(state: &AppState) -> Result<FullStatePayload> {
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

/// Get full timer state for initial sync
#[tauri::command]
pub(crate) async fn get_full_state(
    window: WebviewWindow,
    state: State<'_, AppState>,
) -> Result<FullStatePayload> {
    authorize_main_window(&window, "get_full_state")?;

    get_full_state_contract(&state).await
}

async fn resolve_boot_auto_start_prompt_contract(
    state: &AppState,
    start_now: bool,
    runtime_effect_shell: &mut impl StartRuntimeEffectShell,
) -> Result<BootPromptRuntimeState> {
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
            mark_boot_prompt_not_applicable(state);
        }
        return Ok(BootPromptRuntimeState::NotApplicable);
    }

    let timer_settings = {
        let timer_manager = state.timer_manager.lock().await;
        timer_manager.get_state().settings.clone()
    };

    if start_now {
        return resolve_boot_auto_start_prompt_start_contract(
            state,
            timer_settings,
            runtime_effect_shell,
        )
        .await;
    }

    mark_boot_prompt_resolved_later(state);
    Ok(BootPromptRuntimeState::ResolvedLater)
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

    let app = window.app_handle();
    let mut runtime_effect_shell = AppHandleRuntimeEffectShell { app };
    resolve_boot_auto_start_prompt_contract(&state, start_now, &mut runtime_effect_shell).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::managers::clock::testing::FixedClock;
    use crate::managers::clock::Clock;
    use crate::managers::persistence::PersistenceManager;
    use crate::scheduler::SchedulerTickTestRuntime;
    use chrono::{Local, LocalResult, Utc};
    use std::path::{Path, PathBuf};
    use std::sync::atomic::AtomicBool as StdAtomicBool;
    use std::sync::{Arc, Mutex as StdMutex};
    use std::time::Duration as StdDuration;
    use std::time::{SystemTime, UNIX_EPOCH};

    use crate::types::{DayAutoRunSlot, HhMm, SessionTransitionReason, SessionType, TimerState};
    use chrono::TimeZone;
    use serde_json::json;
    use tokio_util::sync::CancellationToken;

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    enum RuntimeEffectEvent {
        EmitTick,
        UpdateTray,
        EmitWorkStarted,
        SpawnTimerTask,
        RequestFullscreenBreakExit,
        Persist,
    }

    #[derive(Clone, Debug, PartialEq)]
    struct TimerTickSnapshot {
        status: TimerStatus,
        remaining_seconds: u64,
        session_type: SessionType,
        current_cycle: u32,
        completed_pomodoros: u32,
        break_interaction_count: u32,
        is_fullscreen_break: bool,
    }

    impl From<&TimerTickPayload> for TimerTickSnapshot {
        fn from(payload: &TimerTickPayload) -> Self {
            Self {
                status: payload.status,
                remaining_seconds: payload.remaining_seconds,
                session_type: payload.session_type,
                current_cycle: payload.current_cycle,
                completed_pomodoros: payload.completed_pomodoros,
                break_interaction_count: payload.break_interaction_count,
                is_fullscreen_break: payload.is_fullscreen_break,
            }
        }
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

    #[derive(Default)]
    struct RecordingCommandRuntimeEffectShell {
        events: Vec<RuntimeEffectEvent>,
        emitted_tick_snapshots: Vec<TimerTickSnapshot>,
        tray_tick_snapshots: Vec<TimerTickSnapshot>,
    }

    impl RecordingCommandRuntimeEffectShell {
        fn events(&self) -> &[RuntimeEffectEvent] {
            &self.events
        }
    }

    impl TimerCommandRuntimeEffectShell for RecordingCommandRuntimeEffectShell {
        fn emit_tick(&mut self, tick_payload: &TimerTickPayload) {
            self.events.push(RuntimeEffectEvent::EmitTick);
            self.emitted_tick_snapshots
                .push(TimerTickSnapshot::from(tick_payload));
        }

        fn update_tray(&mut self, tick_payload: &TimerTickPayload) {
            self.events.push(RuntimeEffectEvent::UpdateTray);
            self.tray_tick_snapshots
                .push(TimerTickSnapshot::from(tick_payload));
        }

        fn spawn_timer_task<'a>(
            &'a mut self,
            _state: &'a AppState,
        ) -> Pin<Box<dyn Future<Output = ()> + Send + 'a>> {
            self.events.push(RuntimeEffectEvent::SpawnTimerTask);
            Box::pin(async {})
        }

        fn request_fullscreen_break_exit(&mut self) {
            self.events
                .push(RuntimeEffectEvent::RequestFullscreenBreakExit);
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

    fn local_datetime(
        year: i32,
        month: u32,
        day: u32,
        hour: u32,
        minute: u32,
        second: u32,
    ) -> chrono::DateTime<Local> {
        match Local.with_ymd_and_hms(year, month, day, hour, minute, second) {
            LocalResult::Single(datetime) => datetime,
            LocalResult::Ambiguous(first, _) => first,
            LocalResult::None => panic!("test local datetime should exist"),
        }
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

    async fn build_test_state_at(now_local: chrono::DateTime<Local>) -> AppState {
        let clock = Arc::new(FixedClock::new(now_local));
        let persistence = PersistenceManager::new(as_clock_trait(&clock), None);
        let (app_state, _) =
            AppState::build_startup_state_for_tests(as_clock_trait(&clock), persistence).await;
        app_state
    }

    async fn build_test_state_with_injected_persistence_error() -> AppState {
        let clock = fixed_clock(create_utc_datetime(2025, 2, 1, 9, 0, 0));
        let persistence = PersistenceManager::new(as_clock_trait(&clock), Some(PathBuf::from(".")))
            .with_injected_save_error_hook(Arc::new(|| {
                Some(crate::managers::persistence::SaveError::Serialize {
                    source: serde_json::from_str::<serde_json::Value>("{")
                        .expect_err("invalid JSON should fail to parse"),
                })
            }));
        let (app_state, _) =
            AppState::build_startup_state_for_tests(as_clock_trait(&clock), persistence).await;
        app_state
    }

    async fn build_test_state_with_panicking_persistence_hook() -> AppState {
        let clock = fixed_clock(create_utc_datetime(2025, 2, 1, 9, 0, 0));
        let persistence = PersistenceManager::new(as_clock_trait(&clock), Some(PathBuf::from(".")))
            .with_injected_save_error_hook(Arc::new(|| {
                panic!("simulated persistence worker panic at C:\\Users\\nak\\secret")
            }));
        let (app_state, _) =
            AppState::build_startup_state_for_tests(as_clock_trait(&clock), persistence).await;
        app_state
    }

    fn timer_settings(
        work_duration_secs: u64,
        short_break_duration_secs: u64,
        long_break_duration_secs: u64,
        long_break_interval: u32,
        post_break_transition_policy: PostBreakTransitionPolicy,
    ) -> TimerSettings {
        TimerSettings::new(
            WorkDurationSecs::try_new(work_duration_secs).expect("work duration should be valid"),
            ShortBreakDurationSecs::try_new(short_break_duration_secs)
                .expect("short break duration should be valid"),
            LongBreakDurationSecs::try_new(long_break_duration_secs)
                .expect("long break duration should be valid"),
            LongBreakIntervalCount::try_new(long_break_interval)
                .expect("long break interval should be valid"),
            post_break_transition_policy,
        )
    }

    fn command_work_duration_secs() -> WorkDurationSecs {
        WorkDurationSecs::try_new(1800).expect("work duration should be valid")
    }

    fn command_short_break_duration_secs() -> ShortBreakDurationSecs {
        ShortBreakDurationSecs::try_new(360).expect("short break duration should be valid")
    }

    fn command_long_break_duration_secs() -> LongBreakDurationSecs {
        LongBreakDurationSecs::try_new(1200).expect("long break duration should be valid")
    }

    fn command_long_break_interval() -> LongBreakIntervalCount {
        LongBreakIntervalCount::try_new(5).expect("long break interval should be valid")
    }

    async fn prepare_post_break_decision_pending_state(state: &AppState) {
        let mut timer = state.timer_manager.lock().await;
        let settings = timer_settings(
            1500,
            300,
            900,
            4,
            PostBreakTransitionPolicy::RequireManualContinue,
        );
        timer
            .start_work_with_source(settings, TimerStartSource::ManualOutsideWindow)
            .expect("work session should start");
        timer.transition_to_break();
        timer.complete_break_session_transition();
        assert_eq!(timer.status(), TimerStatus::Idle);
        assert!(timer.get_state().post_break_decision_pending);
    }

    async fn apply_restored_post_break_decision_pending_state(state: &AppState) {
        let mut timer = state.timer_manager.lock().await;
        let settings = timer_settings(
            1500,
            300,
            900,
            4,
            PostBreakTransitionPolicy::RequireManualContinue,
        );
        let _ = timer.apply_persisted_state(TimerState {
            status: TimerStatus::Idle,
            remaining_seconds: settings.work_duration_secs.get(),
            session_type: SessionType::Work,
            current_cycle: 4,
            completed_pomodoros: 3,
            post_break_decision_pending: true,
            settings,
            ..TimerState::default()
        });
        assert_eq!(timer.status(), TimerStatus::Idle);
        assert!(timer.get_state().post_break_decision_pending);
    }

    async fn prepare_fullscreen_break_state(state: &AppState) {
        let mut timer = state.timer_manager.lock().await;
        let settings = timer_settings(
            1500,
            300,
            900,
            4,
            PostBreakTransitionPolicy::RequireManualContinue,
        );
        timer
            .start_work_with_source(settings, TimerStartSource::ManualOutsideWindow)
            .expect("work session should start");
        timer.transition_to_break();
        timer.set_fullscreen_break(true);
        assert_eq!(timer.status(), TimerStatus::OnBreak);
        assert!(timer.get_state().is_fullscreen_break);
    }

    async fn install_cancellable_timer_task(state: &AppState) -> Arc<StdAtomicBool> {
        let task_completed = Arc::new(StdAtomicBool::new(false));
        let task_completed_for_task = Arc::clone(&task_completed);
        let cancellation_token = CancellationToken::new();
        let task_token = cancellation_token.clone();
        let timer_task = tokio::spawn(async move {
            task_token.cancelled().await;
            task_completed_for_task.store(true, Ordering::Release);
        });

        state
            .timer_manager
            .lock()
            .await
            .set_task_handles_for_test(cancellation_token, timer_task);

        task_completed
    }

    async fn install_timer_countdown_task(state: &Arc<AppState>) {
        let state_for_task = Arc::downgrade(state);

        state.timer_manager.lock().await.spawn_timer_task(move || {
            let state_for_tick = std::sync::Weak::clone(&state_for_task);
            async move {
                if let Some(state) = state_for_tick.upgrade() {
                    let mut timer = state.timer_manager.lock().await;
                    if timer.status() != TimerStatus::Idle {
                        let _ = timer.tick();
                    }
                }
            }
        });
    }

    fn assert_tick_emit_and_tray_match(runtime_effect_shell: &RecordingCommandRuntimeEffectShell) {
        assert_eq!(runtime_effect_shell.emitted_tick_snapshots.len(), 1);
        assert_eq!(runtime_effect_shell.tray_tick_snapshots.len(), 1);
        assert_eq!(
            runtime_effect_shell.emitted_tick_snapshots,
            runtime_effect_shell.tray_tick_snapshots
        );
    }

    fn event_count(
        runtime_effect_shell: &RecordingCommandRuntimeEffectShell,
        event: RuntimeEffectEvent,
    ) -> usize {
        runtime_effect_shell
            .events()
            .iter()
            .filter(|recorded| **recorded == event)
            .count()
    }

    fn assert_event_once(
        runtime_effect_shell: &RecordingCommandRuntimeEffectShell,
        event: RuntimeEffectEvent,
    ) {
        assert_eq!(event_count(runtime_effect_shell, event), 1);
    }

    fn assert_event_absent(
        runtime_effect_shell: &RecordingCommandRuntimeEffectShell,
        event: RuntimeEffectEvent,
    ) {
        assert_eq!(event_count(runtime_effect_shell, event), 0);
    }

    fn assert_persistence_save_error(result: &Result<()>) {
        assert!(matches!(result, Err(PomoliceError::PersistenceSave(_))));
    }

    fn assert_command_result_public_error<T>(
        result: Result<T>,
        expected_code: &str,
        diagnostic_fragments: &[&str],
    ) {
        let Err(error) = result else {
            panic!("command result should fail");
        };

        let serialized = serde_json::to_value(&error).expect("command error should serialize");
        let response = serialized
            .as_object()
            .expect("command error should serialize as a JSON object");

        assert_eq!(
            response
                .get("code")
                .expect("command error should include code")
                .as_str(),
            Some(expected_code)
        );
        let message = response
            .get("message")
            .expect("command error should include message")
            .as_str()
            .expect("command error message should be text");
        assert!(!message.is_empty());
        assert_public_error_excludes_retry_affordance(response);

        let serialized = serialized.to_string();
        for fragment in diagnostic_fragments {
            assert!(
                !serialized.contains(fragment),
                "public command error leaked diagnostic fragment: {fragment}"
            );
        }
    }

    fn assert_public_error_excludes_retry_affordance(
        response: &serde_json::Map<String, serde_json::Value>,
    ) {
        for key in response.keys() {
            let normalized = key.to_ascii_lowercase();
            assert!(
                !normalized.contains("retry") && !normalized.contains("transient"),
                "backend public command error encoded retry eligibility in field: {key}"
            );
        }
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

    fn monday_and_tuesday_config(start_time: &str, end_time: &str) -> WeeklyAutoRunConfig {
        WeeklyAutoRunConfig::try_new(
            enabled_slot(start_time, end_time),
            enabled_slot(start_time, end_time),
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

    async fn sync_auto_run_policies_for_test(
        state: &AppState,
        weekly_auto_run_config: WeeklyAutoRunConfig,
        post_break_transition_policy: PostBreakTransitionPolicy,
    ) {
        sync_auto_run_policies_with_feature_for_test(
            state,
            true,
            weekly_auto_run_config,
            post_break_transition_policy,
        )
        .await;
    }

    async fn sync_auto_run_policies_with_feature_for_test(
        state: &AppState,
        auto_run_feature_enabled: bool,
        weekly_auto_run_config: WeeklyAutoRunConfig,
        post_break_transition_policy: PostBreakTransitionPolicy,
    ) {
        let mut runtime_effect_shell = RecordingCommandRuntimeEffectShell::default();

        sync_automation_policies_with_runtime_effects(
            state,
            auto_run_feature_enabled,
            weekly_auto_run_config,
            post_break_transition_policy,
            &mut runtime_effect_shell,
        )
        .await
        .expect("automation policy sync should succeed");
    }

    async fn start_auto_run_eligible_work_session(state: &AppState) {
        let mut timer = state.timer_manager.lock().await;
        timer
            .start_work_with_source(
                TimerSettings::default(),
                TimerStartSource::ManualInsideWindow,
            )
            .expect("eligible work session should start");
    }

    async fn complete_work_session_reason(state: &AppState) -> SessionTransitionReason {
        let mut timer = state.timer_manager.lock().await;
        timer.complete_work_session_transition().transition_reason
    }

    fn assert_daytime_monday_window(full_state: &FullStatePayload) {
        let active_window = full_state
            .active_window_summary
            .as_ref()
            .expect("Monday daytime window should be active");

        assert_eq!(active_window.weekday_key, "mon");
        assert_eq!(active_window.start_time, "09:00");
        assert_eq!(active_window.end_time, "18:00");
        assert!(!active_window.crosses_midnight);
    }

    async fn build_prompting_state(
        now_local: chrono::DateTime<Local>,
        weekly_auto_run_config: WeeklyAutoRunConfig,
    ) -> AppState {
        let state = build_test_state_at(now_local).await;
        sync_auto_run_policies_for_test(
            &state,
            weekly_auto_run_config,
            PostBreakTransitionPolicy::RequireManualContinue,
        )
        .await;

        let full_state = get_full_state_contract(&state)
            .await
            .expect("full state should be available");
        assert!(full_state.should_prompt_start_on_boot);
        assert!(full_state.active_window_summary.is_some());

        state
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
    fn test_unauthorized_window_returns_public_command_error() {
        let result = authorize_main_window_label("C:\\Users\\nak\\secret\\settings");

        assert_command_result_public_error(
            result,
            "unauthorized_window",
            &["C:\\Users\\nak", "secret", "settings"],
        );
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

    #[tokio::test]
    async fn get_full_state_prompts_on_first_idle_sync_inside_enabled_window() {
        let state = build_test_state_at(local_datetime(2025, 1, 6, 10, 0, 0)).await;
        sync_auto_run_policies_for_test(
            &state,
            make_enabled_daytime_monday_config(),
            PostBreakTransitionPolicy::RequireManualContinue,
        )
        .await;

        let full_state = get_full_state_contract(&state)
            .await
            .expect("full state should be available");

        assert_eq!(full_state.status, TimerStatus::Idle);
        assert!(full_state.should_prompt_start_on_boot);
        assert_daytime_monday_window(&full_state);
    }

    #[tokio::test]
    async fn get_full_state_hides_active_window_when_auto_run_is_disabled() {
        let state = build_test_state_at(local_datetime(2025, 1, 6, 10, 0, 0)).await;
        sync_auto_run_policies_with_feature_for_test(
            &state,
            false,
            make_enabled_daytime_monday_config(),
            PostBreakTransitionPolicy::RequireManualContinue,
        )
        .await;

        let full_state = get_full_state_contract(&state)
            .await
            .expect("full state should be available");

        assert_eq!(full_state.status, TimerStatus::Idle);
        assert!(!full_state.should_prompt_start_on_boot);
        assert!(full_state.active_window_summary.is_none());
    }

    #[tokio::test]
    async fn get_full_state_prompts_only_on_first_full_sync() {
        let state = build_test_state_at(local_datetime(2025, 1, 6, 10, 0, 0)).await;
        sync_auto_run_policies_for_test(
            &state,
            make_enabled_daytime_monday_config(),
            PostBreakTransitionPolicy::RequireManualContinue,
        )
        .await;

        let first_full_state = get_full_state_contract(&state)
            .await
            .expect("first full state should be available");
        let second_full_state = get_full_state_contract(&state)
            .await
            .expect("second full state should be available");

        assert!(first_full_state.should_prompt_start_on_boot);
        assert_daytime_monday_window(&first_full_state);
        assert!(!second_full_state.should_prompt_start_on_boot);
        assert_daytime_monday_window(&second_full_state);
    }

    #[tokio::test]
    async fn get_full_state_does_not_prompt_when_timer_is_already_running() {
        let state = build_test_state_at(local_datetime(2025, 1, 6, 10, 0, 0)).await;
        sync_auto_run_policies_for_test(
            &state,
            make_enabled_daytime_monday_config(),
            PostBreakTransitionPolicy::RequireManualContinue,
        )
        .await;
        {
            let mut timer = state.timer_manager.lock().await;
            timer
                .start_work_with_source(
                    TimerSettings::default(),
                    TimerStartSource::ManualInsideWindow,
                )
                .expect("work session should start");
        }

        let full_state = get_full_state_contract(&state)
            .await
            .expect("full state should be available");

        assert_eq!(full_state.status, TimerStatus::Working);
        assert!(!full_state.should_prompt_start_on_boot);
        assert_daytime_monday_window(&full_state);
    }

    #[tokio::test]
    async fn scheduler_does_not_auto_start_while_boot_prompt_is_waiting() {
        let state = build_prompting_state(
            local_datetime(2025, 1, 6, 9, 0, 5),
            make_enabled_daytime_monday_config(),
        )
        .await;
        let mut scheduler_runtime =
            SchedulerTickTestRuntime::from_state(&state, local_datetime(2025, 1, 6, 8, 59, 50));

        scheduler_runtime
            .run_tick(&state, local_datetime(2025, 1, 6, 9, 0, 10))
            .await;

        let full_state = state.timer_manager.lock().await.get_full_state();
        assert_eq!(full_state.status, TimerStatus::Idle);
        assert_eq!(full_state.session_type, SessionType::Work);
    }

    #[tokio::test]
    async fn resolve_boot_prompt_later_keeps_timer_idle() {
        let state = build_prompting_state(
            local_datetime(2025, 1, 6, 10, 0, 0),
            make_enabled_daytime_monday_config(),
        )
        .await;
        let mut runtime_effect_shell = RecordingRuntimeEffectShell::default();

        let result =
            resolve_boot_auto_start_prompt_contract(&state, false, &mut runtime_effect_shell)
                .await
                .expect("boot prompt should resolve later");

        let full_state = state.timer_manager.lock().await.get_full_state();
        assert_eq!(result, BootPromptRuntimeState::ResolvedLater);
        assert_eq!(full_state.status, TimerStatus::Idle);
        assert_eq!(full_state.session_type, SessionType::Work);
        assert!(runtime_effect_shell.events().is_empty());
    }

    #[tokio::test]
    async fn resolved_later_allows_next_scheduled_start_boundary() {
        let state = build_prompting_state(
            local_datetime(2025, 1, 6, 10, 0, 0),
            monday_and_tuesday_config("09:00", "18:00"),
        )
        .await;
        let mut runtime_effect_shell = RecordingRuntimeEffectShell::default();
        let mut scheduler_runtime =
            SchedulerTickTestRuntime::from_state(&state, local_datetime(2025, 1, 6, 10, 0, 0));

        let result =
            resolve_boot_auto_start_prompt_contract(&state, false, &mut runtime_effect_shell)
                .await
                .expect("boot prompt should resolve later");
        scheduler_runtime
            .run_tick(&state, local_datetime(2025, 1, 7, 9, 0, 10))
            .await;

        let full_state = state.timer_manager.lock().await.get_full_state();
        assert_eq!(result, BootPromptRuntimeState::ResolvedLater);
        assert_eq!(full_state.status, TimerStatus::Working);
        assert_eq!(full_state.session_type, SessionType::Work);
        assert_eq!(
            full_state.remaining_seconds,
            full_state.settings.work_duration_secs.get()
        );
    }

    #[tokio::test]
    async fn resolve_boot_prompt_start_begins_work_when_prompt_is_active() {
        let state = build_prompting_state(
            local_datetime(2025, 1, 6, 10, 0, 0),
            make_enabled_daytime_monday_config(),
        )
        .await;
        let mut runtime_effect_shell = RecordingRuntimeEffectShell::default();

        let result =
            resolve_boot_auto_start_prompt_contract(&state, true, &mut runtime_effect_shell)
                .await
                .expect("boot prompt should start work");

        let full_state = state.timer_manager.lock().await.get_full_state();
        assert_eq!(result, BootPromptRuntimeState::ResolvedStart);
        assert_eq!(full_state.status, TimerStatus::Working);
        assert_eq!(full_state.session_type, SessionType::Work);
        assert_eq!(
            full_state.remaining_seconds,
            full_state.settings.work_duration_secs.get()
        );
    }

    #[tokio::test]
    async fn stale_boot_prompt_resolution_does_not_replace_running_session() {
        for start_now in [true, false] {
            let state = build_prompting_state(
                local_datetime(2025, 1, 6, 10, 0, 0),
                make_enabled_daytime_monday_config(),
            )
            .await;
            {
                let mut timer = state.timer_manager.lock().await;
                timer
                    .start_work_with_source(
                        TimerSettings::default(),
                        TimerStartSource::ManualOutsideWindow,
                    )
                    .expect("work session should start through another path");
            }
            let before = state.timer_manager.lock().await.get_full_state();
            let mut runtime_effect_shell = RecordingRuntimeEffectShell::default();

            let result = resolve_boot_auto_start_prompt_contract(
                &state,
                start_now,
                &mut runtime_effect_shell,
            )
            .await
            .expect("stale boot prompt should resolve safely");

            let after = state.timer_manager.lock().await.get_full_state();
            assert_eq!(result, BootPromptRuntimeState::NotApplicable);
            assert_eq!(after.status, TimerStatus::Working);
            assert_eq!(after.session_type, before.session_type);
            assert_eq!(after.current_cycle, before.current_cycle);
            assert_eq!(after.remaining_seconds, before.remaining_seconds);
            assert!(runtime_effect_shell.events().is_empty());
        }
    }

    #[tokio::test]
    async fn boot_prompt_resolution_without_active_prompt_does_not_start_work() {
        for start_now in [true, false] {
            let state = build_test_state_at(local_datetime(2025, 1, 6, 10, 0, 0)).await;
            let mut runtime_effect_shell = RecordingRuntimeEffectShell::default();

            let result = resolve_boot_auto_start_prompt_contract(
                &state,
                start_now,
                &mut runtime_effect_shell,
            )
            .await
            .expect("missing boot prompt should resolve safely");

            let full_state = state.timer_manager.lock().await.get_full_state();
            assert_eq!(result, BootPromptRuntimeState::NotApplicable);
            assert_eq!(full_state.status, TimerStatus::Idle);
            assert_eq!(full_state.session_type, SessionType::Work);
            assert!(runtime_effect_shell.events().is_empty());
        }
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

    #[tokio::test]
    async fn test_resolve_post_break_decision_invalid_state_returns_public_command_error() {
        let state = build_test_state_without_persistence_path().await;
        let mut runtime_effect_shell = RecordingCommandRuntimeEffectShell::default();

        let result = resolve_post_break_decision_with_runtime_effects(
            &state,
            PostBreakDecisionAction::Continue,
            &mut runtime_effect_shell,
        )
        .await;

        assert_command_result_public_error(
            result,
            "invalid_state_transition",
            &["resolve post-break decision", "Idle"],
        );
    }

    #[tokio::test]
    async fn test_start_work_rate_limit_returns_public_command_error() {
        let state = build_test_state_without_persistence_path().await;
        let mut first_runtime_effect_shell = RecordingRuntimeEffectShell::default();

        start_work_for_tests(
            &state,
            timer_settings(
                1500,
                300,
                900,
                4,
                PostBreakTransitionPolicy::RequireManualContinue,
            ),
            TimerStartSource::ManualOutsideWindow,
            &mut first_runtime_effect_shell,
        )
        .await
        .expect("first start should succeed");

        {
            let mut timer = state.timer_manager.lock().await;
            timer.emergency_stop();
        }

        let mut second_runtime_effect_shell = RecordingRuntimeEffectShell::default();
        let result = start_work_for_tests(
            &state,
            timer_settings(
                1500,
                300,
                900,
                4,
                PostBreakTransitionPolicy::RequireManualContinue,
            ),
            TimerStartSource::ManualOutsideWindow,
            &mut second_runtime_effect_shell,
        )
        .await;

        assert_command_result_public_error(result, "rate_limited", &["please wait", "ms"]);
    }

    #[tokio::test(start_paused = true)]
    async fn test_start_work_rejected_during_break_keeps_break_countdown_running() {
        let state = Arc::new(build_test_state_without_persistence_path().await);
        prepare_fullscreen_break_state(&state).await;
        install_timer_countdown_task(&state).await;

        tokio::task::yield_now().await;
        tokio::time::advance(StdDuration::from_secs(1)).await;
        tokio::task::yield_now().await;

        let remaining_after_first_tick = {
            let timer = state.timer_manager.lock().await;
            let full_state = timer.get_full_state();
            assert_eq!(full_state.status, TimerStatus::OnBreak);
            assert!(full_state.is_fullscreen_break);
            assert!(full_state.remaining_seconds < 300);
            full_state.remaining_seconds
        };

        let mut runtime_effect_shell = RecordingRuntimeEffectShell::default();
        let result = start_work_for_tests(
            state.as_ref(),
            timer_settings(
                1500,
                300,
                900,
                4,
                PostBreakTransitionPolicy::RequireManualContinue,
            ),
            TimerStartSource::ManualOutsideWindow,
            &mut runtime_effect_shell,
        )
        .await;

        assert_command_result_public_error(result, "invalid_state_transition", &["start work"]);

        let remaining_after_rejection = {
            let timer = state.timer_manager.lock().await;
            let full_state = timer.get_full_state();
            assert_eq!(full_state.status, TimerStatus::OnBreak);
            assert!(full_state.is_fullscreen_break);
            assert_eq!(full_state.remaining_seconds, remaining_after_first_tick);
            full_state.remaining_seconds
        };

        tokio::time::advance(StdDuration::from_secs(3)).await;
        tokio::task::yield_now().await;

        let timer = state.timer_manager.lock().await;
        let full_state = timer.get_full_state();
        assert_eq!(full_state.status, TimerStatus::OnBreak);
        assert!(full_state.is_fullscreen_break);
        assert!(full_state.remaining_seconds < remaining_after_rejection);
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
    async fn topology_change_does_not_replay_past_end_boundary() {
        let state = build_test_state_without_persistence_path().await;
        sync_auto_run_policies_for_test(
            &state,
            make_enabled_daytime_monday_config(),
            PostBreakTransitionPolicy::RequireManualContinue,
        )
        .await;
        start_auto_run_eligible_work_session(&state).await;

        let mut scheduler_runtime =
            SchedulerTickTestRuntime::from_state(&state, local_datetime(2025, 1, 6, 17, 59, 50));

        sync_auto_run_policies_for_test(
            &state,
            monday_and_tuesday_config("09:00", "18:00"),
            PostBreakTransitionPolicy::RequireManualContinue,
        )
        .await;
        scheduler_runtime
            .run_tick(&state, local_datetime(2025, 1, 6, 18, 30, 0))
            .await;

        assert_eq!(
            complete_work_session_reason(&state).await,
            SessionTransitionReason::WorkToBreak
        );
    }

    #[tokio::test]
    async fn topology_change_keeps_future_end_boundary_active() {
        let state = build_test_state_without_persistence_path().await;
        sync_auto_run_policies_for_test(
            &state,
            make_enabled_daytime_monday_config(),
            PostBreakTransitionPolicy::RequireManualContinue,
        )
        .await;
        start_auto_run_eligible_work_session(&state).await;

        let mut scheduler_runtime =
            SchedulerTickTestRuntime::from_state(&state, local_datetime(2025, 1, 6, 17, 59, 50));

        sync_auto_run_policies_for_test(
            &state,
            monday_and_tuesday_config("09:00", "18:00"),
            PostBreakTransitionPolicy::RequireManualContinue,
        )
        .await;
        scheduler_runtime
            .run_tick(&state, local_datetime(2025, 1, 6, 18, 30, 0))
            .await;
        scheduler_runtime
            .run_tick(&state, local_datetime(2025, 1, 7, 18, 0, 10))
            .await;

        assert_eq!(
            complete_work_session_reason(&state).await,
            SessionTransitionReason::ScheduledStopExecuted
        );
    }

    #[tokio::test]
    async fn policy_only_change_does_not_suppress_arriving_end_boundary() {
        let state = build_test_state_without_persistence_path().await;
        sync_auto_run_policies_for_test(
            &state,
            make_enabled_daytime_monday_config(),
            PostBreakTransitionPolicy::RequireManualContinue,
        )
        .await;
        start_auto_run_eligible_work_session(&state).await;

        let mut scheduler_runtime =
            SchedulerTickTestRuntime::from_state(&state, local_datetime(2025, 1, 6, 17, 59, 50));

        sync_auto_run_policies_for_test(
            &state,
            make_enabled_daytime_monday_config(),
            PostBreakTransitionPolicy::StartNextWorkAutomatically,
        )
        .await;
        scheduler_runtime
            .run_tick(&state, local_datetime(2025, 1, 6, 18, 0, 10))
            .await;

        assert_eq!(
            complete_work_session_reason(&state).await,
            SessionTransitionReason::ScheduledStopExecuted
        );
    }

    #[tokio::test(start_paused = true)]
    async fn test_resolve_post_break_decision_continue_command_side_effects() {
        let state = Arc::new(build_test_state_without_persistence_path().await);
        prepare_post_break_decision_pending_state(&state).await;
        install_timer_countdown_task(&state).await;
        let mut runtime_effect_shell = RecordingCommandRuntimeEffectShell::default();

        resolve_post_break_decision_with_runtime_effects(
            &state,
            PostBreakDecisionAction::Continue,
            &mut runtime_effect_shell,
        )
        .await
        .expect("continue decision should succeed");

        assert_tick_emit_and_tray_match(&runtime_effect_shell);
        assert_event_once(&runtime_effect_shell, RuntimeEffectEvent::EmitTick);
        assert_event_once(&runtime_effect_shell, RuntimeEffectEvent::UpdateTray);
        assert_event_once(&runtime_effect_shell, RuntimeEffectEvent::SpawnTimerTask);
        assert_event_once(&runtime_effect_shell, RuntimeEffectEvent::Persist);
        assert_event_absent(
            &runtime_effect_shell,
            RuntimeEffectEvent::RequestFullscreenBreakExit,
        );
        tokio::time::advance(StdDuration::from_secs(3)).await;
        tokio::task::yield_now().await;

        let timer = state.timer_manager.lock().await;
        let timer_state = timer.get_state();
        assert_eq!(timer_state.status, TimerStatus::Working);
        assert_eq!(timer_state.session_type, SessionType::Work);
        assert_eq!(timer_state.remaining_seconds, 1500);
        assert!(!timer_state.post_break_decision_pending);
        assert_eq!(
            runtime_effect_shell.emitted_tick_snapshots[0].status,
            TimerStatus::Working
        );
    }

    #[tokio::test]
    async fn test_resolve_post_break_decision_continue_after_restore_starts_work() {
        let state = build_test_state_without_persistence_path().await;
        apply_restored_post_break_decision_pending_state(&state).await;
        let mut runtime_effect_shell = RecordingCommandRuntimeEffectShell::default();

        resolve_post_break_decision_with_runtime_effects(
            &state,
            PostBreakDecisionAction::Continue,
            &mut runtime_effect_shell,
        )
        .await
        .expect("restored continue decision should succeed");

        assert_tick_emit_and_tray_match(&runtime_effect_shell);
        assert_event_once(&runtime_effect_shell, RuntimeEffectEvent::EmitTick);
        assert_event_once(&runtime_effect_shell, RuntimeEffectEvent::UpdateTray);
        assert_event_once(&runtime_effect_shell, RuntimeEffectEvent::SpawnTimerTask);
        assert_event_once(&runtime_effect_shell, RuntimeEffectEvent::Persist);

        let timer = state.timer_manager.lock().await;
        let timer_state = timer.get_state();
        assert_eq!(timer_state.status, TimerStatus::Working);
        assert_eq!(timer_state.session_type, SessionType::Work);
        assert_eq!(timer_state.remaining_seconds, 1500);
        assert!(!timer_state.post_break_decision_pending);
    }

    #[tokio::test]
    async fn test_resolve_post_break_decision_stop_after_restore_returns_to_idle() {
        let state = build_test_state_without_persistence_path().await;
        apply_restored_post_break_decision_pending_state(&state).await;
        let mut runtime_effect_shell = RecordingCommandRuntimeEffectShell::default();

        resolve_post_break_decision_with_runtime_effects(
            &state,
            PostBreakDecisionAction::StopCycle,
            &mut runtime_effect_shell,
        )
        .await
        .expect("restored stop decision should succeed");

        assert_tick_emit_and_tray_match(&runtime_effect_shell);
        assert_event_once(&runtime_effect_shell, RuntimeEffectEvent::EmitTick);
        assert_event_once(&runtime_effect_shell, RuntimeEffectEvent::UpdateTray);
        assert_event_once(&runtime_effect_shell, RuntimeEffectEvent::Persist);
        assert_event_absent(&runtime_effect_shell, RuntimeEffectEvent::SpawnTimerTask);

        let timer = state.timer_manager.lock().await;
        let timer_state = timer.get_state();
        assert_eq!(timer_state.status, TimerStatus::Idle);
        assert_eq!(timer_state.session_type, SessionType::Work);
        assert_eq!(timer_state.remaining_seconds, 1500);
        assert!(!timer_state.post_break_decision_pending);
    }

    #[tokio::test]
    async fn test_resolve_post_break_decision_stop_command_side_effects() {
        let state = build_test_state_without_persistence_path().await;
        prepare_post_break_decision_pending_state(&state).await;
        let mut runtime_effect_shell = RecordingCommandRuntimeEffectShell::default();

        resolve_post_break_decision_with_runtime_effects(
            &state,
            PostBreakDecisionAction::StopCycle,
            &mut runtime_effect_shell,
        )
        .await
        .expect("stop decision should succeed");

        assert_tick_emit_and_tray_match(&runtime_effect_shell);
        assert_event_once(&runtime_effect_shell, RuntimeEffectEvent::EmitTick);
        assert_event_once(&runtime_effect_shell, RuntimeEffectEvent::UpdateTray);
        assert_event_once(&runtime_effect_shell, RuntimeEffectEvent::Persist);
        assert_event_absent(&runtime_effect_shell, RuntimeEffectEvent::SpawnTimerTask);
        assert_event_absent(
            &runtime_effect_shell,
            RuntimeEffectEvent::RequestFullscreenBreakExit,
        );

        let timer = state.timer_manager.lock().await;
        assert_eq!(timer.status(), TimerStatus::Idle);
        assert!(!timer.get_state().post_break_decision_pending);
        assert_eq!(
            runtime_effect_shell.emitted_tick_snapshots[0].status,
            TimerStatus::Idle
        );
    }

    #[tokio::test]
    async fn test_emergency_stop_command_side_effects() {
        let fullscreen_state = build_test_state_without_persistence_path().await;
        prepare_fullscreen_break_state(&fullscreen_state).await;
        let old_task_completed = install_cancellable_timer_task(&fullscreen_state).await;
        let mut fullscreen_runtime_effect_shell = RecordingCommandRuntimeEffectShell::default();

        emergency_stop_with_runtime_effects(
            &fullscreen_state,
            &mut fullscreen_runtime_effect_shell,
        )
        .await
        .expect("emergency stop should succeed");

        assert_tick_emit_and_tray_match(&fullscreen_runtime_effect_shell);
        assert_event_once(
            &fullscreen_runtime_effect_shell,
            RuntimeEffectEvent::EmitTick,
        );
        assert_event_once(
            &fullscreen_runtime_effect_shell,
            RuntimeEffectEvent::UpdateTray,
        );
        assert_event_once(
            &fullscreen_runtime_effect_shell,
            RuntimeEffectEvent::RequestFullscreenBreakExit,
        );
        assert_event_once(
            &fullscreen_runtime_effect_shell,
            RuntimeEffectEvent::Persist,
        );
        assert_event_absent(
            &fullscreen_runtime_effect_shell,
            RuntimeEffectEvent::SpawnTimerTask,
        );
        assert!(old_task_completed.load(Ordering::Acquire));

        {
            let fullscreen_timer = fullscreen_state.timer_manager.lock().await;
            assert_eq!(fullscreen_timer.status(), TimerStatus::Idle);
            assert!(!fullscreen_timer.get_state().is_fullscreen_break);
        }
        assert_eq!(
            fullscreen_runtime_effect_shell.emitted_tick_snapshots[0].status,
            TimerStatus::Idle
        );
        assert!(!fullscreen_runtime_effect_shell.emitted_tick_snapshots[0].is_fullscreen_break);

        let non_fullscreen_state = build_test_state_without_persistence_path().await;
        {
            let mut timer = non_fullscreen_state.timer_manager.lock().await;
            timer
                .start_work_with_source(
                    timer_settings(
                        1500,
                        300,
                        900,
                        4,
                        PostBreakTransitionPolicy::RequireManualContinue,
                    ),
                    TimerStartSource::ManualOutsideWindow,
                )
                .expect("work session should start");
        }
        let mut non_fullscreen_runtime_effect_shell = RecordingCommandRuntimeEffectShell::default();

        emergency_stop_with_runtime_effects(
            &non_fullscreen_state,
            &mut non_fullscreen_runtime_effect_shell,
        )
        .await
        .expect("emergency stop should succeed");

        assert_tick_emit_and_tray_match(&non_fullscreen_runtime_effect_shell);
        assert_event_once(
            &non_fullscreen_runtime_effect_shell,
            RuntimeEffectEvent::EmitTick,
        );
        assert_event_once(
            &non_fullscreen_runtime_effect_shell,
            RuntimeEffectEvent::UpdateTray,
        );
        assert_event_once(
            &non_fullscreen_runtime_effect_shell,
            RuntimeEffectEvent::Persist,
        );
        assert_event_absent(
            &non_fullscreen_runtime_effect_shell,
            RuntimeEffectEvent::RequestFullscreenBreakExit,
        );
    }

    #[tokio::test]
    async fn test_sync_timer_duration_settings_command_side_effects() {
        let state = build_test_state_without_persistence_path().await;
        let mut runtime_effect_shell = RecordingCommandRuntimeEffectShell::default();
        let expected_work_duration_secs = command_work_duration_secs();

        sync_timer_duration_settings_with_runtime_effects(
            &state,
            expected_work_duration_secs,
            command_short_break_duration_secs(),
            command_long_break_duration_secs(),
            command_long_break_interval(),
            &mut runtime_effect_shell,
        )
        .await
        .expect("duration settings sync should succeed");

        assert_tick_emit_and_tray_match(&runtime_effect_shell);
        assert_event_once(&runtime_effect_shell, RuntimeEffectEvent::EmitTick);
        assert_event_once(&runtime_effect_shell, RuntimeEffectEvent::UpdateTray);
        assert_event_once(&runtime_effect_shell, RuntimeEffectEvent::Persist);
        assert_event_absent(&runtime_effect_shell, RuntimeEffectEvent::SpawnTimerTask);
        assert_event_absent(
            &runtime_effect_shell,
            RuntimeEffectEvent::RequestFullscreenBreakExit,
        );

        let timer = state.timer_manager.lock().await;
        assert_eq!(timer.status(), TimerStatus::Idle);
        assert_eq!(
            timer.get_state().settings.work_duration_secs,
            expected_work_duration_secs
        );
        assert_eq!(
            runtime_effect_shell.emitted_tick_snapshots[0].remaining_seconds,
            expected_work_duration_secs.get()
        );
    }

    #[tokio::test]
    async fn test_sync_automation_policies_command_side_effects() {
        let state = build_test_state_without_persistence_path().await;
        let mut runtime_effect_shell = RecordingCommandRuntimeEffectShell::default();

        sync_automation_policies_with_runtime_effects(
            &state,
            false,
            WeeklyAutoRunConfig::default(),
            PostBreakTransitionPolicy::StartNextWorkAutomatically,
            &mut runtime_effect_shell,
        )
        .await
        .expect("automation policy sync should succeed");

        assert_tick_emit_and_tray_match(&runtime_effect_shell);
        assert_event_once(&runtime_effect_shell, RuntimeEffectEvent::EmitTick);
        assert_event_once(&runtime_effect_shell, RuntimeEffectEvent::UpdateTray);
        assert_event_once(&runtime_effect_shell, RuntimeEffectEvent::Persist);
        assert_event_absent(&runtime_effect_shell, RuntimeEffectEvent::SpawnTimerTask);
        assert_event_absent(
            &runtime_effect_shell,
            RuntimeEffectEvent::RequestFullscreenBreakExit,
        );

        let timer = state.timer_manager.lock().await;
        assert_eq!(
            timer.get_state().settings.post_break_transition_policy,
            PostBreakTransitionPolicy::StartNextWorkAutomatically
        );
    }

    #[tokio::test]
    async fn test_command_timer_side_effects_preserve_persistence_failure_policy() {
        let continue_state = build_test_state_with_injected_persistence_error().await;
        prepare_post_break_decision_pending_state(&continue_state).await;
        let mut continue_runtime_effect_shell = RecordingCommandRuntimeEffectShell::default();

        let continue_result = resolve_post_break_decision_with_runtime_effects(
            &continue_state,
            PostBreakDecisionAction::Continue,
            &mut continue_runtime_effect_shell,
        )
        .await;

        assert_persistence_save_error(&continue_result);
        assert_tick_emit_and_tray_match(&continue_runtime_effect_shell);
        assert_event_once(
            &continue_runtime_effect_shell,
            RuntimeEffectEvent::SpawnTimerTask,
        );
        assert_event_once(&continue_runtime_effect_shell, RuntimeEffectEvent::Persist);

        let emergency_state = build_test_state_with_injected_persistence_error().await;
        prepare_fullscreen_break_state(&emergency_state).await;
        let mut emergency_runtime_effect_shell = RecordingCommandRuntimeEffectShell::default();

        let emergency_result = emergency_stop_with_runtime_effects(
            &emergency_state,
            &mut emergency_runtime_effect_shell,
        )
        .await;

        assert_persistence_save_error(&emergency_result);
        assert_tick_emit_and_tray_match(&emergency_runtime_effect_shell);
        assert_event_once(
            &emergency_runtime_effect_shell,
            RuntimeEffectEvent::RequestFullscreenBreakExit,
        );
        assert_event_once(&emergency_runtime_effect_shell, RuntimeEffectEvent::Persist);

        let duration_state = build_test_state_with_injected_persistence_error().await;
        let mut duration_runtime_effect_shell = RecordingCommandRuntimeEffectShell::default();

        let duration_result = sync_timer_duration_settings_with_runtime_effects(
            &duration_state,
            command_work_duration_secs(),
            command_short_break_duration_secs(),
            command_long_break_duration_secs(),
            command_long_break_interval(),
            &mut duration_runtime_effect_shell,
        )
        .await;

        assert_persistence_save_error(&duration_result);
        assert_tick_emit_and_tray_match(&duration_runtime_effect_shell);
        assert_event_once(&duration_runtime_effect_shell, RuntimeEffectEvent::Persist);

        let automation_state = build_test_state_with_injected_persistence_error().await;
        let mut automation_runtime_effect_shell = RecordingCommandRuntimeEffectShell::default();

        let automation_result = sync_automation_policies_with_runtime_effects(
            &automation_state,
            false,
            WeeklyAutoRunConfig::default(),
            PostBreakTransitionPolicy::StartNextWorkAutomatically,
            &mut automation_runtime_effect_shell,
        )
        .await;

        assert_persistence_save_error(&automation_result);
        assert_tick_emit_and_tray_match(&automation_runtime_effect_shell);
        assert_event_once(
            &automation_runtime_effect_shell,
            RuntimeEffectEvent::Persist,
        );
    }

    #[tokio::test]
    async fn test_timer_command_persistence_failure_returns_public_command_error() {
        let state = build_test_state_with_injected_persistence_error().await;
        prepare_post_break_decision_pending_state(&state).await;
        let mut runtime_effect_shell = RecordingCommandRuntimeEffectShell::default();

        let result = resolve_post_break_decision_with_runtime_effects(
            &state,
            PostBreakDecisionAction::Continue,
            &mut runtime_effect_shell,
        )
        .await;

        assert_command_result_public_error(
            result,
            "persistence_error",
            &[
                "failed to serialize persisted timer state",
                "expected value",
            ],
        );
    }

    #[tokio::test]
    async fn test_timer_command_persistence_task_join_returns_public_command_error() {
        let state = build_test_state_with_panicking_persistence_hook().await;
        prepare_post_break_decision_pending_state(&state).await;
        let mut runtime_effect_shell = RecordingCommandRuntimeEffectShell::default();

        let result = resolve_post_break_decision_with_runtime_effects(
            &state,
            PostBreakDecisionAction::Continue,
            &mut runtime_effect_shell,
        )
        .await;

        assert_command_result_public_error(
            result,
            "persistence_error",
            &[
                "simulated persistence worker panic",
                "C:\\Users\\nak",
                "secret",
            ],
        );
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

        let result = resolve_boot_auto_start_prompt_start_contract(
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
    async fn test_start_work_persistence_failure_does_not_roll_back_started_session() {
        let temp_dir = create_temp_dir("start-work-save-fails-keeps-started-session");
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

        assert_persistence_save_error(&result);

        let full_state = {
            let timer_manager = state.timer_manager.lock().await;
            timer_manager.get_full_state()
        };

        assert_eq!(full_state.status, TimerStatus::Working);
        assert_eq!(full_state.session_type, SessionType::Work);
        assert_eq!(full_state.remaining_seconds, 1500);

        cleanup_temp_dir(&temp_dir);
    }

    #[tokio::test]
    async fn test_boot_prompt_start_contract_returns_save_error_after_start_state_is_resolved() {
        let state = build_test_state_with_injected_persistence_error().await;
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

        let result = resolve_boot_auto_start_prompt_start_contract(
            &state,
            timer_settings,
            &mut runtime_effect_shell,
        )
        .await;

        assert!(matches!(result, Err(PomoliceError::PersistenceSave(_))));
        assert_eq!(
            state.timer_manager.lock().await.status(),
            TimerStatus::Working
        );
        assert_eq!(
            *state
                .boot_prompt_runtime_state
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner),
            BootPromptRuntimeState::ResolvedStart
        );
        assert!(state.auto_start_boundary_enabled.load(Ordering::Acquire));
        assert!(runtime_effect_shell
            .events()
            .contains(&RuntimeEffectEvent::EmitWorkStarted));
    }

    #[tokio::test]
    async fn test_boot_prompt_start_persistence_failure_records_runtime_effects() {
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

        let result = resolve_boot_auto_start_prompt_start_contract(
            &state,
            timer_settings,
            &mut runtime_effect_shell,
        )
        .await;

        assert!(matches!(result, Err(PomoliceError::PersistenceSave(_))));
        assert!(runtime_effect_shell
            .events()
            .contains(&RuntimeEffectEvent::EmitTick));
        assert!(runtime_effect_shell
            .events()
            .contains(&RuntimeEffectEvent::UpdateTray));
        assert!(runtime_effect_shell
            .events()
            .contains(&RuntimeEffectEvent::EmitWorkStarted));
        assert!(runtime_effect_shell
            .events()
            .contains(&RuntimeEffectEvent::SpawnTimerTask));
        assert!(runtime_effect_shell
            .events()
            .contains(&RuntimeEffectEvent::Persist));

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
