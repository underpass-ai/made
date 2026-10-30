use async_trait::async_trait;
use made_core::ports::{
    PositionedRecord, ProtocolEventSubscriberPort, ProtocolProgressNotifierPort,
    ProtocolProgressSubscriptionPort,
};
use tokio::sync::watch;

use super::protocol_progress_subscription::ProtocolProgressSubscription;

/// One bounded, payload-free append generation shared by the process.
#[derive(Debug)]
pub struct ProtocolProgressNotifier {
    generation: watch::Sender<u64>,
}

impl ProtocolProgressNotifier {
    #[must_use]
    pub fn new() -> Self {
        let (generation, _) = watch::channel(0);
        Self { generation }
    }

    #[cfg(test)]
    pub(super) fn receiver_count(&self) -> usize {
        self.generation.receiver_count()
    }
}

impl Default for ProtocolProgressNotifier {
    fn default() -> Self {
        Self::new()
    }
}

impl ProtocolProgressNotifierPort for ProtocolProgressNotifier {
    fn subscribe(&self) -> Box<dyn ProtocolProgressSubscriptionPort> {
        Box::new(ProtocolProgressSubscription::new(
            self.generation.subscribe(),
        ))
    }
}

#[async_trait]
impl ProtocolEventSubscriberPort for ProtocolProgressNotifier {
    async fn observe(&self, _records: &[PositionedRecord]) {
        self.generation.send_modify(|generation| {
            *generation = generation.wrapping_add(1);
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn subscriptions_are_payload_free_and_released_on_drop() {
        let notifier = ProtocolProgressNotifier::new();
        let mut subscription = notifier.subscribe();
        assert_eq!(notifier.receiver_count(), 1);

        notifier.observe(&[]).await;
        subscription.wait().await;
        drop(subscription);

        assert_eq!(notifier.receiver_count(), 0);
    }

    #[tokio::test]
    async fn a_closed_notifier_does_not_turn_wait_into_a_busy_loop() {
        let notifier = ProtocolProgressNotifier::new();
        let mut subscription = notifier.subscribe();
        drop(notifier);

        let waiter = tokio::spawn(async move { subscription.wait().await });
        tokio::task::yield_now().await;

        assert!(!waiter.is_finished());
        waiter.abort();
    }
}
