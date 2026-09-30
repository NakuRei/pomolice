use chrono::{DateTime, Duration as ChronoDuration, Local};

use super::boot_auto_start::initial_boot_auto_start_offer;
use super::phase::{BreakKind, BreakSessionState, TimerPhase};
use super::schedule::ScheduleState;
use super::TimerCore;
use crate::ipc::settings::SettingsFileStatus;
use crate::logging_event::{log_warn_event, BackendLogEvent};
use crate::settings::UserSettings;
use crate::types::{
    LongBreakDurationSecs, PersistedState, ScheduleStopState, SessionDurationSecs, SessionType,
    ShortBreakDurationSecs, TimerStatus, WorkDurationSecs,
};

const RESTORE_BOUNDARY_LOOKBACK_DAYS: i64 = 7;

impl TimerCore {
    /// Builds the core from the persisted timer state (or an idle timer) and
    /// the settings in effect, as of `now`. A restored work session that has
    /// already run out gets `ends_at == now` and is finalized by
    /// `Event::Started`.
    pub(crate) fn restore(
        persisted: Option<PersistedState>,
        settings: UserSettings,
        settings_file_status: SettingsFileStatus,
        now: DateTime<Local>,
    ) -> Self {
        let Some(persisted) = persisted else {
            return Self::idle(settings, settings_file_status, now);
        };

        let saved_at_local = persisted.saved_at.with_timezone(&Local);
        let elapsed_seconds_since_save =
            u64::try_from((now - saved_at_local).num_seconds().max(0)).unwrap_or_default();
        let normalized = normalize_persisted(persisted, &settings);
        let remaining_seconds = restored_remaining_seconds(&normalized, elapsed_seconds_since_save);
        let duration_secs = normalized.session_duration_secs;
        let status = normalized.status;
        let schedule_stop_state = match status {
            TimerStatus::Idle => ScheduleStopState::NotControlledBySchedule,
            TimerStatus::Working | TimerStatus::OnBreak => normalized.schedule_stop_state,
        };
        let phase = match (status, break_kind_of(normalized.session_type)) {
            (TimerStatus::Working, _) => TimerPhase::Working {
                ends_at: now
                    + ChronoDuration::seconds(i64::try_from(remaining_seconds).unwrap_or(0)),
                duration_secs,
            },
            // Break-scoped values are not persisted, so a restored break starts fresh.
            (TimerStatus::OnBreak, Some(break_kind)) => TimerPhase::OnBreak(
                BreakSessionState::starting(break_kind, duration_secs, remaining_seconds),
            ),
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
            boot_auto_start_offer: initial_boot_auto_start_offer(phase.status(), &settings, now),
            phase,
            current_cycle: normalized.current_cycle,
            completed_pomodoros: normalized.completed_pomodoros,
            settings,
            settings_file_status,
            last_transition_at: None,
            schedule: ScheduleState {
                stop_state: schedule_stop_state,
                last_boundary_check_local,
                starts_blocked_until_idle: is_running,
                last_published_active_window: None,
            },
        }
    }

    fn idle(
        settings: UserSettings,
        settings_file_status: SettingsFileStatus,
        now: DateTime<Local>,
    ) -> Self {
        Self {
            boot_auto_start_offer: initial_boot_auto_start_offer(TimerStatus::Idle, &settings, now),
            phase: TimerPhase::Idle {
                post_break_decision_pending: false,
            },
            current_cycle: 0,
            completed_pomodoros: 0,
            settings,
            settings_file_status,
            last_transition_at: None,
            schedule: ScheduleState {
                stop_state: ScheduleStopState::NotControlledBySchedule,
                last_boundary_check_local: now,
                starts_blocked_until_idle: false,
                last_published_active_window: None,
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

/// Whether the session length is one the settings could have given a session
/// of this kind.
fn session_duration_fits_session_type(persisted: &PersistedState) -> bool {
    let duration_secs = persisted.session_duration_secs.get();
    match persisted.session_type {
        SessionType::Work => WorkDurationSecs::try_new(duration_secs).is_ok(),
        SessionType::ShortBreak => ShortBreakDurationSecs::try_new(duration_secs).is_ok(),
        SessionType::LongBreak => LongBreakDurationSecs::try_new(duration_secs).is_ok(),
    }
}

fn configured_session_duration(
    settings: &UserSettings,
    session_type: SessionType,
) -> SessionDurationSecs {
    match session_type {
        SessionType::Work => settings.work_duration_secs.into(),
        SessionType::ShortBreak => settings.short_break_duration_secs.into(),
        SessionType::LongBreak => settings.long_break_duration_secs.into(),
    }
}

/// The file is user-writable, so combinations the core cannot represent are
/// normalized and reported: a session length outside its kind's range takes
/// the configured length, a pending post-break decision is only ever idle,
/// and a break must have a break kind.
fn normalize_persisted(mut persisted: PersistedState, settings: &UserSettings) -> PersistedState {
    if !session_duration_fits_session_type(&persisted) {
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
        persisted.session_duration_secs =
            configured_session_duration(settings, persisted.session_type);
    }

    let break_without_kind =
        persisted.status == TimerStatus::OnBreak && break_kind_of(persisted.session_type).is_none();
    if !persisted.post_break_decision_pending && !break_without_kind {
        return persisted;
    }

    let work_duration_secs = settings.work_duration_secs.get();
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
    persisted.session_duration_secs = settings.work_duration_secs.into();
    persisted.schedule_stop_state = ScheduleStopState::NotControlledBySchedule;
    persisted
}

fn restored_remaining_seconds(persisted: &PersistedState, elapsed_seconds_since_save: u64) -> u64 {
    // The persisted state file is user-writable, so a restored countdown can
    // carry an arbitrary value. Legitimate saves never exceed the duration the
    // session started with; cap anything larger instead of trusting it.
    let bounded_remaining_seconds = persisted
        .remaining_seconds
        .min(persisted.session_duration_secs.get());

    if persisted.status == TimerStatus::Working {
        bounded_remaining_seconds.saturating_sub(elapsed_seconds_since_save)
    } else {
        bounded_remaining_seconds
    }
}
