// What happens inside a break: attendance, input penalties, the escape
// combo, and the outcomes of the break lock.

use chrono::{DateTime, Duration as ChronoDuration, Local};

use super::{Effect, TimerCore};
use crate::error::{PomoliceError, Result};
use crate::logging_event::{log_warn_event, BackendLogEvent};
use crate::types::{BreakAttendance, TimerStatus};

const INPUT_PENALTY_DEBOUNCE: ChronoDuration = ChronoDuration::milliseconds(100);

impl TimerCore {
    pub(super) fn set_break_attendance(
        &mut self,
        attendance: BreakAttendance,
        now: DateTime<Local>,
    ) -> Result<Vec<Effect>> {
        let Some(current_break) = self.phase.current_break_mut() else {
            log_warn_event(
                BackendLogEvent::TimerTransitionRejected,
                &[
                    ("action", "set_break_attendance".to_string()),
                    ("current_state", format!("{:?}", self.status())),
                ],
            );
            return Err(PomoliceError::InvalidStateTransition {
                action: "set break attendance".to_string(),
                current_state: self.status(),
            });
        };
        current_break.attendance = attendance;
        Ok(vec![self.publish(now)])
    }

    pub(super) fn apply_input_penalty(&mut self, now: DateTime<Local>) -> Vec<Effect> {
        let Some(current_break) = self.phase.current_break_mut() else {
            return Vec::new();
        };
        if let Some(last_penalty_at) = current_break.last_penalty_at {
            let elapsed = now - last_penalty_at;
            if elapsed < ChronoDuration::zero() || elapsed < INPUT_PENALTY_DEBOUNCE {
                return Vec::new();
            }
        }

        current_break.interaction_count += 1;
        current_break.last_penalty_at = Some(now);
        current_break.remaining_seconds = current_break.duration_secs.get();

        vec![self.publish(now), Effect::Persist]
    }

    pub(super) fn escape_combo_pressed(&self) -> Vec<Effect> {
        if self.status() == TimerStatus::OnBreak {
            vec![Effect::ReleaseBreakLock]
        } else {
            Vec::new()
        }
    }

    pub(super) fn break_lock_entered(&mut self, now: DateTime<Local>) -> Vec<Effect> {
        if let Some(current_break) = self.phase.current_break_mut() {
            current_break.is_fullscreen = true;
        }
        vec![self.publish(now), Effect::Persist]
    }

    pub(super) fn break_lock_enter_failed(&mut self, now: DateTime<Local>) -> Vec<Effect> {
        if let Some(current_break) = self.phase.current_break_mut() {
            current_break.is_fullscreen = false;
        }
        vec![self.publish(now), Effect::Persist]
    }

    pub(super) fn break_lock_released(&mut self, now: DateTime<Local>) -> Vec<Effect> {
        if let Some(current_break) = self.phase.current_break_mut() {
            current_break.is_fullscreen = false;
        }
        vec![self.publish(now), Effect::Persist]
    }

    pub(super) fn break_lock_release_failed(&self, now: DateTime<Local>) -> Vec<Effect> {
        vec![self.publish(now), Effect::Persist]
    }
}
