use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

use crate::value_objects::{HostDeliveryId, HostDeliveryObservation};

use super::DeliveryRecipient;

/// The sealed proof that a named agent saw a named intervention.
///
/// The one transport-adjacent fact that belongs in the journal, and it
/// is here because it is not a transport fact: queueing, leasing and
/// pushing are the engine's attempts, while this is a statement by the
/// host about what it observed. A host-reported status label saying an
/// intervention was delivered is a claim; this is the evidence.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InterventionDeliveryAck {
    delivery_id: HostDeliveryId,
    recipient: DeliveryRecipient,
    observation: HostDeliveryObservation,
    #[serde(with = "time::serde::rfc3339")]
    acknowledged_at: OffsetDateTime,
}

impl InterventionDeliveryAck {
    #[must_use]
    pub const fn new(
        delivery_id: HostDeliveryId,
        recipient: DeliveryRecipient,
        observation: HostDeliveryObservation,
        acknowledged_at: OffsetDateTime,
    ) -> Self {
        Self {
            delivery_id,
            recipient,
            observation,
            acknowledged_at,
        }
    }

    #[must_use]
    pub const fn delivery_id(&self) -> &HostDeliveryId {
        &self.delivery_id
    }

    #[must_use]
    pub const fn recipient(&self) -> &DeliveryRecipient {
        &self.recipient
    }

    #[must_use]
    pub const fn observation(&self) -> &HostDeliveryObservation {
        &self.observation
    }

    #[must_use]
    pub const fn acknowledged_at(&self) -> OffsetDateTime {
        self.acknowledged_at
    }

    /// Whether this acknowledgement is the same fact as another.
    ///
    /// Two acknowledgements of one delivery agree only if the same
    /// recipient said the same thing. A host that retried after losing
    /// its answer repeats itself and is recognised; a host that changed
    /// its answer is a conflict, because the first answer was already
    /// sealed and the stream does not overwrite.
    #[must_use]
    pub fn agrees_with(&self, other: &Self) -> bool {
        self.delivery_id == other.delivery_id
            && self.recipient == other.recipient
            && self.observation == other.observation
    }
}

#[cfg(test)]
mod tests {
    use time::macros::datetime;

    use super::*;
    use crate::value_objects::{
        CeremonyAgentExecutionId, DeliveryNote, HostAgentIncarnation, HostDeliveryObservationKind,
        RoleId,
    };

    fn ack(note: &str, at: OffsetDateTime) -> InterventionDeliveryAck {
        InterventionDeliveryAck::new(
            HostDeliveryId::new("c-1:intervention:i-1:agent:exec-1:inc-1").unwrap(),
            DeliveryRecipient::new(
                CeremonyAgentExecutionId::new("exec-1").unwrap(),
                HostAgentIncarnation::new("inc-1").unwrap(),
                RoleId::new("ENGINEER").unwrap(),
            ),
            HostDeliveryObservation::new(
                HostDeliveryObservationKind::Received,
                at,
                None,
                DeliveryNote::new(note).unwrap(),
            ),
            at,
        )
    }

    #[test]
    fn a_repeat_agrees_and_a_changed_answer_does_not() {
        let at = datetime!(2026-09-20 10:00:00 UTC);
        assert!(ack("picked it up", at).agrees_with(&ack("picked it up", at)));
        assert!(!ack("picked it up", at).agrees_with(&ack("second thoughts", at)));
    }

    #[test]
    fn the_time_it_was_sealed_does_not_make_two_facts_disagree() {
        let first = ack("picked it up", datetime!(2026-09-20 10:00:00 UTC));
        let later = InterventionDeliveryAck::new(
            first.delivery_id().clone(),
            first.recipient().clone(),
            first.observation().clone(),
            datetime!(2026-09-20 10:00:05 UTC),
        );
        assert!(first.agrees_with(&later));
    }
}
