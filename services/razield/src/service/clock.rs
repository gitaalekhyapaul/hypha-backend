use std::error::Error;
use std::fmt;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::admission::MAX_PROTOBUF_TIMESTAMP_SECONDS;

/// Owner-trusted source of whole-second UTC protocol time.
pub trait TrustedClock: Send + Sync + 'static {
    fn now_unix_seconds(&self) -> Result<i64, ClockError>;
}

/// Production clock backed by the host's system time.
#[derive(Debug, Clone, Copy, Default)]
pub struct SystemClock;

impl TrustedClock for SystemClock {
    fn now_unix_seconds(&self) -> Result<i64, ClockError> {
        system_time_to_unix_seconds(SystemTime::now())
    }
}

pub(crate) fn system_time_to_unix_seconds(time: SystemTime) -> Result<i64, ClockError> {
    let elapsed = time
        .duration_since(UNIX_EPOCH)
        .map_err(|_| ClockError::BeforeUnixEpoch)?;
    let seconds = i64::try_from(elapsed.as_secs()).map_err(|_| ClockError::OutOfRange)?;
    if seconds > MAX_PROTOBUF_TIMESTAMP_SECONDS {
        return Err(ClockError::OutOfRange);
    }
    Ok(seconds)
}

/// Invalid trusted time that cannot enter a capability request.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClockError {
    BeforeUnixEpoch,
    OutOfRange,
}

impl fmt::Display for ClockError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("trusted service time is unavailable")
    }
}

impl Error for ClockError {}
