use async_trait::async_trait;
use made_core::ports::ProtocolAgentActivitySubscriptionPort;
use tokio::sync::watch;

#[derive(Debug)]
pub(super) struct ProtocolAgentActivitySubscription {
    receiver: watch::Receiver<u64>,
}

impl ProtocolAgentActivitySubscription {
    pub(super) const fn new(receiver: watch::Receiver<u64>) -> Self {
        Self { receiver }
    }
}

#[async_trait]
impl ProtocolAgentActivitySubscriptionPort for ProtocolAgentActivitySubscription {
    async fn wait(&mut self) {
        if self.receiver.changed().await.is_err() {
            std::future::pending::<()>().await;
        }
    }
}
