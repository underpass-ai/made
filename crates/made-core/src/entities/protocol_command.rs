//! [`ProtocolCommand`] — what a caller asks of a running protocol.
//!
//! A command is decided, never applied: [`ProtocolInstance::decide`]
//! turns it into the events it would produce, or refuses it, and only
//! the events change state. One variant per mutation the aggregate
//! accepts, each wrapping a struct that carries exactly what the
//! corresponding mutator takes.
//!
//! Starting a protocol is not here. A command is addressed to an
//! instance that exists; opening one is a constructor,
//! [`ProtocolInstance::decide_start`], which yields the opening event
//! with nothing to decide against.
//!
//! [`ProtocolInstance::decide`]: super::ProtocolInstance::decide
//! [`ProtocolInstance::decide_start`]: super::ProtocolInstance::decide_start

use super::protocol_commands::{
    AcceptChildCompletion, AcknowledgeInterventionDelivery, AdoptChildSpawnPlan,
    ApplyExecutionReceiptResult, ApplyStepResult, ApplyTransition, ApproveGuard, AssertReason,
    BindParticipant, CancelProtocol, CloseIntervention, DeferGuard, EnforceProtocolDeadlines,
    PauseProtocol, PlanProtocolChildren, PlanSuccessor, RenewStepLease, RequestIntervention,
    RespondToIntervention, RespondToInterventionWithEvidence, ResumeProtocol, StartStep,
};

/// A request to change a protocol, in the terms the aggregate decides.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProtocolCommand {
    RecordHostHandoff(super::protocol_commands::RecordHostHandoff),
    BindParticipant(BindParticipant),
    StartStep(StartStep),
    RenewStepLease(RenewStepLease),
    ApplyStepResult(ApplyStepResult),
    ApplyExecutionReceiptResult(ApplyExecutionReceiptResult),
    ApplyTransition(ApplyTransition),
    ApproveGuard(ApproveGuard),
    DeferGuard(DeferGuard),
    RequestIntervention(RequestIntervention),
    RespondToIntervention(RespondToIntervention),
    RespondToInterventionWithEvidence(RespondToInterventionWithEvidence),
    AcknowledgeInterventionDelivery(AcknowledgeInterventionDelivery),
    AssertReason(AssertReason),
    CloseIntervention(CloseIntervention),
    PlanProtocolChildren(PlanProtocolChildren),
    PlanSuccessor(PlanSuccessor),
    AdoptChildSpawnPlan(AdoptChildSpawnPlan),
    AcceptChildCompletion(AcceptChildCompletion),
    PauseProtocol(PauseProtocol),
    ResumeProtocol(ResumeProtocol),
    CancelProtocol(CancelProtocol),
    EnforceProtocolDeadlines(EnforceProtocolDeadlines),
}
