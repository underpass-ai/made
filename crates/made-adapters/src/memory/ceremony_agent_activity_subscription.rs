use async_trait::async_trait;
use made_core::ports::CeremonyAgentActivitySubscriptionPort;
use tokio::sync::watch;

#[derive(Debug)]
pub(super) struct CeremonyAgentActivitySubscription {
    receiver: watch::Receiver<u64>,
}

impl CeremonyAgentActivitySubscription {
    pub(super) const fn new(receiver: watch::Receiver<u64>) -> Self {
        Self { receiver }
    }
}

#[async_trait]
impl CeremonyAgentActivitySubscriptionPort for CeremonyAgentActivitySubscription {
    async fn wait(&mut self) {
        if self.receiver.changed().await.is_err() {
            std::future::pending::<()>().await;
        }
    }
}
