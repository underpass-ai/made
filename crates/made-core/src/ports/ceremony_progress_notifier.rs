use super::CeremonyProgressSubscriptionPort;

/// Creates process-local append wake subscriptions.
pub trait CeremonyProgressNotifierPort: Send + Sync {
    /// Subscribe before reading the store so an append cannot hide in that race.
    fn subscribe(&self) -> Box<dyn CeremonyProgressSubscriptionPort>;
}
