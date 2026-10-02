use chrono::{DateTime, Duration as ChronoDuration, Utc};

use super::phase::{BreakKind, BreakSessionState, TimerPhase};
use super::{BreakLockTarget, Effect, TimerCore};
use crate::error::{Result, TimerError};
use crate::settings::UserSettings;
use crate::timer_core::observation::SessionCompletedPayload;
use crate::types::{
    PostBreakDecisionAction, PostBreakTransitionPolicy, ScheduleStopState, SessionDurationSecs,
    SessionTransitionReason, SessionType, TimerStatus, WorkStartOrigin,
};

const MANUAL_START_RATE_LIMIT: ChronoDuration = ChronoDuration::milliseconds(300);

enum FinishedSession {
    Work,
    Break,
}

impl<Tz: chrono::TimeZone> TimerCore<Tz> {
    pub(super) fn start_work_requested(&mut self, now: DateTime<Utc>) -> Result<Vec<Effect>> {
        self.start_work(self.manual_start_origin(now), now)?;
        Ok(vec![Effect::Persist])
    }

    pub(super) fn start_work(&mut self, origin: WorkStartOrigin, now: DateTime<Utc>) -> Result<()> {
        if self.status() != TimerStatus::Idle {
            log::warn!(
                "event_id=timer.start.rejected reason={} current_state={:?}",
                "invalid_state_transition",
                self.status(),
            );
            return Err(TimerError::InvalidStateTransition {
                action: "start work".to_string(),
                current_state: self.status(),
            });
        }

        if origin != WorkStartOrigin::ScheduleStartBoundary {
            if let Some(remaining_ms) = self.rate_limit_remaining_ms(now) {
                log::warn!(
                    "event_id=timer.start.rejected reason={} current_state={:?}",
                    "rate_limited",
                    self.status(),
                );
                return Err(TimerError::RateLimited { remaining_ms });
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

        log::info!(
            "event_id=timer.work_session.started duration_secs={} work_start_origin={:?}",
            work_duration_secs.get(),
            origin,
        );
        Ok(())
    }

    fn begin_work_session(&mut self, duration_secs: SessionDurationSecs, now: DateTime<Utc>) {
        self.phase = TimerPhase::Working {
            ends_at: now
                + ChronoDuration::seconds(i64::try_from(duration_secs.get()).unwrap_or(i64::MAX)),
            duration_secs,
        };
        self.last_transition_at = Some(now);
    }

    fn rate_limit_remaining_ms(&self, now: DateTime<Utc>) -> Option<u64> {
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
        now: DateTime<Utc>,
    ) -> Result<Vec<Effect>> {
        if !self.post_break_decision_pending() {
            log::warn!(
                "event_id=timer.transition.rejected action={} current_state={:?} pending={}",
                "resolve_post_break_decision",
                self.status(),
                "false",
            );
            return Err(TimerError::InvalidStateTransition {
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

        log::info!(
            "event_id=timer.transition.completed transition_stage={} transition_reason={} action={:?} policy={:?} post_break_decision_pending={} schedule_stop_state={:?}",
            "resolve_post_break_decision",
            "manual_post_break_decision",
            action,
            self.settings.post_break_transition_policy,
            self.post_break_decision_pending(),
            self.schedule.stop_state,
        );
        Ok(vec![Effect::Persist])
    }

    fn enter_idle(&mut self, post_break_decision_pending: bool, now: DateTime<Utc>) {
        self.phase = TimerPhase::Idle {
            post_break_decision_pending,
        };
        self.last_transition_at = Some(now);
    }

    pub(super) fn emergency_stop(&mut self, now: DateTime<Utc>) -> Vec<Effect> {
        self.boot_auto_start_offer = None;
        self.schedule.stop_state = ScheduleStopState::NotControlledBySchedule;
        self.enter_idle(false, now);

        vec![
            Effect::SetBreakLockTarget(BreakLockTarget::Unlocked),
            Effect::Persist,
        ]
    }

    /// Countdown step for the current session. Work derives from the wall
    /// clock; breaks consume only the seconds explicitly credited by the caller.
    pub(super) fn count_down(
        &mut self,
        now: DateTime<Utc>,
        credited_break_seconds: u64,
    ) -> Vec<Effect> {
        let finished = match &mut self.phase {
            TimerPhase::Idle { .. } => return Vec::new(),
            TimerPhase::Working { ends_at, .. } => {
                (*ends_at <= now).then_some(FinishedSession::Work)
            }
            TimerPhase::OnBreak(current_break) => {
                current_break.remaining_seconds = current_break
                    .remaining_seconds
                    .saturating_sub(credited_break_seconds);
                (current_break.remaining_seconds == 0).then_some(FinishedSession::Break)
            }
        };

        let mut effects = Vec::new();
        match finished {
            Some(FinishedSession::Work) => effects.extend(self.complete_work_session(now)),
            Some(FinishedSession::Break) => effects.extend(self.complete_break_session(now)),
            None => {}
        }
        effects
    }

    pub(super) fn complete_work_session(&mut self, now: DateTime<Utc>) -> Vec<Effect> {
        if self.scheduled_stop_is_due() {
            let payload = self.execute_scheduled_stop(SessionType::Work, now);
            return vec![
                Effect::EmitSessionCompleted(payload),
                Effect::EmitAutoRunStopped,
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
            Effect::Persist,
        ]
    }

    fn complete_break_session(&mut self, now: DateTime<Utc>) -> Vec<Effect> {
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
            Effect::Persist,
        ]
    }

    fn finish_break_waiting_for_decision(
        &mut self,
        completed_session_type: SessionType,
        now: DateTime<Utc>,
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
        now: DateTime<Utc>,
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
        now: DateTime<Utc>,
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
        log::info!(
            "event_id=timer.transition.completed transition_stage={} transition_reason={:?} policy={:?} post_break_decision_pending={} schedule_stop_state={:?}",
            transition_stage,
            transition_reason,
            self.settings.post_break_transition_policy,
            self.post_break_decision_pending(),
            self.schedule.stop_state,
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
