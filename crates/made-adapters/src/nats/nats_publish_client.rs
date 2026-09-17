use async_nats::Client;
use async_trait::async_trait;

/// The two client operations that enqueue and flush one Core NATS delivery.
#[async_trait]
pub(super) trait NatsPublishClient: Send + Sync {
    async fn publish(&self, subject: String, payload: Vec<u8>) -> Result<(), ()>;
    async fn flush(&self) -> Result<(), ()>;
}

#[async_trait]
impl NatsPublishClient for Client {
    async fn publish(&self, subject: String, payload: Vec<u8>) -> Result<(), ()> {
        Client::publish(self, subject, payload.into())
            .await
            .map_err(|_| ())
    }

    async fn flush(&self) -> Result<(), ()> {
        Client::flush(self).await.map_err(|_| ())
    }
}
