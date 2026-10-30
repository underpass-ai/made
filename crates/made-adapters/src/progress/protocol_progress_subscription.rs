use async_trait::async_trait;
use made_core::ports::ProtocolProgressSubscriptionPort;
use tokio::sync::watch;

/// One receiver over the process-wide append generation.
#[derive(Debug)]
pub(super) struct ProtocolProgressSubscription {
    receiver: watch::Receiver<u64>,
}

impl ProtocolProgressSubscription {
    pub(super) const fn new(receiver: watch::Receiver<u64>) -> Self {
        Self { receiver }
    }
}

#[async_trait]
impl ProtocolProgressSubscriptionPort for ProtocolProgressSubscription {
    async fn wait(&mut self) {
        if self.receiver.changed().await.is_err() {
            std::future::pending::<()>().await;
        }
    }
}
