//! Builders shared by the conformance suites.
//!
//! A suite that fabricated its own ceremonies differently from the ones
//! the engine produces would be testing a shape nothing else uses.

use time::OffsetDateTime;

use crate::entities::ceremony_events::StepCompleted;
use crate::entities::{AuditFact, CeremonyDefinition, CeremonyEvent};
use crate::error::DomainError;
use crate::value_objects::{
    AuditActor, AuditActorKind, CeremonyId, CeremonyName, CeremonyState, CeremonyTransition,
    CeremonyVersion, EventId, RoleId, StateId, StepAttempt, StepId, StepIteration, StepOutput,
    StepResult, TransitionTrigger,
};

pub(super) fn definition() -> Result<CeremonyDefinition, DomainError> {
    CeremonyDefinition::new(
        CeremonyName::new("conformance_ceremony")?,
        CeremonyVersion::v1(),
        None,
        Vec::new(),
        Vec::new(),
        vec![
            CeremonyState::initial(StateId::new("OPEN")?),
            CeremonyState::terminal(StateId::new("DONE")?),
        ],
        vec![CeremonyTransition::new(
            StateId::new("OPEN")?,
            StateId::new("DONE")?,
            TransitionTrigger::new("finish")?,
            Vec::new(),
        )?],
        Vec::new(),
        Vec::new(),
        Vec::new(),
    )
}

/// The one event the suites seal: a step that completed with no output.
pub(super) fn ceremony_event() -> Result<CeremonyEvent, DomainError> {
    Ok(CeremonyEvent::StepCompleted(StepCompleted {
        step_id: StepId::new("conformance_step")?,
        state_iteration: None,
        iteration: StepIteration::FIRST,
        attempt: StepAttempt::FIRST,
        result: StepResult::completed(StepOutput::empty())?,
        next_iteration: None,
        finished_by: RoleId::new("conformance")?,
        finished_at: OffsetDateTime::UNIX_EPOCH,
    }))
}

pub(super) fn audit_fact(
    event: &str,
    ceremony_id: &CeremonyId,
    definition: &CeremonyDefinition,
) -> Result<AuditFact, DomainError> {
    Ok(AuditFact {
        event_id: EventId::new(event)?,
        event: ceremony_event()?,
        ceremony_id: ceremony_id.clone(),
        definition_name: definition.name().clone(),
        definition_version: definition.version().clone(),
        occurred_at: OffsetDateTime::UNIX_EPOCH,
        actor: AuditActor::new("conformance", AuditActorKind::Engine, None)?,
        correlation_id: None,
        causation_id: None,
        trace: None,
    })
}
