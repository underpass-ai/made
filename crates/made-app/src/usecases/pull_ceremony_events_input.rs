use made_core::value_objects::{CeremonyEventConsumer, CeremonyEventPageLimit, GlobalPosition};

/// One read of a named global ceremony-event feed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PullCeremonyEventsInput {
    consumer: CeremonyEventConsumer,
    limit: CeremonyEventPageLimit,
    acknowledge_through: Option<GlobalPosition>,
}

impl PullCeremonyEventsInput {
    #[must_use]
    pub fn new(
        consumer: CeremonyEventConsumer,
        limit: CeremonyEventPageLimit,
        acknowledge_through: Option<GlobalPosition>,
    ) -> Self {
        Self {
            consumer,
            limit,
            acknowledge_through,
        }
    }

    #[must_use]
    pub fn consumer(&self) -> &CeremonyEventConsumer {
        &self.consumer
    }

    #[must_use]
    pub const fn limit(&self) -> CeremonyEventPageLimit {
        self.limit
    }

    #[must_use]
    pub const fn acknowledge_through(&self) -> Option<GlobalPosition> {
        self.acknowledge_through
    }
}
