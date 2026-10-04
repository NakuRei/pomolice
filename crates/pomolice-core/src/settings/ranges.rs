use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq, Error)]
#[error("{kind} must be between {min} and {max}, got {actual}")]
pub struct IntegerRangeValidationError {
    kind: &'static str,
    min: u64,
    max: u64,
    actual: u64,
}

impl IntegerRangeValidationError {
    pub(crate) fn new(kind: &'static str, actual: u64, min: u64, max: u64) -> Self {
        Self {
            kind,
            min,
            max,
            actual,
        }
    }
}

macro_rules! bounded_u64_newtype {
    ($name:ident, $kind:literal, $min:expr, $max:expr) => {
        #[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
        #[serde(try_from = "u64", into = "u64")]
        pub struct $name(u64);

        impl $name {
            pub const MIN: u64 = $min;
            pub const MAX: u64 = $max;

            /// # Errors
            /// Rejects values outside the inclusive `MIN` and `MAX` bounds.
            pub fn try_new(
                value: u64,
            ) -> std::result::Result<Self, $crate::settings::IntegerRangeValidationError> {
                if ($min..=$max).contains(&value) {
                    return Ok(Self(value));
                }

                Err($crate::settings::IntegerRangeValidationError::new(
                    $kind, value, $min, $max,
                ))
            }

            #[must_use]
            pub fn get(self) -> u64 {
                self.0
            }
        }

        impl std::fmt::Display for $name {
            fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                write!(formatter, "{}", self.0)
            }
        }

        impl TryFrom<u64> for $name {
            type Error = $crate::settings::IntegerRangeValidationError;

            fn try_from(value: u64) -> std::result::Result<Self, Self::Error> {
                Self::try_new(value)
            }
        }

        impl From<$name> for u64 {
            fn from(value: $name) -> Self {
                value.0
            }
        }
    };
}

macro_rules! bounded_u32_newtype {
    ($name:ident, $kind:literal, $min:expr, $max:expr) => {
        #[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
        #[serde(try_from = "u32", into = "u32")]
        pub struct $name(u32);

        impl $name {
            pub const MIN: u32 = $min;
            pub const MAX: u32 = $max;

            /// # Errors
            /// Rejects values outside the inclusive `MIN` and `MAX` bounds.
            pub fn try_new(
                value: u32,
            ) -> std::result::Result<Self, $crate::settings::IntegerRangeValidationError> {
                if ($min..=$max).contains(&value) {
                    return Ok(Self(value));
                }

                Err($crate::settings::IntegerRangeValidationError::new(
                    $kind,
                    u64::from(value),
                    $min as u64,
                    $max as u64,
                ))
            }

            #[must_use]
            pub fn get(self) -> u32 {
                self.0
            }
        }

        impl std::fmt::Display for $name {
            fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                write!(formatter, "{}", self.0)
            }
        }

        impl TryFrom<u32> for $name {
            type Error = $crate::settings::IntegerRangeValidationError;

            fn try_from(value: u32) -> std::result::Result<Self, Self::Error> {
                Self::try_new(value)
            }
        }

        impl From<$name> for u32 {
            fn from(value: $name) -> Self {
                value.0
            }
        }
    };
}

pub(crate) use bounded_u64_newtype;

// The 1-second minimums let a developer reach a break without waiting;
// raise them before release.
bounded_u64_newtype!(WorkDurationSecs, "work_duration_secs", 1, 7_200);
bounded_u64_newtype!(
    ShortBreakDurationSecs,
    "short_break_duration_secs",
    1,
    3_600
);
bounded_u64_newtype!(LongBreakDurationSecs, "long_break_duration_secs", 1, 3_600);
bounded_u32_newtype!(LongBreakIntervalCount, "long_break_interval", 2, 10);
bounded_u32_newtype!(
    NotificationSoundVolumePercent,
    "notification_sound_volume_percent",
    0,
    100
);
