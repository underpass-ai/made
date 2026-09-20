//! Which sealed records an integrator is woken for, and why.
//!
//! One function per reading, all of them pure: a record in, at most
//! one [`AttentionEvent`] out. Nothing here reads a store or writes
//! one, which is what lets the whole mapping be tested against
//! fixtures instead of against a running ceremony.
//!
//! # What is not here
//!
//! A rejected review has two readings (ADR 022). The documented
//! convention — an output that says `accepted: false` — is decided
//! here, because the record carries it. The other, a guard on an
//! output field whose value only admits transitions back to states
//! already visited, needs the definition and the visit history, so it
//! belongs to the projector that holds them.
//!
//! Host-reported sources — a participant declaring itself finished, a
//! participant declaring itself blocked, silence for longer than the
//! policy allows — are not records at all, so they are not decided
//! here either.

use made_core::entities::{AuditRecord, CeremonyEvent};
use made_core::error::DomainError;
use made_core::ports::PositionedRecord;
use made_core::value_objects::{
    AttentionEventId, AttentionKind, AttentionReason, EvidenceReference, RoleId, StepId,
};

use super::{AttentionEvent, EventRef, ResultAcceptance};

/// The output key by which a reviewing step says whether it accepted
/// the work. Documented convention rather than a sealed field: a
/// ceremony that does not use it simply never produces this reading.
const ACCEPTED_KEY: &str = "accepted";

/// Read one sealed record as news for the integrator bound to `role`.
///
/// `None` means the record says nothing this integrator can act on,
/// which is most of them. A paused ceremony deliberately produces
/// nothing: pausing is a state of the loop, not an event in it.
pub fn attention_for(
    record: &PositionedRecord,
    integrator: &RoleId,
) -> Result<Option<AttentionEvent>, DomainError> {
    let Some(event) = record.record.event() else {
        return Ok(None);
    };
    match event {
        CeremonyEvent::StepCompleted(completed) => {
            step_completed(record, &completed.step_id).map(Some)
        }
        CeremonyEvent::StepFailed(failed) => step_failed(record, Some(&failed.step_id)).map(Some),
        CeremonyEvent::StepDeadlineExceeded(_) => step_failed(record, None).map(Some),
        CeremonyEvent::InterventionRequested(requested) => {
            intervention_requested(record, requested.intervention.target(), integrator)
        }
        CeremonyEvent::CeremonyCompleted(_)
        | CeremonyEvent::CeremonyCancelled(_)
        | CeremonyEvent::CeremonyDeadlineExceeded(_)
        | CeremonyEvent::StateDeadlineExceeded(_) => ceremony_ended(record).map(Some),
        _ => Ok(None),
    }
}

/// A step finished. Whether the work was accepted is a second
/// question, and the answer changes the kind rather than adding an
/// event: a host told twice about one step would act twice.
fn step_completed(record: &PositionedRecord, step: &StepId) -> Result<AttentionEvent, DomainError> {
    let rejected = rejected_review(&record.record);
    let (kind, reason) = if rejected {
        (
            AttentionKind::ReviewRejected,
            format!("step {step} completed without accepting the work"),
        )
    } else {
        (
            AttentionKind::ResultAvailable,
            format!("step {step} completed and its result is sealed"),
        )
    };
    Ok(build(record, kind, &reason, ResultAcceptance::Accepted)?.about_step(Some(step.clone())))
}

fn step_failed(
    record: &PositionedRecord,
    step: Option<&StepId>,
) -> Result<AttentionEvent, DomainError> {
    let reason = step.map_or_else(
        || "a step ran out of time".to_owned(),
        |step| format!("step {step} did not finish"),
    );
    Ok(build(
        record,
        AttentionKind::StepFailed,
        &reason,
        ResultAcceptance::NotApplicable,
    )?
    .about_step(step.cloned()))
}

fn intervention_requested(
    record: &PositionedRecord,
    target: &made_core::value_objects::CeremonyInterventionTarget,
    integrator: &RoleId,
) -> Result<Option<AttentionEvent>, DomainError> {
    // A question put to somebody else is not this integrator's to
    // answer. Waking it anyway would teach hosts to ignore the feed.
    if !target.accepts(integrator) {
        return Ok(None);
    }
    build(
        record,
        AttentionKind::InterventionRequested,
        "a participant was asked something this integrator can answer",
        ResultAcceptance::NotApplicable,
    )
    .map(Some)
}

fn ceremony_ended(record: &PositionedRecord) -> Result<AttentionEvent, DomainError> {
    build(
        record,
        AttentionKind::CeremonyEnded,
        "the ceremony reached an end",
        ResultAcceptance::NotApplicable,
    )
}

/// The documented convention: a reviewing step records whether it
/// accepted the work in its output.
fn rejected_review(record: &AuditRecord) -> bool {
    let Some(CeremonyEvent::StepCompleted(completed)) = record.event() else {
        return false;
    };
    completed
        .result
        .output()
        .attributes()
        .get(ACCEPTED_KEY)
        .and_then(serde_json::Value::as_bool)
        .is_some_and(|accepted| !accepted)
}

/// Everything an event needs that comes from the record itself.
fn build(
    record: &PositionedRecord,
    kind: AttentionKind,
    reason: &str,
    acceptance: ResultAcceptance,
) -> Result<AttentionEvent, DomainError> {
    let sealed = &record.record;
    let id = AttentionEventId::derive(sealed.ceremony_id(), sealed.event_id().as_str(), kind)?;
    let source = EventRef::new(
        sealed.event_id().clone(),
        sealed.sequence(),
        sealed.record_hash(),
    );
    Ok(AttentionEvent::new(
        id,
        kind,
        sealed.ceremony_id().clone(),
        source,
        record.position,
        sealed.occurred_at(),
        AttentionReason::new(reason)?,
        acceptance,
    )
    .caused_by(
        sealed.correlation_id().cloned(),
        sealed.causation_id().cloned(),
    )
    .with_evidence(Vec::<EvidenceReference>::new()))
}

#[cfg(test)]
#[path = "attention_rules_tests.rs"]
mod tests;
