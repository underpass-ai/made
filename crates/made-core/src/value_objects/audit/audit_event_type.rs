use serde::{Deserialize, Serialize};

/// The audited fact a record states.
///
/// The catalogue only names facts the engine can emit today. A type
/// that nothing can produce is dead vocabulary, and in an audit
/// catalogue it is worse than dead: it suggests coverage that does not
/// exist. Publication and artifacts join when their concepts do.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AuditEventType {
    HostHandoffRecorded,
    CeremonyDefinitionValidated,
    CeremonyDefinitionPublished,
    CeremonyInstanceStarted,
    StepStarted,
    StepLeaseRenewed,
    StepCompleted,
    StepFailed,
    ContextWritten,
    StateIterationStarted,
    TransitionApplied,
    InterventionRequested,
    InterventionResponded,
    InterventionClosed,
    EvidenceCollected,
    ParticipantsBound,
    ReasonAsserted,
    HumanApprovalRecorded,
    HumanDeferralRecorded,
    ChildSpawnPlanned,
    ChildSpawnPlanAdopted,
    ChildCompletionAccepted,
    CeremonyCompleted,
    CeremonyFailed,
    /// A session written before ceremonies were streams was brought
    /// into one. It is the only fact the engine seals that did not
    /// happen inside the ceremony it belongs to.
    InstanceImported,
    MemoryRecalled,
    CeremonyPaused,
    CeremonyResumed,
    CeremonyCancelled,
    CeremonyDeadlineExceeded,
    StateDeadlineExceeded,
    StepDeadlineExceeded,
    LateStepResultObserved,
    ExecutionReceiptLinked,
    /// A handoff to a successor was sealed in the ceremony handing off.
    SuccessorPlanned,
    /// A successor recorded what it was given to start from.
    SuccessionCarried,
}

impl AuditEventType {
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::HostHandoffRecorded => "host_handoff_recorded",
            Self::CeremonyDefinitionValidated => "ceremony_definition_validated",
            Self::CeremonyDefinitionPublished => "ceremony_definition_published",
            Self::CeremonyInstanceStarted => "ceremony_instance_started",
            Self::StepStarted => "step_started",
            Self::StepLeaseRenewed => "step_lease_renewed",
            Self::StepCompleted => "step_completed",
            Self::StepFailed => "step_failed",
            Self::ContextWritten => "context_written",
            Self::StateIterationStarted => "state_iteration_started",
            Self::TransitionApplied => "transition_applied",
            Self::InterventionRequested => "intervention_requested",
            Self::InterventionResponded => "intervention_responded",
            Self::InterventionClosed => "intervention_closed",
            Self::EvidenceCollected => "evidence_collected",
            Self::ParticipantsBound => "participants_bound",
            Self::ReasonAsserted => "reason_asserted",
            Self::HumanApprovalRecorded => "human_approval_recorded",
            Self::HumanDeferralRecorded => "human_deferral_recorded",
            Self::ChildSpawnPlanned => "child_spawn_planned",
            Self::ChildSpawnPlanAdopted => "child_spawn_plan_adopted",
            Self::ChildCompletionAccepted => "child_completion_accepted",
            Self::CeremonyCompleted => "ceremony_completed",
            Self::CeremonyFailed => "ceremony_failed",
            Self::InstanceImported => "instance_imported",
            Self::MemoryRecalled => "memory_recalled",
            Self::CeremonyPaused => "ceremony_paused",
            Self::CeremonyResumed => "ceremony_resumed",
            Self::CeremonyCancelled => "ceremony_cancelled",
            Self::CeremonyDeadlineExceeded => "ceremony_deadline_exceeded",
            Self::StateDeadlineExceeded => "state_deadline_exceeded",
            Self::StepDeadlineExceeded => "step_deadline_exceeded",
            Self::LateStepResultObserved => "late_step_result_observed",
            Self::ExecutionReceiptLinked => "execution_receipt_linked",
            Self::SuccessorPlanned => "successor_planned",
            Self::SuccessionCarried => "succession_carried",
        }
    }

    /// Whether the fact terminates the ceremony it belongs to.
    #[must_use]
    pub fn is_terminal(self) -> bool {
        matches!(
            self,
            Self::CeremonyCompleted
                | Self::CeremonyFailed
                | Self::CeremonyCancelled
                | Self::CeremonyDeadlineExceeded
                | Self::StateDeadlineExceeded
        )
    }

    /// Whether the fact records a decision only a human can make.
    #[must_use]
    pub fn records_human_authority(self) -> bool {
        matches!(
            self,
            Self::HumanApprovalRecorded | Self::HumanDeferralRecorded
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_wire_name_matches_the_serialized_name() {
        for event_type in [
            AuditEventType::CeremonyInstanceStarted,
            AuditEventType::HumanApprovalRecorded,
            AuditEventType::CeremonyFailed,
        ] {
            let serialized = serde_json::to_string(&event_type).unwrap();

            assert_eq!(serialized, format!("\"{}\"", event_type.as_str()));
        }
    }

    #[test]
    fn only_the_two_ceremony_endings_are_terminal() {
        assert!(AuditEventType::CeremonyCompleted.is_terminal());
        assert!(AuditEventType::CeremonyFailed.is_terminal());
        assert!(!AuditEventType::StepCompleted.is_terminal());
    }

    #[test]
    fn human_authority_is_limited_to_approvals_and_deferrals() {
        assert!(AuditEventType::HumanApprovalRecorded.records_human_authority());
        assert!(AuditEventType::HumanDeferralRecorded.records_human_authority());
        assert!(!AuditEventType::InterventionResponded.records_human_authority());
    }
}
