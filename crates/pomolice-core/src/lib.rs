//! Deterministic timer transitions, weekly schedules, and validated settings.
//!
//! Supply UTC instants and an explicit timezone. Use fixed timezone rules for
//! replay; a system timezone provider belongs to the application adapter.
//! Effects describe requested work, not evidence that saving or locking succeeded.
//!
//! ```
//! use chrono::{TimeZone, Utc};
//! use pomolice_core::{settings::UserSettings, timer_core::{Event, TimerCore}};
//!
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! let now = Utc.with_ymd_and_hms(2025, 1, 6, 9, 0, 0).single().ok_or("invalid time")?;
//! let (mut timer, startup_effects) = TimerCore::restore(None, UserSettings::default(), now, Utc);
//! let start_effects = timer.apply(Event::StartWorkRequested, now)?;
//! let later = now + chrono::Duration::seconds(10);
//! let effects = timer.apply(Event::TimeObserved { credited_break_seconds: 0 }, later)?;
//! assert_eq!(timer.snapshot(later).remaining_seconds, 25 * 60 - 10);
//! # Ok(())
//! # }
//! ```

pub mod schedule;
pub mod settings;
pub mod timer_core;
