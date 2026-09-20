use made_core::entities::{AuditChain, AuditRecord, CeremonyEvent, CeremonyInstance};
use made_core::error::DomainError;
use made_core::value_objects::{
    AuditRecordHash, CarriedEvidence, IdempotencyKey, StepId, StreamVersion, SuccessionPlan,
};

use crate::services::succession_evidence;

/// The handoff already sealed under this plan id, if there is one.
pub(super) fn sealed_plan<'a>(
    instance: &'a CeremonyInstance,
    plan_id: &IdempotencyKey,
) -> Option<&'a SuccessionPlan> {
    instance
        .successor_plan()
        .filter(|sealed| sealed.plan_id() == plan_id)
}

/// The evidence the named steps would be carried with, resolved
/// against the predecessor's own journal.
///
/// A step the caller names that this ceremony did not complete is a
/// mistake worth saying out loud: silently dropping it would open a
/// successor missing work its caller believed it had.
pub(super) fn carried_evidence(
    instance: &CeremonyInstance,
    records: &[AuditRecord],
    carried: &[StepId],
) -> Result<Vec<CarriedEvidence>, DomainError> {
    let mut evidence = Vec::with_capacity(carried.len());
    for step_id in carried {
        let source = succession_evidence::completed_source(instance, records, step_id)?
            .ok_or_else(|| DomainError::InvalidDocument {
                reason: format!(
                    "step `{}` cannot be carried: this ceremony has not completed it",
                    step_id.as_str()
                ),
            })?;
        let output = instance
            .step_record(step_id)
            .ok_or(DomainError::NotFound {
                what: "ceremony_step_record",
            })?
            .output()
            .clone();
        evidence.push(CarriedEvidence::new(step_id.clone(), source, output));
    }
    Ok(evidence)
}

/// The record that sealed this plan, as a cut of the predecessor.
pub(super) fn predecessor_cut(
    records: &[AuditRecord],
    plan_id: &IdempotencyKey,
) -> Result<(AuditRecordHash, StreamVersion), DomainError> {
    records
        .iter()
        .find(|record| seals(record, plan_id))
        .map(|record| {
            (
                record.record_hash(),
                StreamVersion::from_sequence(record.sequence()),
            )
        })
        .ok_or(DomainError::InvariantViolated {
            reason: "the sealed handoff is not in the predecessor's journal",
        })
}

fn seals(record: &AuditRecord, plan_id: &IdempotencyKey) -> bool {
    matches!(
        record.event(),
        Some(CeremonyEvent::SuccessorPlanned(planned)) if planned.plan.plan_id() == plan_id
    )
}

/// Whether the stream already there is the opening this plan makes.
///
/// Compared event by event against an intact chain. A successor's id
/// is derived, so a stream holding that id and different content is
/// not a half-finished retry: it is somebody else's ceremony, and
/// appending to it would be worse than refusing.
pub(super) fn verify_existing_opening(
    records: &[AuditRecord],
    expected: &[CeremonyEvent],
) -> Result<(), DomainError> {
    if !AuditChain::verify(records).is_intact() {
        return Err(DomainError::InvariantViolated {
            reason: "the successor's journal is not intact",
        });
    }
    if records.len() < expected.len()
        || records
            .iter()
            .zip(expected)
            .any(|(record, event)| record.event() != Some(event))
    {
        return Err(DomainError::AlreadyExists {
            what: "foreign_successor_ceremony",
        });
    }
    Ok(())
}
