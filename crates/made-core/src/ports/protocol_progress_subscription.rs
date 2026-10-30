use async_trait::async_trait;

/// One payload-free signal source for newly appended protocol events.
#[async_trait]
pub trait ProtocolProgressSubscriptionPort: Send {
    /// Wait until the local append generation changes.
    async fn wait(&mut self);
}
