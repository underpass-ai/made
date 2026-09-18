use made_core::value_objects::{
    CeremonyEventPageLimit, CeremonyId, CeremonyProgressWait, StreamVersion,
};

/// Resume point and bounds for one ceremony progress stream.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StreamCeremonyInput {
    ceremony_id: CeremonyId,
    after_sequence: StreamVersion,
    max_events: CeremonyEventPageLimit,
    wait_timeout: CeremonyProgressWait,
}

impl StreamCeremonyInput {
    #[must_use]
    pub const fn new(
        ceremony_id: CeremonyId,
        after_sequence: StreamVersion,
        max_events: CeremonyEventPageLimit,
        wait_timeout: CeremonyProgressWait,
    ) -> Self {
        Self {
            ceremony_id,
            after_sequence,
            max_events,
            wait_timeout,
        }
    }

    #[must_use]
    pub const fn ceremony_id(&self) -> &CeremonyId {
        &self.ceremony_id
    }

    #[must_use]
    pub const fn after_sequence(&self) -> StreamVersion {
        self.after_sequence
    }

    #[must_use]
    pub const fn max_events(&self) -> CeremonyEventPageLimit {
        self.max_events
    }

    #[must_use]
    pub const fn wait_timeout(&self) -> CeremonyProgressWait {
        self.wait_timeout
    }
}
