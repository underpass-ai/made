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
//! against. Every derivation below is the one the journal used before
//! sessions were folded from their streams, so the same fact keeps the
//! same id.

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
fn about(instance: &CeremonyInstance, event: &CeremonyEvent) -> String {
    match event {
        CeremonyEvent::CeremonyInstanceStarted(_) | CeremonyEvent::InstanceImported(_) => {
            "session".to_owned()
        }
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
            started.iteration.get(),
            started.attempt.get(),
        ),
        CeremonyEvent::StepCompleted(completed) => step_about(
            &completed.step_id,
            completed.iteration.get(),
            completed.attempt.get(),
        ),
        CeremonyEvent::StepFailed(failed) => step_about(
            &failed.step_id,
            failed.iteration.get(),
            failed.attempt.get(),
        ),
        CeremonyEvent::TransitionApplied(_) | CeremonyEvent::CeremonyCompleted(_) => {
            format!("transition:{}", instance.transitions().len() + 1)
        }
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
        CeremonyEvent::EvidenceCollected(collected) => format!(
            "intervention:{}:source:{}",
            collected.intervention_id, collected.source_id
        ),
        CeremonyEvent::ReasonAsserted(_) => format!("reason:{}", instance.reasons().len() + 1),
        CeremonyEvent::HumanApprovalRecorded(recorded) => {
            format!("guard:{}", recorded.approval.guard_name())
        }
        CeremonyEvent::HumanDeferralRecorded(recorded) => {
            format!("guard:{}", recorded.deferral.guard_name())
        }
    }
}

fn step_about(step_id: &made_core::value_objects::StepId, iteration: u32, attempt: u32) -> String {
    format!("step:{step_id}:iteration:{iteration}:attempt:{attempt}")
}
