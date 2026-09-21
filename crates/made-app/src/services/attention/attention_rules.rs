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
//! A human guard has two halves and only one of them is here. That a
//! person has *answered* one is a record, so it is decided here, and it
//! has to be: the loop stops in front of a human guard, and nothing
//! else in the journal would ever start it again. That a session has
//! newly *arrived* in front of an unanswered one is the projector's,
//! because whether the state it entered is guarded is read from the
//! definition and telling one visit from the next — once per guard and
//! visit, says ADR 022 — needs a history these rules cannot see.
//!
//! Host-reported sources — a participant declaring itself finished, a
//! participant declaring itself blocked, silence for longer than the
//! policy allows — are not records at all, so they are not decided
//! here either.

use made_core::entities::{AuditRecord, CeremonyEvent};
use made_core::error::DomainError;
use made_core::ports::PositionedRecord;
use made_core::value_objects::{
    AttentionEventId, AttentionKind, AttentionReason, GuardName, RoleId, StepId,
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
        CeremonyEvent::StepFailed(failed) => {
            step_failed(record, &failed.step_id, "did not finish").map(Some)
        }
        CeremonyEvent::StepDeadlineExceeded(exceeded) => {
            step_failed(record, exceeded.deadline.step_id(), "ran out of time").map(Some)
        }
        CeremonyEvent::HumanApprovalRecorded(recorded) => {
            human_guard_answered(record, recorded.approval.guard_name(), "approved").map(Some)
        }
        CeremonyEvent::HumanDeferralRecorded(recorded) => {
            human_guard_answered(record, recorded.deferral.guard_name(), "deferred").map(Some)
        }
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

/// A step is not going to produce a result, and which step it was is
/// the first thing the integrator needs. A deadline carries it inside
/// the deadline rather than beside it, so reading only the shape of
/// the event would wake a host that cannot tell what to retry.
fn step_failed(
    record: &PositionedRecord,
    step: &StepId,
    what_happened: &str,
) -> Result<AttentionEvent, DomainError> {
    Ok(build(
        record,
        AttentionKind::StepFailed,
        &format!("step {step} {what_happened}"),
        ResultAcceptance::NotApplicable,
    )?
    .about_step(Some(step.clone())))
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

/// A person decided a guard the loop was not allowed to decide.
///
/// Both answers are news, and the deferral more than the approval: an
/// integrator that was only told about approvals would sit in front of
/// a guard somebody had deliberately left open, waiting for an answer
/// that has already been given.
fn human_guard_answered(
    record: &PositionedRecord,
    guard: &GuardName,
    what_happened: &str,
) -> Result<AttentionEvent, DomainError> {
    build(
        record,
        AttentionKind::HumanDecisionRequested,
        &format!("a person {what_happened} the human guard {guard}"),
        ResultAcceptance::NotApplicable,
    )
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

/// The news that a full queue has cost this integrator something.
///
/// Derived from the record that arrived when the queue overflowed
/// rather than from the one that was dropped: what a host needs to
/// learn is that it is behind, and what was shed is already in the
/// ledger with the cause on it. Blocking, so the signal that things
/// are being lost is never itself the thing lost.
pub fn queue_overflow(record: &PositionedRecord) -> Result<AttentionEvent, DomainError> {
    build(
        record,
        AttentionKind::Blocked,
        "this integrator's queue is full; older results were dropped",
        ResultAcceptance::NotApplicable,
    )
}

/// Everything an event needs that comes from the record itself.
///
/// Evidence is not among it. What a host should go and read is a
/// question about the ceremony around the record rather than about
/// the record, so the projector attaches it.
fn build(
    record: &PositionedRecord,
    kind: AttentionKind,
    reason: &str,
    acceptance: ResultAcceptance,
) -> Result<AttentionEvent, DomainError> {
    let sealed = &record.record;
    let id = AttentionEventId::derive(
        sealed.ceremony_id(),
        record.position,
        sealed.event_id().as_str(),
        kind,
    )?;
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
    ))
}

#[cfg(test)]
#[path = "attention_rules_tests.rs"]
mod tests;
