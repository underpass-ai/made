//! [`CeremonyEventFanout`] — many projections behind one seam.
//!
//! `SessionStream` tells exactly one subscriber what an append sealed,
//! because a stream that held a list would have to decide what a
//! failure in the middle of it means, and that decision belongs to
//! whoever composed the list. This is that composition: subscribers in
//! the order they were given, each told in turn, none able to stop the
//! next.

use std::sync::Arc;

use async_trait::async_trait;
use made_core::ports::{CeremonyEventSubscriberPort, PositionedRecord};

/// Tells each subscriber, in order, what one append sealed.
pub struct CeremonyEventFanout {
    subscribers: Vec<Arc<dyn CeremonyEventSubscriberPort>>,
}

impl std::fmt::Debug for CeremonyEventFanout {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("CeremonyEventFanout")
            .field("subscribers", &self.subscribers.len())
            .finish()
    }
}

impl CeremonyEventFanout {
    /// Fan out to these subscribers, in this order.
    ///
    /// Order is the caller's, and it is kept: a projection a later one
    /// reads back — memory before something that recalls it — has to
    /// have been written by then, and "in the order given" is a
    /// promise a host can rely on rather than an accident of the
    /// collection type.
    #[must_use]
    pub fn new(subscribers: Vec<Arc<dyn CeremonyEventSubscriberPort>>) -> Self {
        Self { subscribers }
    }

    /// Everything the engine wires by default, plus whatever the host
    /// added.
    #[must_use]
    pub fn of(
        engine: Arc<dyn CeremonyEventSubscriberPort>,
        host: Option<Arc<dyn CeremonyEventSubscriberPort>>,
    ) -> Self {
        Self::new(core::iter::once(engine).chain(host).collect())
    }

    /// How many subscribers are listening.
    #[must_use]
    pub fn len(&self) -> usize {
        self.subscribers.len()
    }

    /// Whether nothing is listening.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.subscribers.is_empty()
    }
}

#[async_trait]
impl CeremonyEventSubscriberPort for CeremonyEventFanout {
    async fn observe(&self, records: &[PositionedRecord]) {
        for subscriber in &self.subscribers {
            subscriber.observe(records).await;
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicUsize, Ordering};

    use super::*;

    /// A subscriber that says when it was told, and one that reports
    /// nothing at all — the second must still be told.
    #[derive(Debug, Default)]
    struct Counting {
        calls: AtomicUsize,
        order: AtomicUsize,
        seen: AtomicUsize,
    }

    static TURN: AtomicUsize = AtomicUsize::new(0);

    #[async_trait]
    impl CeremonyEventSubscriberPort for Counting {
        async fn observe(&self, records: &[PositionedRecord]) {
            self.calls.fetch_add(1, Ordering::SeqCst);
            self.order
                .store(TURN.fetch_add(1, Ordering::SeqCst), Ordering::SeqCst);
            self.seen.fetch_add(records.len(), Ordering::SeqCst);
        }
    }

    #[tokio::test]
    async fn every_subscriber_is_told_in_the_order_it_was_given() {
        TURN.store(0, Ordering::SeqCst);
        let first = Arc::new(Counting::default());
        let second = Arc::new(Counting::default());
        let fanout = CeremonyEventFanout::of(first.clone(), Some(second.clone()));

        fanout.observe(&[]).await;

        assert_eq!(fanout.len(), 2);
        assert!(!fanout.is_empty());
        assert_eq!(first.calls.load(Ordering::SeqCst), 1);
        assert_eq!(second.calls.load(Ordering::SeqCst), 1);
        assert!(first.order.load(Ordering::SeqCst) < second.order.load(Ordering::SeqCst));
    }

    #[tokio::test]
    async fn a_fanout_over_nothing_is_a_fanout() {
        let fanout = CeremonyEventFanout::new(Vec::new());

        fanout.observe(&[]).await;

        assert!(fanout.is_empty());
    }
}
