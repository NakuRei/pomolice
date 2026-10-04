use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq, Error)]
#[error("invalid HH:MM value: {message}")]
pub struct HhMmParseError {
    message: String,
}

impl HhMmParseError {
    fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct HhMm {
    hour: u8,
    minute: u8,
}

impl HhMm {
    /// # Errors
    /// Rejects hours above 23 or minutes above 59.
    pub fn try_new(hour: u8, minute: u8) -> std::result::Result<Self, HhMmParseError> {
        if hour >= 24 {
            return Err(HhMmParseError::new(format!(
                "hour {hour:02} is out of range"
            )));
        }

        if minute >= 60 {
            return Err(HhMmParseError::new(format!(
                "minute {minute:02} is out of range"
            )));
        }

        Ok(Self { hour, minute })
    }

    #[must_use]
    pub fn hour(self) -> u8 {
        self.hour
    }

    #[must_use]
    pub fn minute(self) -> u8 {
        self.minute
    }

    #[must_use]
    pub fn minutes_since_midnight(self) -> u32 {
        u32::from(self.hour()) * 60 + u32::from(self.minute())
    }
}

impl TryFrom<String> for HhMm {
    type Error = HhMmParseError;

    fn try_from(value: String) -> std::result::Result<Self, Self::Error> {
        let bytes = value.as_bytes();
        if bytes.len() != 5
            || bytes[2] != b':'
            || !bytes[0].is_ascii_digit()
            || !bytes[1].is_ascii_digit()
            || !bytes[3].is_ascii_digit()
            || !bytes[4].is_ascii_digit()
        {
            return Err(HhMmParseError::new(format!(
                "{value:?} must use zero-padded HH:MM format"
            )));
        }

        let hour = (bytes[0] - b'0') * 10 + (bytes[1] - b'0');
        let minute = (bytes[3] - b'0') * 10 + (bytes[4] - b'0');
        Self::try_new(hour, minute)
    }
}

impl std::fmt::Display for HhMm {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{:02}:{:02}", self.hour(), self.minute())
    }
}
