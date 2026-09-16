use std::sync::atomic::AtomicUsize;
use std::sync::Arc;

use super::EventStoreFake;

/// An event store whose every append is overtaken by another writer.
///
/// Every append is refused as a conflict with nothing written, however
/// often the caller reloads and decides again, and `appends` counts
/// how often it was asked. A retrying use case must give up at its
/// bound and say so; a fail-fast one must say so at once.
#[derive(Debug)]
pub(in crate::usecases) struct StoreThatLosesEveryRace {
    pub(super) inner: Arc<EventStoreFake>,
    pub(super) appends: AtomicUsize,
}
