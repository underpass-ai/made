//! [`CeremonyCommand`] — what a caller asks of a running ceremony.
//!
//! A command is decided, never applied: [`CeremonyInstance::decide`]
//! turns it into the events it would produce, or refuses it, and only
//! the events change state. One variant per mutation the aggregate
//! accepts, each wrapping a struct that carries exactly what the
//! corresponding mutator takes.
//!
//! Starting a ceremony is not here. A command is addressed to an
//! instance that exists; opening one is a constructor,
//! [`CeremonyInstance::decide_start`], which yields the opening event
//! with nothing to decide against.
//!
//! [`CeremonyInstance::decide`]: super::CeremonyInstance::decide
//! [`CeremonyInstance::decide_start`]: super::CeremonyInstance::decide_start

use super::ceremony_commands::{
    AcceptChildCompletion, AdoptChildSpawnPlan, ApplyStepResult, ApplyTransition, ApproveGuard,
    AssertReason, BindParticipant, CancelCeremony, CloseIntervention, DeferGuard,
    EnforceCeremonyDeadlines, PauseCeremony, PlanCeremonyChildren, RequestIntervention,
    RespondToIntervention, RespondToInterventionWithEvidence, ResumeCeremony, StartStep,
};

/// A request to change a ceremony, in the terms the aggregate decides.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CeremonyCommand {
    BindParticipant(BindParticipant),
    StartStep(StartStep),
    ApplyStepResult(ApplyStepResult),
    ApplyTransition(ApplyTransition),
    ApproveGuard(ApproveGuard),
    DeferGuard(DeferGuard),
    RequestIntervention(RequestIntervention),
    RespondToIntervention(RespondToIntervention),
    RespondToInterventionWithEvidence(RespondToInterventionWithEvidence),
    AssertReason(AssertReason),
    CloseIntervention(CloseIntervention),
    PlanCeremonyChildren(PlanCeremonyChildren),
    AdoptChildSpawnPlan(AdoptChildSpawnPlan),
    AcceptChildCompletion(AcceptChildCompletion),
    PauseCeremony(PauseCeremony),
    ResumeCeremony(ResumeCeremony),
    CancelCeremony(CancelCeremony),
    EnforceCeremonyDeadlines(EnforceCeremonyDeadlines),
}
