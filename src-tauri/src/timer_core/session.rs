use chrono::{DateTime, Duration as ChronoDuration, Local};

use super::phase::{BreakKind, BreakSessionState, TimerPhase};
use super::{BreakLockTarget, Effect, TimerCore};
use crate::error::{PomoliceError, Result};
use crate::ipc::responses::SessionCompletedPayload;
use crate::logging_event::{log_info_event, log_warn_event, BackendLogEvent};
use crate::settings::UserSettings;
use crate::types::{
    PostBreakDecisionAction, PostBreakTransitionPolicy, ScheduleStopState, SessionDurationSecs,
    SessionTransitionReason, SessionType, TimerStatus, WorkStartOrigin,
};

const MANUAL_START_RATE_LIMIT: ChronoDuration = ChronoDuration::milliseconds(300);

enum FinishedSession {
    Work,
    Break,
}

impl TimerCore {
    pub(super) fn apply_started(&mut self, now: DateTime<Local>) -> Vec<Effect> {
        match self.phase {
            TimerPhase::Working { ends_at, .. } if ends_at <= now => {
                self.complete_work_session(now)
            }
            TimerPhase::Working { .. } => vec![self.publish(now)],
            TimerPhase::OnBreak(_) => vec![Effect::SetBreakLockTarget(BreakLockTarget::Locked)],
            TimerPhase::Idle { .. } => Vec::new(),
        }
    }

    pub(super) fn start_work_requested(&mut self, now: DateTime<Local>) -> Result<Vec<Effect>> {
        self.start_work(self.manual_start_origin(now), now)?;
        Ok(vec![self.publish(now), Effect::Persist])
    }

    pub(super) fn start_work(
        &mut self,
        origin: WorkStartOrigin,
        now: DateTime<Local>,
    ) -> Result<()> {
        if self.status() != TimerStatus::Idle {
            log_warn_event(
                BackendLogEvent::TimerStartRejected,
                &[
                    ("reason", "invalid_state_transition".to_string()),
                    ("current_state", format!("{:?}", self.status())),
                ],
            );
            return Err(PomoliceError::InvalidStateTransition {
                action: "start work".to_string(),
                current_state: self.status(),
            });
        }

        if origin != WorkStartOrigin::ScheduleStartBoundary {
            if let Some(remaining_ms) = self.rate_limit_remaining_ms(now) {
                log_warn_event(
                    BackendLogEvent::TimerStartRejected,
                    &[
                        ("reason", "rate_limited".to_string()),
                        ("current_state", format!("{:?}", self.status())),
                    ],
                );
                return Err(PomoliceError::RateLimited { remaining_ms });
            }
        }

        let work_duration_secs = self.settings.work_duration_secs.into();
        self.schedule.stop_state = match origin {
            WorkStartOrigin::ManualOutsideScheduleWindow => {
                ScheduleStopState::NotControlledBySchedule
            }
            WorkStartOrigin::ManualInsideScheduleWindow
            | WorkStartOrigin::ScheduleStartBoundary => ScheduleStopState::WaitingForScheduleEnd,
        };
        if self.current_cycle == 0 {
            self.current_cycle = 1;
        }
        self.begin_work_session(work_duration_secs, now);

        log_info_event(
            BackendLogEvent::TimerWorkSessionStarted,
            &[
                ("duration_secs", work_duration_secs.get().to_string()),
                ("work_start_origin", format!("{origin:?}")),
            ],
        );
        Ok(())
    }

    fn begin_work_session(&mut self, duration_secs: SessionDurationSecs, now: DateTime<Local>) {
        self.phase = TimerPhase::Working {
            ends_at: now
                + ChronoDuration::seconds(i64::try_from(duration_secs.get()).unwrap_or(i64::MAX)),
            duration_secs,
        };
        self.last_transition_at = Some(now);
    }

    fn rate_limit_remaining_ms(&self, now: DateTime<Local>) -> Option<u64> {
        let last_transition_at = self.last_transition_at?;
        let elapsed = now - last_transition_at;
        if elapsed < ChronoDuration::zero() || elapsed >= MANUAL_START_RATE_LIMIT {
            return None;
        }
        u64::try_from((MANUAL_START_RATE_LIMIT - elapsed).num_milliseconds()).ok()
    }

    pub(super) fn resolve_post_break_decision(
        &mut self,
        action: PostBreakDecisionAction,
        now: DateTime<Local>,
    ) -> Result<Vec<Effect>> {
        if !self.post_break_decision_pending() {
            log_warn_event(
                BackendLogEvent::TimerTransitionRejected,
                &[
                    ("action", "resolve_post_break_decision".to_string()),
                    ("current_state", format!("{:?}", self.status())),
                    ("pending", "false".to_string()),
                ],
            );
            return Err(PomoliceError::InvalidStateTransition {
                action: "resolve post-break decision".to_string(),
                current_state: self.status(),
            });
        }

        let work_duration_secs = self.settings.work_duration_secs.into();
        self.schedule.stop_state = ScheduleStopState::NotControlledBySchedule;
        match action {
            PostBreakDecisionAction::Continue => {
                self.current_cycle += 1;
                self.begin_work_session(work_duration_secs, now);
            }
            PostBreakDecisionAction::StopCycle => {
                self.enter_idle(false, now);
            }
        }

        log_info_event(
            BackendLogEvent::TimerTransitionCompleted,
            &[
                (
                    "transition_stage",
                    "resolve_post_break_decision".to_string(),
                ),
                (
                    "transition_reason",
                    "manual_post_break_decision".to_string(),
                ),
                ("action", format!("{action:?}")),
                (
                    "policy",
                    format!("{:?}", self.settings.post_break_transition_policy),
                ),
                (
                    "post_break_decision_pending",
                    self.post_break_decision_pending().to_string(),
                ),
                (
                    "schedule_stop_state",
                    format!("{:?}", self.schedule.stop_state),
                ),
            ],
        );
        Ok(vec![self.publish(now), Effect::Persist])
    }

    fn enter_idle(&mut self, post_break_decision_pending: bool, now: DateTime<Local>) {
        self.phase = TimerPhase::Idle {
            post_break_decision_pending,
        };
        self.last_transition_at = Some(now);
    }

    pub(super) fn emergency_stop(&mut self, now: DateTime<Local>) -> Vec<Effect> {
        self.boot_auto_start_offer = None;
        self.schedule.stop_state = ScheduleStopState::NotControlledBySchedule;
        self.enter_idle(false, now);

        vec![
            Effect::SetBreakLockTarget(BreakLockTarget::Unlocked),
            self.publish(now),
            Effect::Persist,
        ]
    }

    /// Countdown step for the current session. Work derives from the wall
    /// clock; breaks count ticks, so seconds the app did not enforce are not
    /// consumed.
    pub(super) fn count_down(&mut self, now: DateTime<Local>) -> Vec<Effect> {
        let finished = match &mut self.phase {
            TimerPhase::Idle { .. } => return Vec::new(),
            TimerPhase::Working { ends_at, .. } => {
                (*ends_at <= now).then_some(FinishedSession::Work)
            }
            TimerPhase::OnBreak(current_break) => {
                current_break.remaining_seconds = current_break.remaining_seconds.saturating_sub(1);
                (current_break.remaining_seconds == 0).then_some(FinishedSession::Break)
            }
        };

        let mut effects = vec![self.publish(now)];
        match finished {
            Some(FinishedSession::Work) => effects.extend(self.complete_work_session(now)),
            Some(FinishedSession::Break) => effects.extend(self.complete_break_session(now)),
            None => {}
        }
        effects
    }

    fn complete_work_session(&mut self, now: DateTime<Local>) -> Vec<Effect> {
        if self.scheduled_stop_is_due() {
            let payload = self.execute_scheduled_stop(SessionType::Work, now);
            return vec![
                Effect::EmitSessionCompleted(payload),
                Effect::EmitAutoRunStopped,
                self.publish(now),
                Effect::Persist,
            ];
        }

        self.completed_pomodoros += 1;
        let break_kind =
            break_kind_for_cycle(self.current_cycle, self.settings.long_break_interval.get());
        let break_duration_secs = break_duration(&self.settings, break_kind);
        self.phase = TimerPhase::OnBreak(BreakSessionState::starting(
            break_kind,
            break_duration_secs,
            break_duration_secs.get(),
        ));
        self.last_transition_at = Some(now);

        let payload = SessionCompletedPayload::work_to_break(break_kind.session_type());
        self.log_transition("transition_to_break", payload.transition_reason);
        vec![
            Effect::EmitSessionCompleted(payload),
            Effect::SetBreakLockTarget(BreakLockTarget::Locked),
            self.publish(now),
            Effect::Persist,
        ]
    }

    fn complete_break_session(&mut self, now: DateTime<Local>) -> Vec<Effect> {
        let completed_session_type = self.session_type();
        let is_away = self
            .current_break()
            .is_some_and(|current_break| current_break.attendance.is_away());

        if self.scheduled_stop_is_due() {
            let payload = self.execute_scheduled_stop(completed_session_type, now);
            return vec![
                Effect::EmitSessionCompleted(payload),
                Effect::EmitAutoRunStopped,
                Effect::SetBreakLockTarget(BreakLockTarget::Unlocked),
                self.publish(now),
                Effect::Persist,
            ];
        }

        let payload = if is_away
            || self.settings.post_break_transition_policy
                == PostBreakTransitionPolicy::RequireManualContinue
        {
            self.finish_break_waiting_for_decision(completed_session_type, now)
        } else {
            self.finish_break_starting_next_work(completed_session_type, now)
        };
        vec![
            Effect::EmitSessionCompleted(payload),
            Effect::SetBreakLockTarget(BreakLockTarget::Unlocked),
            self.publish(now),
            Effect::Persist,
        ]
    }

    fn finish_break_waiting_for_decision(
        &mut self,
        completed_session_type: SessionType,
        now: DateTime<Local>,
    ) -> SessionCompletedPayload {
        if completed_session_type == SessionType::LongBreak {
            self.current_cycle = 0;
        }
        self.schedule.stop_state = ScheduleStopState::NotControlledBySchedule;
        self.enter_idle(true, now);

        let payload = SessionCompletedPayload::break_waiting_decision(completed_session_type);
        self.log_transition("transition_to_idle_after_break", payload.transition_reason);
        payload
    }

    fn finish_break_starting_next_work(
        &mut self,
        completed_session_type: SessionType,
        now: DateTime<Local>,
    ) -> SessionCompletedPayload {
        if completed_session_type == SessionType::LongBreak {
            self.current_cycle = 0;
        }
        self.current_cycle += 1;
        // Schedule control carries over an automatic break-to-work transition.
        self.begin_work_session(self.settings.work_duration_secs.into(), now);

        let payload = SessionCompletedPayload::break_auto_started(completed_session_type);
        self.log_transition(
            "transition_break_to_work_automatically",
            payload.transition_reason,
        );
        payload
    }

    fn execute_scheduled_stop(
        &mut self,
        completed_session_type: SessionType,
        now: DateTime<Local>,
    ) -> SessionCompletedPayload {
        if completed_session_type == SessionType::Work {
            self.completed_pomodoros += 1;
        }
        if completed_session_type == SessionType::LongBreak {
            self.current_cycle = 0;
        }
        self.schedule.stop_state = ScheduleStopState::NotControlledBySchedule;
        self.enter_idle(false, now);

        let payload = SessionCompletedPayload::scheduled_stop_executed(completed_session_type);
        self.log_transition(
            "execute_scheduled_stop_transition",
            payload.transition_reason,
        );
        payload
    }

    fn log_transition(
        &self,
        transition_stage: &'static str,
        transition_reason: SessionTransitionReason,
    ) {
        log_info_event(
            BackendLogEvent::TimerTransitionCompleted,
            &[
                ("transition_stage", transition_stage.to_string()),
                ("transition_reason", format!("{transition_reason:?}")),
                (
                    "policy",
                    format!("{:?}", self.settings.post_break_transition_policy),
                ),
                (
                    "post_break_decision_pending",
                    self.post_break_decision_pending().to_string(),
                ),
                (
                    "schedule_stop_state",
                    format!("{:?}", self.schedule.stop_state),
                ),
            ],
        );
    }
}

fn break_duration(settings: &UserSettings, kind: BreakKind) -> SessionDurationSecs {
    match kind {
        BreakKind::Short => settings.short_break_duration_secs.into(),
        BreakKind::Long => settings.long_break_duration_secs.into(),
    }
}

/// The cycle counts work sessions since the last long break. Reaching the
/// interval, rather than landing on a multiple of it, keeps a long break from
/// being postponed when the interval is lowered past the current cycle.
fn break_kind_for_cycle(cycle: u32, long_break_interval: u32) -> BreakKind {
    if cycle >= long_break_interval {
        BreakKind::Long
    } else {
        BreakKind::Short
    }
}
