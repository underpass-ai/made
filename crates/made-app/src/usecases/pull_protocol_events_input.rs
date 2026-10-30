use made_core::value_objects::{GlobalPosition, ProtocolEventConsumer, ProtocolEventPageLimit};

/// One read of a named global protocol-event feed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PullProtocolEventsInput {
    consumer: ProtocolEventConsumer,
    limit: ProtocolEventPageLimit,
    acknowledge_through: Option<GlobalPosition>,
}

impl PullProtocolEventsInput {
    #[must_use]
    pub fn new(
        consumer: ProtocolEventConsumer,
        limit: ProtocolEventPageLimit,
        acknowledge_through: Option<GlobalPosition>,
    ) -> Self {
        Self {
            consumer,
            limit,
            acknowledge_through,
        }
    }

    #[must_use]
    pub fn consumer(&self) -> &ProtocolEventConsumer {
        &self.consumer
    }

    #[must_use]
    pub const fn limit(&self) -> ProtocolEventPageLimit {
        self.limit
    }

    #[must_use]
    pub const fn acknowledge_through(&self) -> Option<GlobalPosition> {
        self.acknowledge_through
    }
}
