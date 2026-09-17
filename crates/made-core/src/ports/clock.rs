//! [`ClockPort`] — source of wall-clock time.
//!
//! The domain never calls `OffsetDateTime::now_utc` directly so that
//! deliberations stay reproducible under test and deterministic
//! clocks can be injected (frozen clocks for replay, accelerated
//! clocks for load tests, etc.).

use time::OffsetDateTime;

use crate::value_objects::DurationMs;

pub trait ClockPort: Send + Sync {
    fn now(&self) -> OffsetDateTime;

    /// Monotonic time since the composition root created this clock.
    /// Deterministic test clocks default to a frozen zero uptime.
    fn uptime(&self) -> DurationMs {
        DurationMs::ZERO
    }
}
