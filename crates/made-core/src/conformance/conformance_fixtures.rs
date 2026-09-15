//! Builders shared by the conformance suites.
//!
//! A suite that fabricated its own ceremonies differently from the ones
//! the engine produces would be testing a shape nothing else uses.

use serde_json::json;
use time::OffsetDateTime;

use crate::entities::ceremony_events::StepCompleted;
use crate::entities::{
    AuditFact, CeremonyCommit, CeremonyDefinition, CeremonyEvent, CeremonyInstance,
};
use crate::error::DomainError;
use crate::value_objects::{
    AuditActor, AuditActorKind, CeremonyContext, CeremonyId, CeremonyName, CeremonyState,
    CeremonyTransition, CeremonyVersion, EventId, ExpectedRevision, OutboxMessage, OutboxSubject,
    RoleId, StateId, StepAttempt, StepId, StepIteration, StepOutput, StepResult, TransitionTrigger,
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

pub(super) fn outbox_message(event: &str) -> Result<OutboxMessage, DomainError> {
    OutboxMessage::new(
        EventId::new(event)?,
        OutboxSubject::new("conformance.message")?,
        json!({ "event": event }),
        OffsetDateTime::UNIX_EPOCH,
    )
}

/// A commit carrying one audit fact and `events.len()` messages.
pub(super) fn commit_with(
    ceremony_id: &CeremonyId,
    expected: ExpectedRevision,
    fact_event: &str,
    message_events: &[&str],
) -> Result<CeremonyCommit, DomainError> {
    let definition = definition()?;
    let instance = CeremonyInstance::start(
        ceremony_id.clone(),
        &definition,
        CeremonyContext::empty(),
        OffsetDateTime::UNIX_EPOCH,
    );
    let messages = message_events
        .iter()
        .map(|event| outbox_message(event))
        .collect::<Result<Vec<_>, _>>()?;
    CeremonyCommit::new(
        instance,
        expected,
        [audit_fact(fact_event, ceremony_id, &definition)?],
        messages,
    )
}
