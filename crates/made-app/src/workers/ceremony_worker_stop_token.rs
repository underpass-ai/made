use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

/// Cooperative stop shared by a host and its recoverable worker driver.
#[derive(Debug, Clone, Default)]
pub struct CeremonyWorkerStopToken {
    requested: Arc<AtomicBool>,
}

impl CeremonyWorkerStopToken {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    pub fn request(&self) {
        self.requested.store(true, Ordering::Release);
    }

    #[must_use]
    pub fn is_requested(&self) -> bool {
        self.requested.load(Ordering::Acquire)
    }
}
