use async_trait::async_trait;

/// Payload-free wake signal for appended host activity.
#[async_trait]
pub trait CeremonyAgentActivitySubscriptionPort: Send {
    async fn wait(&mut self);
}
