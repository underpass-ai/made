//! What a session did, in the journal's terms.
//!
//! One place turns an event a decision produced into a fact that can
//! be sealed, so the shape of a fact is decided once instead of at
//! every call site.
//!
//! # The event id is derived, not generated
//!
//! A retried commit must produce the same fact rather than a second
//! one wearing a new name. Deriving the id from what the fact is about
//! — the session, what happened, and which thing it happened to —
//! makes a retry idempotent by construction, where a fresh identifier
//! each time would turn one approval into two entries in a chain that
//! is supposed to be the record of what happened.
//!
//! What a fact is about is read off the event itself, and for the two
//! kinds numbered by position, off the session the event was decided
//! against. Legacy payloads retain their former derivation; new visit-bearing
//! payloads add the visit coordinate so re-entering a state creates new facts
//! without changing the identity of a historical event.

use made_core::entities::{AuditFact, CeremonyEvent, CeremonyInstance};
use made_core::error::DomainError;
use made_core::value_objects::{
    AuditActor, AuditActorKind, AuditEventType, CeremonyId, EventId, RoleId,
};
use time::OffsetDateTime;

/// The id of the record that opens a stream.
///
/// Derived rather than read because every fact of a stream names it
/// as its correlation, and the opening is the one fact whose identity
/// follows from the ceremony alone.
pub(crate) fn opening_event_id(ceremony_id: &CeremonyId) -> Result<EventId, DomainError> {
    event_id(
        ceremony_id,
        AuditEventType::CeremonyInstanceStarted,
        "session",
    )
}

/// Who did it, as a seat at this table.
///
/// The kind is carried through from what the caller declared. This
/// engine sees a seat and cannot see what fills it, so the one thing
/// it must not do here is decide.
pub(crate) fn seat(role_id: &RoleId, kind: AuditActorKind) -> Result<AuditActor, DomainError> {
    AuditActor::new(role_id.as_str(), kind, Some(role_id.clone()))
}

/// The seat sealed by a successful step-ending decision.
///
/// Derived inside optimistic retries so the envelope actor and the event's
/// `finished_by` always describe the same accepted attempt.
pub(crate) fn step_result_seat(
    events: &[CeremonyEvent],
    kind: AuditActorKind,
) -> Result<AuditActor, DomainError> {
    let role = events.iter().find_map(|event| match event {
        CeremonyEvent::StepCompleted(completed) => Some(&completed.finished_by),
        CeremonyEvent::StepFailed(failed) => Some(&failed.finished_by),
        CeremonyEvent::LateStepResultObserved(observed) => Some(observed.result.finished_by()),
        _ => None,
    });
    seat(
        role.ok_or(DomainError::InvariantViolated {
            reason: "a step result decision emits a completed or failed event",
        })?,
        kind,
    )
}

/// The seat sealed by the step-start decision that won an optimistic append.
///
/// Automatic role binding is resolved by the aggregate against the session
/// seen by each retry. Reading the accepted event here keeps the audit actor
/// aligned with that decision even when context changed after the first read.
pub(crate) fn step_started_seat(
    events: &[CeremonyEvent],
    kind: AuditActorKind,
) -> Result<AuditActor, DomainError> {
    let role = events.iter().find_map(|event| match event {
        CeremonyEvent::StepStarted(started) => Some(&started.started_by),
        _ => None,
    });
    seat(
        role.ok_or(DomainError::InvariantViolated {
            reason: "a step start decision emits a started event",
        })?,
        kind,
    )
}

/// Who did it, holding no seat.
///
/// Whoever opens a session or seats its table may be a participant,
/// an operator, or a scheduler that will never take part. The caller
/// declares an identity of their own choosing and what kind of party
/// it is, and the fact records both without pretending either is a
/// role this ceremony declared.
pub(crate) fn party(actor_id: &str, kind: AuditActorKind) -> Result<AuditActor, DomainError> {
    AuditActor::new(actor_id, kind, None)
}

/// The facts of one decision, in the order it produced them.
///
/// `instance` is the session the events were decided against, before
/// any of them is folded. The two facts numbered by position — a move
/// and a reason — take their ordinal from it.
pub(crate) fn facts(
    instance: &CeremonyInstance,
    events: Vec<CeremonyEvent>,
    actor: &AuditActor,
    occurred_at: OffsetDateTime,
) -> Result<Vec<AuditFact>, DomainError> {
    events
        .into_iter()
        .map(|event| fact(instance, event, actor.clone(), occurred_at))
        .collect()
}

/// The envelope around an event: its derived id, who did it and when.
///
/// Correlation and causation are left empty on purpose; the stream
/// fills them at commit, because only it knows the head.
pub(crate) fn fact(
    instance: &CeremonyInstance,
    event: CeremonyEvent,
    actor: AuditActor,
    occurred_at: OffsetDateTime,
) -> Result<AuditFact, DomainError> {
    Ok(AuditFact {
        event_id: event_id(instance.id(), event.event_type(), &about(instance, &event))?,
        event,
        ceremony_id: instance.id().clone(),
        definition_name: instance.definition_name().clone(),
        definition_version: instance.definition_version().clone(),
        occurred_at,
        actor,
        correlation_id: None,
        causation_id: None,
        trace: None,
    })
}

fn event_id(
    ceremony_id: &CeremonyId,
    event_type: AuditEventType,
    about: &str,
) -> Result<EventId, DomainError> {
    EventId::new(format!(
        "{}:{}:{about}",
        ceremony_id.as_str(),
        event_type.as_str()
    ))
}

/// Which thing the event happened to, as the id names it.
///
/// Each rule is the one the journal always used for that fact:
///
/// - A seating is identified by the seating itself — which roles, to
///   which specialties. A role can be seated more than once, and the
///   session keeps only the current seating, so there is no position
///   to number; re-seating a role to the specialty it already had
///   derives the same id, and seating it elsewhere a different one.
/// - A step start and ending are keyed on the semantic iteration and
///   the technical attempt as well as the step. A retry and a
///   successful repeat are both distinct starts and endings; a scheme
///   that only knew the step and attempt would fold iteration two,
///   attempt one into iteration one, attempt one.
/// - A move is numbered by how many moves the session had made, so
///   successive transitions are distinct facts while a retry of the
///   same one — decided again against the same session — derives the
///   same id. The completion a move into a terminal state produces
///   shares its ordinal: they are two facts about one move.
/// - A response is keyed on the seat as well as the item, because an
///   item put to the whole table is answered by more than one of them;
///   a closing on the item alone, because an item is closed once; the
///   evidence behind an answer on the source as well as the item, since
///   an item answered out of two sources was looked into twice.
/// - A reason is numbered by how many the session held, not keyed on
///   the edge: two seats can reach the same conclusion, and one seat
///   can say it again with a different why.
/// - A guard decision is keyed on the guard.
/// - An import is identified by the session, like the opening it
///   stands in for: a session is imported once, and a second import
///   of the same one derives the same id and is refused by the store.
/// - A recollection is keyed on nothing else, because a session opens
///   once and reads its memory once: deciding the opening again derives
///   the same two ids, which is what makes a retried start land once.
fn about(instance: &CeremonyInstance, event: &CeremonyEvent) -> String {
    match event {
        CeremonyEvent::HostHandoffRecorded(recorded) => {
            format!("host-handoff:{}", recorded.declaration.id.as_str())
        }
        CeremonyEvent::CeremonyInstanceStarted(_) | CeremonyEvent::InstanceImported(_) => {
            "session".to_owned()
        }
        CeremonyEvent::MemoryRecalled(_) => "recollection".to_owned(),
        CeremonyEvent::ParticipantsBound(bound) => {
            let seated = bound
                .bindings
                .iter()
                .map(|binding| format!("{}={}", binding.role_id(), binding.specialty()))
                .collect::<Vec<_>>()
                .join(",");
            format!("seating:{seated}")
        }
        CeremonyEvent::StepStarted(started) => step_about(
            &started.step_id,
            started.state_visit,
            started.state_iteration().get(),
            started.iteration.get(),
            started.attempt.get(),
        ),
        CeremonyEvent::StepLeaseRenewed(renewed) if renewed.request.is_some() => format!(
            "renewal:{}",
            renewed
                .request
                .as_ref()
                .expect("checked request")
                .id
                .as_str()
        ),
        CeremonyEvent::StepLeaseRenewed(renewed) => format!(
            "step:{}:claim:{}:expiry:{}",
            renewed.step_id,
            renewed.claim_fence.as_str(),
            renewed.expires_at.unix_timestamp_nanos()
        ),
        CeremonyEvent::StepCompleted(completed) => step_about(
            &completed.step_id,
            completed.state_visit,
            completed.state_iteration().get(),
            completed.iteration.get(),
            completed.attempt.get(),
        ),
        CeremonyEvent::StepFailed(failed) => step_about(
            &failed.step_id,
            failed.state_visit,
            failed.state_iteration().get(),
            failed.iteration.get(),
            failed.attempt.get(),
        ),
        CeremonyEvent::ContextWritten(written) => step_about(
            &written.step_id,
            written.state_visit,
            written.state_iteration.get(),
            written.iteration.get(),
            written.attempt.get(),
        ),
        CeremonyEvent::StateIterationStarted(started) => {
            let visit = started
                .state_visit
                .map(|visit| format!(":visit:{}", visit.get()))
                .unwrap_or_default();
            format!(
                "state:{}{visit}:iteration:{}",
                started.state_id,
                started.state_iteration.get()
            )
        }
        CeremonyEvent::TransitionApplied(_) | CeremonyEvent::CeremonyCompleted(_) => {
            format!("transition:{}", instance.transitions().len() + 1)
        }
        event @ (CeremonyEvent::InterventionRequested(_)
        | CeremonyEvent::InterventionResponded(_)
        | CeremonyEvent::InterventionClosed(_)
        | CeremonyEvent::InterventionDeliveryAcknowledged(_)
        | CeremonyEvent::EvidenceCollected(_)) => intervention_about(event),
        CeremonyEvent::ReasonAsserted(_) => format!("reason:{}", instance.reasons().len() + 1),
        CeremonyEvent::HumanApprovalRecorded(recorded) => {
            format!("guard:{}", recorded.approval.guard_name())
        }
        CeremonyEvent::HumanDeferralRecorded(recorded) => {
            format!("guard:{}", recorded.deferral.guard_name())
        }
        event @ (CeremonyEvent::ChildSpawnPlanned(_)
        | CeremonyEvent::ChildSpawnPlanAdopted(_)
        | CeremonyEvent::ChildCompletionAccepted(_)) => child_about(event),
        CeremonyEvent::CeremonyPaused(_)
        | CeremonyEvent::CeremonyResumed(_)
        | CeremonyEvent::CeremonyCancelled(_)
        | CeremonyEvent::CeremonyDeadlineExceeded(_)
        | CeremonyEvent::StateDeadlineExceeded(_)
        | CeremonyEvent::StepDeadlineExceeded(_)
        | CeremonyEvent::LateStepResultObserved(_) => lifecycle_about(event),
        CeremonyEvent::ExecutionReceiptLinked(linked) => execution_receipt_about(&linked.link),
    }
}

fn execution_receipt_about(link: &made_core::value_objects::ExecutionReceiptLink) -> String {
    match link.kind() {
        made_core::value_objects::ExecutionReceiptLinkKind::Direct => {
            format!("execution_receipt:{}", link.receipt_id())
        }
        made_core::value_objects::ExecutionReceiptLinkKind::Adopted => format!(
            "execution_receipt_adoption:{}:{}",
            link.receipt_id(),
            link.applied_claim_fence().as_str()
        ),
    }
}

/// Sixteen hex characters of SHA-256: enough that two destinations do
/// not collide, short enough that an identifier built from it stays
/// inside the bounds an event id has.
fn short_digest(value: &str) -> String {
    use sha2::{Digest, Sha256};
    let mut digest = Sha256::new();
    digest.update(value.as_bytes());
    format!("{:x}", digest.finalize())[..16].to_owned()
}

fn intervention_about(event: &CeremonyEvent) -> String {
    match event {
        CeremonyEvent::InterventionRequested(requested) => {
            format!("intervention:{}", requested.intervention.id())
        }
        CeremonyEvent::InterventionResponded(responded) => format!(
            "intervention:{}:{}",
            responded.intervention_id,
            responded.response.role_id()
        ),
        CeremonyEvent::InterventionClosed(closed) => {
            format!("intervention:{}", closed.intervention_id)
        }
        // The delivery identity, digested rather than spelled out.
        //
        // It already contains the ceremony, the item and the whole
        // destination including the process generation, so it
        // distinguishes every legitimate repeat — and it is built from
        // three separately bounded strings, so spelled out it can
        // exceed what an event id may be. The item is kept readable
        // because that is what somebody scanning the stream is looking
        // for; which offer it was is in the payload.
        CeremonyEvent::InterventionDeliveryAcknowledged(acknowledged) => format!(
            "intervention:{}:delivery:{}",
            acknowledged.intervention_id,
            short_digest(acknowledged.ack.delivery_id().as_str())
        ),
        CeremonyEvent::EvidenceCollected(collected) => format!(
            "intervention:{}:source:{}",
            collected.intervention_id, collected.source_id
        ),
        _ => unreachable!("caller filters intervention events"),
    }
}

fn child_about(event: &CeremonyEvent) -> String {
    match event {
        CeremonyEvent::ChildSpawnPlanned(planned) => {
            format!("child_group:{}", planned.plan.group_id())
        }
        CeremonyEvent::ChildSpawnPlanAdopted(adopted) => format!(
            "child_group:{}:fence:{}",
            adopted.group_id,
            adopted.claim_fence.as_str()
        ),
        CeremonyEvent::ChildCompletionAccepted(accepted) => format!(
            "child_group:{}:child:{}",
            accepted.completion.group_id(),
            accepted.completion.child_id()
        ),
        _ => unreachable!("only child events are delegated here"),
    }
}

fn lifecycle_about(event: &CeremonyEvent) -> String {
    match event {
        CeremonyEvent::CeremonyPaused(paused) => {
            format!("pause:{}", paused.paused_at.unix_timestamp_nanos())
        }
        CeremonyEvent::CeremonyResumed(resumed) => {
            format!("resume:{}", resumed.resumed_at.unix_timestamp_nanos())
        }
        CeremonyEvent::CeremonyCancelled(_) => "cancel".to_owned(),
        CeremonyEvent::CeremonyDeadlineExceeded(_) => "ceremony_deadline".to_owned(),
        CeremonyEvent::StateDeadlineExceeded(exceeded) => format!(
            "state:{}:visit:{}",
            exceeded.deadline.state_id(),
            exceeded.deadline.state_visit().get()
        ),
        CeremonyEvent::StepDeadlineExceeded(exceeded) => {
            format!("step_deadline:{}", exceeded.deadline.claim_fence().as_str())
        }
        CeremonyEvent::LateStepResultObserved(observed) => format!(
            "late_step_result:{}",
            observed.result.claim_fence().as_str()
        ),
        _ => unreachable!("only lifecycle events are delegated here"),
    }
}

fn step_about(
    step_id: &made_core::value_objects::StepId,
    state_visit: Option<made_core::value_objects::StateVisit>,
    state_iteration: u32,
    iteration: u32,
    attempt: u32,
) -> String {
    let visit = state_visit
        .map(|visit| format!(":visit:{}", visit.get()))
        .unwrap_or_default();
    format!("step:{step_id}{visit}:state_iteration:{state_iteration}:iteration:{iteration}:attempt:{attempt}")
}

#[cfg(test)]
#[path = "session_facts_tests.rs"]
mod tests;
