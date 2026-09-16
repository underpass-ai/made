//! What a session did, as the events the journal seals.
//!
//! Each builder reads the session after the mutation and the arguments
//! the use case already holds, and returns the event with everything
//! the aggregate would need to redo that mutation. The id, the actor
//! and the envelope stay with [`super::session_facts`]; this module
//! only knows payloads.
//!
//! A builder that cannot find in the session what the fact claims —
//! the approval just recorded, the lease just taken — refuses rather
//! than inventing it: the fact is a receipt for a mutation, and a
//! receipt for something the session does not hold is the wrong kind
//! of record to seal.

use std::collections::BTreeMap;

use made_core::entities::ceremony_events::{
    CeremonyCompleted, CeremonyInstanceStarted, EvidenceCollected, HumanApprovalRecorded,
    HumanDeferralRecorded, InterventionClosed, InterventionRequested, InterventionResponded,
    ParticipantsBound, ReasonAsserted, StepCompleted, StepFailed, StepStarted, TransitionApplied,
};
use made_core::entities::{CeremonyEvent, CeremonyInstance, CeremonyIntervention};
use made_core::error::DomainError;
use made_core::value_objects::{
    CeremonyEvidenceSourceId, CeremonyInterventionId, CeremonyInterventionResponse, GuardName,
    RoleId, Specialty, StepAttempt, StepExecutionRecord, StepId, StepIteration, StepResult,
};
use time::OffsetDateTime;

/// The opening of a session, read off the freshly started instance.
pub(crate) fn ceremony_started(instance: &CeremonyInstance) -> CeremonyEvent {
    CeremonyEvent::CeremonyInstanceStarted(CeremonyInstanceStarted {
        ceremony_id: instance.id().clone(),
        definition_name: instance.definition_name().clone(),
        definition_version: instance.definition_version().clone(),
        initial_state: instance.current_state().clone(),
        step_ids: instance.step_records().keys().cloned().collect(),
        context: instance.context().clone(),
        bound_definition: instance.bound_definition(),
        created_at: instance.created_at(),
    })
}

/// The seating as the session recorded it, for the roles just seated.
pub(crate) fn participants_bound(
    instance: &CeremonyInstance,
    seating: &BTreeMap<RoleId, Specialty>,
) -> Result<CeremonyEvent, DomainError> {
    let bindings = seating
        .keys()
        .map(|role_id| {
            instance.participant_bindings().get(role_id).cloned().ok_or(
                DomainError::InvariantViolated {
                    reason: "a seating fact needs the binding the session recorded",
                },
            )
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok(CeremonyEvent::ParticipantsBound(ParticipantsBound {
        bindings,
    }))
}

/// A step taken, with the lease the session holds for it.
pub(crate) fn step_started(
    instance: &CeremonyInstance,
    step_id: &StepId,
    iteration: StepIteration,
    attempt: StepAttempt,
    started_by: &RoleId,
    occurred_at: OffsetDateTime,
) -> Result<CeremonyEvent, DomainError> {
    let lease = instance
        .step_record(step_id)
        .and_then(StepExecutionRecord::lease)
        .cloned()
        .ok_or(DomainError::InvariantViolated {
            reason: "a step start fact needs the lease the session recorded",
        })?;
    Ok(CeremonyEvent::StepStarted(StepStarted {
        step_id: step_id.clone(),
        iteration,
        attempt,
        lease,
        started_by: started_by.clone(),
        started_at: occurred_at,
    }))
}

/// A step ended, as a completion or a failure by its result.
///
/// Whether a successful result reopened the step at the next semantic
/// iteration was decided against the definition when it was applied;
/// it is read off the session here so the event carries it.
pub(crate) fn step_finished(
    instance: &CeremonyInstance,
    step_id: &StepId,
    iteration: StepIteration,
    attempt: StepAttempt,
    result: &StepResult,
    finished_by: &RoleId,
    occurred_at: OffsetDateTime,
) -> CeremonyEvent {
    if result.is_success() {
        let next_iteration = instance
            .step_record(step_id)
            .map(StepExecutionRecord::iteration)
            .filter(|current| *current != iteration);
        CeremonyEvent::StepCompleted(StepCompleted {
            step_id: step_id.clone(),
            iteration,
            attempt,
            result: result.clone(),
            next_iteration,
            finished_by: finished_by.clone(),
            finished_at: occurred_at,
        })
    } else {
        CeremonyEvent::StepFailed(StepFailed {
            step_id: step_id.clone(),
            iteration,
            attempt,
            result: result.clone(),
            finished_by: finished_by.clone(),
            finished_at: occurred_at,
        })
    }
}

/// The move the session just made.
pub(crate) fn transition_applied(
    instance: &CeremonyInstance,
) -> Result<CeremonyEvent, DomainError> {
    let transition =
        instance
            .transitions()
            .last()
            .cloned()
            .ok_or(DomainError::InvariantViolated {
                reason: "a transition fact needs the move the session recorded",
            })?;
    Ok(CeremonyEvent::TransitionApplied(TransitionApplied {
        transition,
    }))
}

/// The ending the session just reached.
pub(crate) fn ceremony_completed(
    instance: &CeremonyInstance,
) -> Result<CeremonyEvent, DomainError> {
    let completed_at = instance
        .completed_at()
        .ok_or(DomainError::InvariantViolated {
            reason: "a completion fact needs the ending the session recorded",
        })?;
    Ok(CeremonyEvent::CeremonyCompleted(CeremonyCompleted {
        final_state: instance.current_state().clone(),
        completed_at,
    }))
}

/// The intervention as it was opened.
pub(crate) fn intervention_requested(
    instance: &CeremonyInstance,
    intervention_id: &CeremonyInterventionId,
) -> Result<CeremonyEvent, DomainError> {
    Ok(CeremonyEvent::InterventionRequested(
        InterventionRequested {
            intervention: intervention(instance, intervention_id)?.clone(),
        },
    ))
}

/// The response a seat just made to an intervention.
///
/// A seat answers an item once, so the seat identifies the response.
pub(crate) fn intervention_responded(
    instance: &CeremonyInstance,
    intervention_id: &CeremonyInterventionId,
    responded_by: &RoleId,
) -> Result<CeremonyEvent, DomainError> {
    let response = intervention(instance, intervention_id)?
        .responses()
        .iter()
        .find(|response| response.role_id() == responded_by)
        .cloned()
        .ok_or(DomainError::InvariantViolated {
            reason: "a response fact needs the response the session recorded",
        })?;
    Ok(CeremonyEvent::InterventionResponded(
        InterventionResponded {
            intervention_id: intervention_id.clone(),
            response,
        },
    ))
}

/// The closing of an intervention.
pub(crate) fn intervention_closed(
    instance: &CeremonyInstance,
    intervention_id: &CeremonyInterventionId,
    closed_by: &RoleId,
) -> Result<CeremonyEvent, DomainError> {
    let closed_at = intervention(instance, intervention_id)?.closed_at().ok_or(
        DomainError::InvariantViolated {
            reason: "a closing fact needs the closing the session recorded",
        },
    )?;
    Ok(CeremonyEvent::InterventionClosed(InterventionClosed {
        intervention_id: intervention_id.clone(),
        closed_by: closed_by.clone(),
        closed_at,
    }))
}

/// The evidence a seat took in from a source to answer an item, read
/// off the response the session recorded for that seat.
pub(crate) fn evidence_collected(
    instance: &CeremonyInstance,
    intervention_id: &CeremonyInterventionId,
    source_id: &CeremonyEvidenceSourceId,
    collected_by: &RoleId,
    occurred_at: OffsetDateTime,
) -> Result<CeremonyEvent, DomainError> {
    let evidence_pack = intervention(instance, intervention_id)?
        .responses()
        .iter()
        .find(|response| response.role_id() == collected_by)
        .and_then(CeremonyInterventionResponse::evidence_pack)
        .cloned()
        .ok_or(DomainError::InvariantViolated {
            reason: "an evidence fact needs the pack the session recorded",
        })?;
    Ok(CeremonyEvent::EvidenceCollected(EvidenceCollected {
        intervention_id: intervention_id.clone(),
        source_id: source_id.clone(),
        collected_by: collected_by.clone(),
        evidence_pack,
        collected_at: occurred_at,
    }))
}

/// The reason a seat just asserted — the last one the session holds.
pub(crate) fn reason_asserted(instance: &CeremonyInstance) -> Result<CeremonyEvent, DomainError> {
    let reason = instance
        .reasons()
        .last()
        .cloned()
        .ok_or(DomainError::InvariantViolated {
            reason: "a reason fact needs the reason the session recorded",
        })?;
    Ok(CeremonyEvent::ReasonAsserted(ReasonAsserted { reason }))
}

/// The approval just recorded for a guard.
pub(crate) fn guard_approved(
    instance: &CeremonyInstance,
    guard_name: &GuardName,
) -> Result<CeremonyEvent, DomainError> {
    let approval = instance
        .guard_approvals()
        .iter()
        .rev()
        .find(|approval| approval.guard_name() == guard_name)
        .cloned()
        .ok_or(DomainError::InvariantViolated {
            reason: "an approval fact needs the approval the session recorded",
        })?;
    Ok(CeremonyEvent::HumanApprovalRecorded(
        HumanApprovalRecorded { approval },
    ))
}

/// The deferral just recorded for a guard.
pub(crate) fn guard_deferred(
    instance: &CeremonyInstance,
    guard_name: &GuardName,
) -> Result<CeremonyEvent, DomainError> {
    let deferral = instance
        .guard_deferrals()
        .iter()
        .rev()
        .find(|deferral| deferral.guard_name() == guard_name)
        .cloned()
        .ok_or(DomainError::InvariantViolated {
            reason: "a deferral fact needs the deferral the session recorded",
        })?;
    Ok(CeremonyEvent::HumanDeferralRecorded(
        HumanDeferralRecorded { deferral },
    ))
}

fn intervention<'a>(
    instance: &'a CeremonyInstance,
    intervention_id: &CeremonyInterventionId,
) -> Result<&'a CeremonyIntervention, DomainError> {
    instance
        .intervention(intervention_id)
        .ok_or(DomainError::InvariantViolated {
            reason: "an intervention fact needs the item the session recorded",
        })
}
