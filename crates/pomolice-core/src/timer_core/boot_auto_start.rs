use chrono::{DateTime, Utc};

use super::error::Result;
use super::schedule_control::WorkStartOrigin;
use super::{Effect, TimerCore};
use crate::schedule::boundary::{is_now_inside_schedule_window, ActiveWindow};
use crate::settings::UserSettings;
use crate::timer_core::TimerStatus;

pub(super) fn initial_boot_auto_start_offer<Tz: chrono::TimeZone>(
    status: TimerStatus,
    settings: &UserSettings,
    now: &DateTime<Tz>,
) -> Option<ActiveWindow> {
    if status != TimerStatus::Idle || !settings.auto_run_feature_enabled {
        return None;
    }
    is_now_inside_schedule_window(now, &settings.weekly_auto_run_config)
}

impl<Tz: chrono::TimeZone> TimerCore<Tz> {
    fn boot_auto_start_offer_is_current(&self, now: DateTime<Utc>) -> bool {
        self.status() == TimerStatus::Idle
            && self
                .boot_auto_start_offer
                .as_ref()
                .is_some_and(|offered| self.active_window_at(now).as_ref() == Some(offered))
    }

    pub(super) fn clear_invalid_boot_auto_start_offer(&mut self, now: DateTime<Utc>) {
        if !self.boot_auto_start_offer_is_current(now) {
            self.boot_auto_start_offer = None;
        }
    }

    pub(super) fn resolve_boot_prompt(
        &mut self,
        start_now: bool,
        now: DateTime<Utc>,
    ) -> Result<Vec<Effect>> {
        if !self.boot_auto_start_offer_is_current(now) {
            self.boot_auto_start_offer = None;
            return Ok(Vec::new());
        }
        let effects = if start_now {
            self.start_work(WorkStartOrigin::ManualInsideScheduleWindow, now)?;
            vec![Effect::Persist]
        } else {
            Vec::new()
        };
        self.boot_auto_start_offer = None;
        Ok(effects)
    }
}
