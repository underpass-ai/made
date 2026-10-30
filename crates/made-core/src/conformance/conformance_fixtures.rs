//! Builders shared by the conformance suites.
//!
//! A suite that fabricated its own protocols differently from the ones
//! the engine produces would be testing a shape nothing else uses.

use time::OffsetDateTime;

use crate::entities::protocol_events::StepCompleted;
use crate::entities::{AuditFact, ProtocolDefinition, ProtocolEvent};
use crate::error::DomainError;
use crate::value_objects::{
    AuditActor, AuditActorKind, EventId, ProtocolId, ProtocolName, ProtocolState,
    ProtocolTransition, ProtocolVersion, RoleId, StateId, StepAttempt, StepId, StepIteration,
    StepOutput, StepResult, TransitionTrigger,
};

pub(super) fn definition() -> Result<ProtocolDefinition, DomainError> {
    ProtocolDefinition::new(
        ProtocolName::new("conformance_protocol")?,
        ProtocolVersion::v1(),
        None,
        Vec::new(),
        Vec::new(),
        vec![
            ProtocolState::initial(StateId::new("OPEN")?),
            ProtocolState::terminal(StateId::new("DONE")?),
        ],
        vec![ProtocolTransition::new(
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
pub(super) fn protocol_event() -> Result<ProtocolEvent, DomainError> {
    Ok(ProtocolEvent::StepCompleted(StepCompleted {
        state_visit: None,
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
    protocol_id: &ProtocolId,
    definition: &ProtocolDefinition,
) -> Result<AuditFact, DomainError> {
    Ok(AuditFact {
        event_id: EventId::new(event)?,
        event: protocol_event()?,
        protocol_id: protocol_id.clone(),
        definition_name: definition.name().clone(),
        definition_version: definition.version().clone(),
        occurred_at: OffsetDateTime::UNIX_EPOCH,
        actor: AuditActor::new("conformance", AuditActorKind::Engine, None)?,
        correlation_id: None,
        causation_id: None,
        trace: None,
    })
}
