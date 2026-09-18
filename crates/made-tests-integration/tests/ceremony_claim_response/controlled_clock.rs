use std::sync::atomic::{AtomicI64, Ordering};

use made_core::ports::ClockPort;
use time::OffsetDateTime;

#[derive(Default)]
pub(super) struct ControlledClock(AtomicI64);

impl ControlledClock {
    pub(super) fn expire_first_claim(&self) {
        self.0.store(2, Ordering::SeqCst);
    }
}

impl ClockPort for ControlledClock {
    fn now(&self) -> OffsetDateTime {
        OffsetDateTime::UNIX_EPOCH + time::Duration::seconds(self.0.load(Ordering::SeqCst))
    }
}
