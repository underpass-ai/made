//! [`CeremonyEvent`] — one thing that happened to a ceremony, with
//! everything it changed.
//!
//! The journal used to record *that* something happened; the event is
//! *what*. Sealed inside an [`AuditRecord`](super::AuditRecord), it is
//! what a fold replays and what the chain's digest now covers.

use serde::{Deserialize, Serialize};

use crate::value_objects::{AuditEventType, EventSchemaVersion};

use super::ceremony_events::{
    CeremonyCancelled, CeremonyCompleted, CeremonyDeadlineExceeded, CeremonyInstanceStarted,
    CeremonyPaused, CeremonyResumed, ChildCompletionAccepted, ChildSpawnPlanAdopted,
    ChildSpawnPlanned, ContextWritten, EvidenceCollected, ExecutionReceiptLinked,
    HumanApprovalRecorded, HumanDeferralRecorded, InstanceImported, InterventionClosed,
    InterventionRequested, InterventionResponded, LateStepResultObserved, MemoryRecalled,
    ParticipantsBound, ReasonAsserted, StateDeadlineExceeded, StateIterationStarted, StepCompleted,
    StepDeadlineExceeded, StepFailed, StepLeaseRenewed, StepStarted, SuccessionCarried,
    SuccessorPlanned, TransitionApplied,
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
    HostHandoffRecorded(super::ceremony_events::HostHandoffRecorded),
    CeremonyInstanceStarted(CeremonyInstanceStarted),
    ParticipantsBound(ParticipantsBound),
    StepStarted(StepStarted),
    StepLeaseRenewed(StepLeaseRenewed),
    StepCompleted(StepCompleted),
    StepFailed(StepFailed),
    ContextWritten(ContextWritten),
    StateIterationStarted(StateIterationStarted),
    TransitionApplied(TransitionApplied),
    InterventionRequested(InterventionRequested),
    InterventionResponded(InterventionResponded),
    InterventionClosed(InterventionClosed),
    EvidenceCollected(EvidenceCollected),
    ReasonAsserted(ReasonAsserted),
    HumanApprovalRecorded(HumanApprovalRecorded),
    HumanDeferralRecorded(HumanDeferralRecorded),
    CeremonyCompleted(CeremonyCompleted),
    InstanceImported(InstanceImported),
    MemoryRecalled(MemoryRecalled),
    ChildSpawnPlanned(ChildSpawnPlanned),
    ChildSpawnPlanAdopted(ChildSpawnPlanAdopted),
    ChildCompletionAccepted(ChildCompletionAccepted),
    CeremonyPaused(CeremonyPaused),
    CeremonyResumed(CeremonyResumed),
    CeremonyCancelled(CeremonyCancelled),
    CeremonyDeadlineExceeded(CeremonyDeadlineExceeded),
    StateDeadlineExceeded(StateDeadlineExceeded),
    StepDeadlineExceeded(StepDeadlineExceeded),
    LateStepResultObserved(LateStepResultObserved),
    ExecutionReceiptLinked(ExecutionReceiptLinked),
    SuccessorPlanned(SuccessorPlanned),
    SuccessionCarried(SuccessionCarried),
}

impl CeremonyEvent {
    /// The catalogue entry this event is an instance of.
    #[must_use]
    pub fn event_type(&self) -> AuditEventType {
        match self {
            Self::HostHandoffRecorded(_) => AuditEventType::HostHandoffRecorded,
            Self::CeremonyInstanceStarted(_) => AuditEventType::CeremonyInstanceStarted,
            Self::ParticipantsBound(_) => AuditEventType::ParticipantsBound,
            Self::StepStarted(_) => AuditEventType::StepStarted,
            Self::StepLeaseRenewed(_) => AuditEventType::StepLeaseRenewed,
            Self::StepCompleted(_) => AuditEventType::StepCompleted,
            Self::StepFailed(_) => AuditEventType::StepFailed,
            Self::ContextWritten(_) => AuditEventType::ContextWritten,
            Self::StateIterationStarted(_) => AuditEventType::StateIterationStarted,
            Self::TransitionApplied(_) => AuditEventType::TransitionApplied,
            Self::InterventionRequested(_) => AuditEventType::InterventionRequested,
            Self::InterventionResponded(_) => AuditEventType::InterventionResponded,
            Self::InterventionClosed(_) => AuditEventType::InterventionClosed,
            Self::EvidenceCollected(_) => AuditEventType::EvidenceCollected,
            Self::ReasonAsserted(_) => AuditEventType::ReasonAsserted,
            Self::HumanApprovalRecorded(_) => AuditEventType::HumanApprovalRecorded,
            Self::HumanDeferralRecorded(_) => AuditEventType::HumanDeferralRecorded,
            Self::CeremonyCompleted(_) => AuditEventType::CeremonyCompleted,
            Self::InstanceImported(_) => AuditEventType::InstanceImported,
            Self::MemoryRecalled(_) => AuditEventType::MemoryRecalled,
            Self::ChildSpawnPlanned(_) => AuditEventType::ChildSpawnPlanned,
            Self::ChildSpawnPlanAdopted(_) => AuditEventType::ChildSpawnPlanAdopted,
            Self::ChildCompletionAccepted(_) => AuditEventType::ChildCompletionAccepted,
            Self::CeremonyPaused(_) => AuditEventType::CeremonyPaused,
            Self::CeremonyResumed(_) => AuditEventType::CeremonyResumed,
            Self::CeremonyCancelled(_) => AuditEventType::CeremonyCancelled,
            Self::CeremonyDeadlineExceeded(_) => AuditEventType::CeremonyDeadlineExceeded,
            Self::StateDeadlineExceeded(_) => AuditEventType::StateDeadlineExceeded,
            Self::StepDeadlineExceeded(_) => AuditEventType::StepDeadlineExceeded,
            Self::LateStepResultObserved(_) => AuditEventType::LateStepResultObserved,
            Self::ExecutionReceiptLinked(_) => AuditEventType::ExecutionReceiptLinked,
            Self::SuccessorPlanned(_) => AuditEventType::SuccessorPlanned,
            Self::SuccessionCarried(_) => AuditEventType::SuccessionCarried,
        }
    }

    /// The shape this event's payload is written in.
    ///
    /// Listed per variant rather than as one constant, so a payload
    /// that changes shape bumps its own version and nobody else's.
    #[must_use]
    pub fn schema_version(&self) -> EventSchemaVersion {
        match self {
            Self::StepStarted(event) => {
                if event.budget_reservation_id.is_some() {
                    EventSchemaVersion::V6
                } else if event.deadline.is_some() {
                    EventSchemaVersion::V5
                } else if event.state_visit.is_some() {
                    EventSchemaVersion::V4
                } else if event.role_from.is_some() || event.sealed_role.is_some() {
                    EventSchemaVersion::V3
                } else {
                    event
                        .state_iteration
                        .map_or(EventSchemaVersion::V1, |_| EventSchemaVersion::V2)
                }
            }
            Self::StepCompleted(event) => {
                if event.state_visit.is_some() {
                    EventSchemaVersion::V3
                } else {
                    event
                        .state_iteration
                        .map_or(EventSchemaVersion::V1, |_| EventSchemaVersion::V2)
                }
            }
            Self::StepFailed(event) => {
                if event.result.failure_kind().is_some() {
                    EventSchemaVersion::V4
                } else if event.state_visit.is_some() {
                    EventSchemaVersion::V3
                } else {
                    event
                        .state_iteration
                        .map_or(EventSchemaVersion::V1, |_| EventSchemaVersion::V2)
                }
            }
            Self::TransitionApplied(event) => {
                if event
                    .destination
                    .as_ref()
                    .is_some_and(|destination| destination.deadline.is_some())
                {
                    EventSchemaVersion::V4
                } else if event.destination.is_some() || event.transition.has_explicit_state_visit()
                {
                    EventSchemaVersion::V3
                } else if event.transition.has_explicit_state_iteration() {
                    EventSchemaVersion::V2
                } else {
                    EventSchemaVersion::V1
                }
            }
            Self::StateIterationStarted(event) => event
                .state_visit
                .map_or(EventSchemaVersion::V1, |_| EventSchemaVersion::V2),
            Self::ContextWritten(event) => event
                .state_visit
                .map_or(EventSchemaVersion::V1, |_| EventSchemaVersion::V2),
            Self::CeremonyInstanceStarted(event) if event.budget_account_id.is_some() => {
                EventSchemaVersion::V4
            }
            Self::CeremonyInstanceStarted(event)
                if event.ceremony_deadline.is_some() || event.state_deadline.is_some() =>
            {
                EventSchemaVersion::V3
            }
            Self::CeremonyInstanceStarted(event) if event.lineage.is_some() => {
                EventSchemaVersion::V2
            }
            Self::StepLeaseRenewed(event) if event.request.is_some() => EventSchemaVersion::V2,
            Self::HostHandoffRecorded(_)
            | Self::CeremonyInstanceStarted(_)
            | Self::StepLeaseRenewed(_)
            | Self::ParticipantsBound(_)
            | Self::InterventionRequested(_)
            | Self::InterventionResponded(_)
            | Self::InterventionClosed(_)
            | Self::EvidenceCollected(_)
            | Self::ReasonAsserted(_)
            | Self::HumanApprovalRecorded(_)
            | Self::HumanDeferralRecorded(_)
            | Self::CeremonyCompleted(_)
            | Self::InstanceImported(_)
            | Self::MemoryRecalled(_)
            | Self::ChildSpawnPlanned(_)
            | Self::ChildSpawnPlanAdopted(_)
            | Self::ChildCompletionAccepted(_)
            | Self::CeremonyPaused(_)
            | Self::CeremonyResumed(_)
            | Self::CeremonyCancelled(_)
            | Self::CeremonyDeadlineExceeded(_)
            | Self::StateDeadlineExceeded(_)
            | Self::StepDeadlineExceeded(_)
            | Self::LateStepResultObserved(_)
            | Self::ExecutionReceiptLinked(_)
            | Self::SuccessorPlanned(_)
            | Self::SuccessionCarried(_) => EventSchemaVersion::V1,
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
