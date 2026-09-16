//! A clock that does not move.
//!
//! Two engines running the same session at the same instant is the
//! only way their answers can be compared field for field without a
//! list of timestamps excused from the comparison — and a list of
//! excused timestamps is where a real divergence hides. Both arms read
//! the time from one of these, set to the same instant, so every
//! `occurred_at` in both answers is that instant.

use std::sync::Arc;

use made_core::ports::ClockPort;
use time::macros::datetime;
use time::OffsetDateTime;

/// The instant the parity session happens at. Arbitrary, fixed, and
/// the same on both arms — which is the whole of its job.
pub const PARITY_INSTANT: OffsetDateTime = datetime!(2026-09-16 09:00:00 UTC);

/// A [`ClockPort`] frozen at one instant.
#[derive(Debug, Clone, Copy)]
pub struct ParityClock {
    instant: OffsetDateTime,
}

impl Default for ParityClock {
    fn default() -> Self {
        Self::at(PARITY_INSTANT)
    }
}

impl ParityClock {
    #[must_use]
    pub const fn at(instant: OffsetDateTime) -> Self {
        Self { instant }
    }

    /// The frozen clock, ready to hand to a builder or a fixture.
    #[must_use]
    pub fn shared() -> Arc<dyn ClockPort> {
        Arc::new(Self::default())
    }

    #[must_use]
    pub const fn instant(&self) -> OffsetDateTime {
        self.instant
    }
}

impl ClockPort for ParityClock {
    fn now(&self) -> OffsetDateTime {
        self.instant
    }
}
