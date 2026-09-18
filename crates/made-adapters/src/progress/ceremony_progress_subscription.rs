use async_trait::async_trait;
use made_core::ports::CeremonyProgressSubscriptionPort;
use tokio::sync::watch;

/// One receiver over the process-wide append generation.
#[derive(Debug)]
pub(super) struct CeremonyProgressSubscription {
    receiver: watch::Receiver<u64>,
}

impl CeremonyProgressSubscription {
    pub(super) const fn new(receiver: watch::Receiver<u64>) -> Self {
        Self { receiver }
    }
}

#[async_trait]
impl CeremonyProgressSubscriptionPort for CeremonyProgressSubscription {
    async fn wait(&mut self) {
        if self.receiver.changed().await.is_err() {
            std::future::pending::<()>().await;
        }
    }
}
