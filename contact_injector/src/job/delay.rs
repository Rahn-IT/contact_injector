//! Validated intervals at the form and database boundaries.

use std::time::Duration;

use serde::{Deserialize, Serialize};

/// A positive interval, capped at one year so timer arithmetic stays bounded.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(try_from = "i64", into = "i64")]
pub struct Delay(i64);

/// An interval outside the supported range.
#[derive(Debug, thiserror::Error)]
#[error("job delay must be between 1 and 525600 minutes, got {0}")]
pub struct Error(i64);

impl TryFrom<i64> for Delay {
    type Error = Error;

    fn try_from(minutes: i64) -> Result<Self, Self::Error> {
        match minutes {
            1..=525_600 => Ok(Self(minutes)),
            _ => Err(Error(minutes)),
        }
    }
}

impl From<Delay> for i64 {
    fn from(delay: Delay) -> Self {
        // SQL and serialization are the boundaries that need the raw minutes.
        delay.0
    }
}

impl Delay {
    /// Returns the bounded duration represented by this interval.
    pub fn duration(self) -> Duration {
        Duration::from_secs(self.0 as u64 * 60)
    }
}

#[cfg(test)]
mod tests {
    use super::Delay;

    #[test]
    fn rejects_intervals_that_spin_or_overflow() {
        for minutes in [i64::MIN, -1, 0, 525_601, i64::MAX] {
            assert!(Delay::try_from(minutes).is_err());
        }
    }

    #[test]
    fn deserialization_cannot_bypass_interval_validation() {
        assert!(serde_json::from_str::<Delay>("0").is_err());
        let delay: Delay = serde_json::from_str("525600").unwrap();
        assert_eq!(delay.duration().as_secs(), 31_536_000);
        assert_eq!(serde_json::to_string(&delay).unwrap(), "525600");
    }
}
