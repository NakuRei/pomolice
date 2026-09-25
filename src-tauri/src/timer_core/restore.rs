use chrono::{DateTime, Duration as ChronoDuration, Local};

use super::phase::{BreakKind, BreakSessionState, TimerPhase};
use super::schedule::{BootPromptGate, ScheduleState};
use super::TimerCore;
use crate::logging_event::{log_warn_event, BackendLogEvent};
use crate::types::{
    PersistedState, ScheduleStopState, SessionType, TimerSettings, TimerStatus, WeeklyAutoRunConfig,
};

const RESTORE_BOUNDARY_LOOKBACK_DAYS: i64 = 7;

impl TimerCore {
    /// Builds the core from the persisted state (or defaults) as of `now`.
    /// A restored work session that has already run out gets `ends_at == now`
    /// and is finalized by `Event::Started`.
    pub(crate) fn restore(persisted: Option<PersistedState>, now: DateTime<Local>) -> Self {
        let Some(persisted) = persisted else {
            return Self::idle(now);
        };

        let saved_at_local = persisted.saved_at.with_timezone(&Local);
        let elapsed_seconds_since_save =
            u64::try_from((now - saved_at_local).num_seconds().max(0)).unwrap_or_default();
        let normalized = normalize_persisted(persisted);
        let remaining_seconds = restored_remaining_seconds(&normalized, elapsed_seconds_since_save);
        let status = normalized.status;
        let schedule_stop_state = match status {
            TimerStatus::Idle => ScheduleStopState::NotControlledBySchedule,
            TimerStatus::Working | TimerStatus::OnBreak => normalized.schedule_stop_state,
        };
        let phase = match (status, break_kind_of(normalized.session_type)) {
            (TimerStatus::Working, _) => TimerPhase::Working {
                ends_at: now
                    + ChronoDuration::seconds(i64::try_from(remaining_seconds).unwrap_or(0)),
            },
            // Break-scoped values are not persisted, so a restored break starts fresh.
            (TimerStatus::OnBreak, Some(break_kind)) => {
                TimerPhase::OnBreak(BreakSessionState::starting(break_kind, remaining_seconds))
            }
            (TimerStatus::Idle | TimerStatus::OnBreak, _) => TimerPhase::Idle {
                post_break_decision_pending: normalized.post_break_decision_pending,
            },
        };
        let is_running = matches!(status, TimerStatus::Working | TimerStatus::OnBreak);
        let last_boundary_check_local = if is_running {
            saved_at_local.max(now - ChronoDuration::days(RESTORE_BOUNDARY_LOOKBACK_DAYS))
        } else {
            now
        };

        Self {
            phase,
            current_cycle: normalized.current_cycle,
            completed_pomodoros: normalized.completed_pomodoros,
            settings: normalized.settings,
            last_transition_at: None,
            schedule: ScheduleState {
                enabled: normalized.auto_run_feature_enabled,
                config: normalized.weekly_auto_run_config,
                stop_state: schedule_stop_state,
                last_boundary_check_local,
                starts_blocked_until_idle: is_running,
                last_published_active_window: None,
                boot_prompt_gate: BootPromptGate::WaitingForFirstFullState,
            },
        }
    }

    fn idle(now: DateTime<Local>) -> Self {
        let settings = TimerSettings::default();
        Self {
            phase: TimerPhase::Idle {
                post_break_decision_pending: false,
            },
            current_cycle: 0,
            completed_pomodoros: 0,
            settings,
            last_transition_at: None,
            schedule: ScheduleState {
                enabled: false,
                config: WeeklyAutoRunConfig::default(),
                stop_state: ScheduleStopState::NotControlledBySchedule,
                last_boundary_check_local: now,
                starts_blocked_until_idle: false,
                last_published_active_window: None,
                boot_prompt_gate: BootPromptGate::WaitingForFirstFullState,
            },
        }
    }
}

fn break_kind_of(session_type: SessionType) -> Option<BreakKind> {
    match session_type {
        SessionType::ShortBreak => Some(BreakKind::Short),
        SessionType::LongBreak => Some(BreakKind::Long),
        SessionType::Work => None,
    }
}

/// The file is user-writable, so combinations the core cannot represent are
/// normalized to an idle work preview and reported: a pending post-break
/// decision is only ever idle, and a break must have a break kind.
fn normalize_persisted(mut persisted: PersistedState) -> PersistedState {
    let break_without_kind =
        persisted.status == TimerStatus::OnBreak && break_kind_of(persisted.session_type).is_none();
    if !persisted.post_break_decision_pending && !break_without_kind {
        return persisted;
    }

    let work_duration_secs = persisted.settings.work_duration_secs.get();
    let differs = persisted.status != TimerStatus::Idle
        || persisted.session_type != SessionType::Work
        || persisted.remaining_seconds != work_duration_secs;
    if differs {
        log_warn_event(
            BackendLogEvent::StoragePersistenceRestoreNormalized,
            &[
                ("status", format!("{:?}", persisted.status)),
                ("session_type", format!("{:?}", persisted.session_type)),
                ("remaining_seconds", persisted.remaining_seconds.to_string()),
                (
                    "completed_pomodoros",
                    persisted.completed_pomodoros.to_string(),
                ),
            ],
        );
    }

    persisted.status = TimerStatus::Idle;
    persisted.session_type = SessionType::Work;
    persisted.remaining_seconds = work_duration_secs;
    persisted.schedule_stop_state = ScheduleStopState::NotControlledBySchedule;
    persisted
}

fn restored_remaining_seconds(persisted: &PersistedState, elapsed_seconds_since_save: u64) -> u64 {
    // The persisted state file is user-writable, so a restored countdown can
    // carry an arbitrary value. Legitimate saves never exceed the configured
    // session duration; cap anything larger instead of trusting it.
    let configured_session_duration_secs = match persisted.session_type {
        SessionType::Work => persisted.settings.work_duration_secs.get(),
        SessionType::ShortBreak => persisted.settings.short_break_duration_secs.get(),
        SessionType::LongBreak => persisted.settings.long_break_duration_secs.get(),
    };
    let bounded_remaining_seconds = persisted
        .remaining_seconds
        .min(configured_session_duration_secs);

    if persisted.status == TimerStatus::Working {
        bounded_remaining_seconds.saturating_sub(elapsed_seconds_since_save)
    } else {
        bounded_remaining_seconds
    }
}
