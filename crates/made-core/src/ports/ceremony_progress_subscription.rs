use async_trait::async_trait;

/// One payload-free signal source for newly appended ceremony events.
#[async_trait]
pub trait CeremonyProgressSubscriptionPort: Send {
    /// Wait until the local append generation changes.
    async fn wait(&mut self);
}
