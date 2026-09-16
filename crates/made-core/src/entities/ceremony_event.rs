//! [`CeremonyEvent`] — one thing that happened to a ceremony, with
//! everything it changed.
//!
//! The journal used to record *that* something happened; the event is
//! *what*. Sealed inside an [`AuditRecord`](super::AuditRecord), it is
//! what a fold replays and what the chain's digest now covers.

use serde::{Deserialize, Serialize};

use crate::value_objects::{AuditEventType, EventSchemaVersion};

use super::ceremony_events::{
    CeremonyCompleted, CeremonyInstanceStarted, EvidenceCollected, HumanApprovalRecorded,
    HumanDeferralRecorded, InterventionClosed, InterventionRequested, InterventionResponded,
    MemoryRecalled, ParticipantsBound, ReasonAsserted, StepCompleted, StepFailed, StepStarted,
    TransitionApplied,
};

/// A fact a ceremony's stream can hold, with its full payload.
///
/// One variant per [`AuditEventType`] the engine seals today, under the
/// same name, and tagged on the wire with the same snake_case string
/// `AuditEventType::as_str` returns. Three types of the catalogue have
/// no variant because nothing produces them: `CeremonyDefinitionValidated`
/// and `CeremonyDefinitionPublished` belong to the definition catalogue,
/// not to a ceremony's stream, and `CeremonyFailed` has no producer —
/// a ceremony reaches its end only by a transition into a terminal
/// state, which is a completion. They join when something can emit
/// them.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum CeremonyEvent {
    CeremonyInstanceStarted(CeremonyInstanceStarted),
    ParticipantsBound(ParticipantsBound),
    StepStarted(StepStarted),
    StepCompleted(StepCompleted),
    StepFailed(StepFailed),
    TransitionApplied(TransitionApplied),
    InterventionRequested(InterventionRequested),
    InterventionResponded(InterventionResponded),
    InterventionClosed(InterventionClosed),
    EvidenceCollected(EvidenceCollected),
    ReasonAsserted(ReasonAsserted),
    HumanApprovalRecorded(HumanApprovalRecorded),
    HumanDeferralRecorded(HumanDeferralRecorded),
    CeremonyCompleted(CeremonyCompleted),
    MemoryRecalled(MemoryRecalled),
}

impl CeremonyEvent {
    /// The catalogue entry this event is an instance of.
    #[must_use]
    pub fn event_type(&self) -> AuditEventType {
        match self {
            Self::CeremonyInstanceStarted(_) => AuditEventType::CeremonyInstanceStarted,
            Self::ParticipantsBound(_) => AuditEventType::ParticipantsBound,
            Self::StepStarted(_) => AuditEventType::StepStarted,
            Self::StepCompleted(_) => AuditEventType::StepCompleted,
            Self::StepFailed(_) => AuditEventType::StepFailed,
            Self::TransitionApplied(_) => AuditEventType::TransitionApplied,
            Self::InterventionRequested(_) => AuditEventType::InterventionRequested,
            Self::InterventionResponded(_) => AuditEventType::InterventionResponded,
            Self::InterventionClosed(_) => AuditEventType::InterventionClosed,
            Self::EvidenceCollected(_) => AuditEventType::EvidenceCollected,
            Self::ReasonAsserted(_) => AuditEventType::ReasonAsserted,
            Self::HumanApprovalRecorded(_) => AuditEventType::HumanApprovalRecorded,
            Self::HumanDeferralRecorded(_) => AuditEventType::HumanDeferralRecorded,
            Self::CeremonyCompleted(_) => AuditEventType::CeremonyCompleted,
            Self::MemoryRecalled(_) => AuditEventType::MemoryRecalled,
        }
    }

    /// The shape this event's payload is written in.
    ///
    /// Listed per variant rather than as one constant, so a payload
    /// that changes shape bumps its own version and nobody else's.
    #[must_use]
    pub fn schema_version(&self) -> EventSchemaVersion {
        match self {
            Self::CeremonyInstanceStarted(_)
            | Self::ParticipantsBound(_)
            | Self::StepStarted(_)
            | Self::StepCompleted(_)
            | Self::StepFailed(_)
            | Self::TransitionApplied(_)
            | Self::InterventionRequested(_)
            | Self::InterventionResponded(_)
            | Self::InterventionClosed(_)
            | Self::EvidenceCollected(_)
            | Self::ReasonAsserted(_)
            | Self::HumanApprovalRecorded(_)
            | Self::HumanDeferralRecorded(_)
            | Self::CeremonyCompleted(_)
            | Self::MemoryRecalled(_) => EventSchemaVersion::V1,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::value_objects::StateId;
    use time::macros::datetime;

    fn completed() -> CeremonyEvent {
        CeremonyEvent::CeremonyCompleted(CeremonyCompleted {
            final_state: StateId::new("DONE").unwrap(),
            completed_at: datetime!(2026-07-29 09:00:00 UTC),
        })
    }

    #[test]
    fn the_wire_tag_is_the_event_type_name() {
        let json = serde_json::to_value(completed()).unwrap();

        assert_eq!(json["type"], AuditEventType::CeremonyCompleted.as_str());
        assert_eq!(json["final_state"], "DONE");
        assert_eq!(json["completed_at"], "2026-07-29T09:00:00Z");
    }

    #[test]
    fn every_payload_is_at_version_one_today() {
        assert_eq!(completed().schema_version(), EventSchemaVersion::V1);
        assert_eq!(completed().event_type(), AuditEventType::CeremonyCompleted);
    }

    #[test]
    fn a_tag_with_a_foreign_shape_does_not_deserialize() {
        let json = serde_json::json!({ "type": "ceremony_completed", "final_state": "DONE" });

        assert!(serde_json::from_value::<CeremonyEvent>(json).is_err());
    }
}
