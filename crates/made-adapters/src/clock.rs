//! Clock adapter.
//!
//! The domain never reads the wall clock directly. Aggregates receive
//! an `OffsetDateTime` through [`ClockPort`] so deliberations stay
//! reproducible under test.

use std::time::Instant;

use made_core::ports::ClockPort;
use made_core::value_objects::DurationMs;
use time::OffsetDateTime;

/// Wall-clock implementation of [`ClockPort`] that returns UTC time
/// from the host's monotonic source as known to `time`.
#[derive(Debug, Clone, Copy)]
pub struct SystemClock {
    started_at: Instant,
}

impl SystemClock {
    #[must_use]
    pub fn new() -> Self {
        Self {
            started_at: Instant::now(),
        }
    }
}

impl Default for SystemClock {
    fn default() -> Self {
        Self::new()
    }
}

impl ClockPort for SystemClock {
    fn now(&self) -> OffsetDateTime {
        OffsetDateTime::now_utc()
    }

    fn uptime(&self) -> DurationMs {
        DurationMs::from_millis(
            u64::try_from(self.started_at.elapsed().as_millis()).unwrap_or(u64::MAX),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread::sleep;
    use std::time::Duration;

    #[test]
    fn now_returns_utc() {
        let now = SystemClock::new().now();
        assert_eq!(now.offset(), time::UtcOffset::UTC);
    }

    #[test]
    fn subsequent_reads_are_monotonically_non_decreasing() {
        let clock = SystemClock::new();
        let a = clock.now();
        sleep(Duration::from_millis(1));
        let b = clock.now();
        assert!(b >= a, "wall clock went backwards: {a:?} -> {b:?}");
    }
}
