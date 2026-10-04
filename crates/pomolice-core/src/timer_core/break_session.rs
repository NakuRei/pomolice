use chrono::{DateTime, Duration as ChronoDuration, Utc};

use super::error::{Result, TimerError};
use super::{BreakLockTarget, Effect, TimerCore};
use crate::timer_core::{BreakAttendance, TimerStatus};

const INPUT_PENALTY_DEBOUNCE: ChronoDuration = ChronoDuration::milliseconds(100);

impl<Tz: chrono::TimeZone> TimerCore<Tz> {
    pub(super) fn set_break_attendance(
        &mut self,
        attendance: BreakAttendance,
    ) -> Result<Vec<Effect>> {
        let Some(current_break) = self.phase.current_break_mut() else {
            log::warn!(
                "event_id=timer.transition.rejected action={} current_state={:?}",
                "set_break_attendance",
                self.status(),
            );
            return Err(TimerError::InvalidStateTransition {
                action: "set break attendance".to_string(),
                current_state: self.status(),
            });
        };
        current_break.attendance = attendance;
        Ok(Vec::new())
    }

    pub(super) fn apply_input_penalty(&mut self, now: DateTime<Utc>) -> Vec<Effect> {
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

        vec![Effect::Persist]
    }

    pub(super) fn escape_combo_pressed(&self) -> Vec<Effect> {
        if self.status() == TimerStatus::OnBreak {
            vec![Effect::SetBreakLockTarget(BreakLockTarget::Unlocked)]
        } else {
            Vec::new()
        }
    }
}
