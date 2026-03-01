// Clock abstractions for deterministic time-dependent behavior.

use chrono::{DateTime, Duration, Local, Utc};
use std::sync::Mutex;

/// Abstract clock used by runtime code and tests.
///
/// Usage examples:
/// ```rust,ignore
/// // Production
/// let system_clock = SystemClock;
/// let current_local = system_clock.now_local();
///
/// // Test
/// let fixed_clock = FixedClock::new(current_local);
/// fixed_clock.advance_seconds(60);
/// ```
pub trait Clock {
    fn now_local(&self) -> DateTime<Local>;

    fn now_utc(&self) -> DateTime<Utc> {
        self.now_local().with_timezone(&Utc)
    }
}

/// Runtime clock implementation backed by the system wall clock.
#[derive(Debug, Default, Clone, Copy)]
pub struct SystemClock;

impl Clock for SystemClock {
    fn now_local(&self) -> DateTime<Local> {
        Local::now()
    }
}

/// Test clock implementation that returns a controlled local datetime.
///
/// Usage example:
/// ```rust,ignore
/// let fixed_clock = FixedClock::new(chrono::Local::now());
/// fixed_clock.set_now_local(chrono::Local::now());
/// ```
#[derive(Debug)]
#[allow(dead_code)]
pub struct FixedClock {
    current_local_datetime: Mutex<DateTime<Local>>,
}

#[allow(dead_code)]
impl FixedClock {
    pub fn new(now_local: DateTime<Local>) -> Self {
        Self {
            current_local_datetime: Mutex::new(now_local),
        }
    }

    pub fn set_now_local(&self, now_local: DateTime<Local>) {
        let mut locked_datetime = self
            .current_local_datetime
            .lock()
            .expect("fixed clock mutex should not be poisoned");
        *locked_datetime = now_local;
    }

    pub fn advance_seconds(&self, seconds: i64) {
        let mut locked_datetime = self
            .current_local_datetime
            .lock()
            .expect("fixed clock mutex should not be poisoned");
        *locked_datetime += Duration::seconds(seconds);
    }
}

impl Clock for FixedClock {
    fn now_local(&self) -> DateTime<Local> {
        self.current_local_datetime
            .lock()
            .expect("fixed clock mutex should not be poisoned")
            .to_owned()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    fn create_utc_datetime(
        year: i32,
        month: u32,
        day: u32,
        hour: u32,
        minute: u32,
        second: u32,
    ) -> DateTime<Utc> {
        Utc.with_ymd_and_hms(year, month, day, hour, minute, second)
            .single()
            .expect("valid UTC datetime")
    }

    #[test]
    fn test_system_clock_now_local_is_recent() {
        let system_clock = SystemClock;
        let actual_local = Local::now();
        let clock_local = system_clock.now_local();
        let seconds_difference = actual_local
            .signed_duration_since(clock_local)
            .num_seconds()
            .abs();
        assert!(seconds_difference <= 2);
    }

    #[test]
    fn test_fixed_clock_returns_seed_time() {
        let seed_utc = create_utc_datetime(2025, 2, 1, 9, 0, 0);
        let fixed_clock = FixedClock::new(seed_utc.with_timezone(&Local));

        let actual_local = fixed_clock.now_local();
        assert_eq!(actual_local, seed_utc.with_timezone(&Local));
    }

    #[test]
    fn test_fixed_clock_set_and_advance_seconds() {
        let first_utc = create_utc_datetime(2025, 2, 1, 9, 0, 0);
        let fixed_clock = FixedClock::new(first_utc.with_timezone(&Local));

        let second_utc = create_utc_datetime(2025, 2, 1, 10, 0, 0);
        fixed_clock.set_now_local(second_utc.with_timezone(&Local));
        assert_eq!(fixed_clock.now_utc(), second_utc);

        fixed_clock.advance_seconds(90);
        assert_eq!(
            fixed_clock.now_utc(),
            create_utc_datetime(2025, 2, 1, 10, 1, 30)
        );
    }
}
