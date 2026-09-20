use made_core::error::DomainError;
use made_core::ports::HostDeliveryPageLimit;
use made_core::value_objects::{CeremonyId, DeliveryRecipient, DurationMs};

const DEFAULT_LEASE_MS: u64 = 60_000;

/// What a working agent asks for when it asks for its questions.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PullCeremonyAgentInterventionsInput {
    pub(crate) instance_id: CeremonyId,
    pub(crate) recipient: DeliveryRecipient,
    pub(crate) lease_duration: DurationMs,
    pub(crate) limit: HostDeliveryPageLimit,
}

impl PullCeremonyAgentInterventionsInput {
    #[must_use]
    pub fn new(instance_id: CeremonyId, recipient: DeliveryRecipient) -> Self {
        Self {
            instance_id,
            recipient,
            lease_duration: DurationMs::from_millis(DEFAULT_LEASE_MS),
            limit: HostDeliveryPageLimit::default(),
        }
    }

    /// How long the agent asks to hold what it is handed.
    ///
    /// A lease and not a take: an agent that dies holding one strands
    /// nothing, because the offer comes back when the lease runs out.
    pub fn leased_for(mut self, lease_duration: DurationMs) -> Result<Self, DomainError> {
        if lease_duration.get() == 0 {
            return Err(DomainError::MustBeNonZero {
                field: "pull_interventions.lease_duration",
            });
        }
        self.lease_duration = lease_duration;
        Ok(self)
    }

    #[must_use]
    pub const fn of_size(mut self, limit: HostDeliveryPageLimit) -> Self {
        self.limit = limit;
        self
    }

    #[must_use]
    pub const fn instance_id(&self) -> &CeremonyId {
        &self.instance_id
    }

    #[must_use]
    pub const fn recipient(&self) -> &DeliveryRecipient {
        &self.recipient
    }
}
