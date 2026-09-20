//! Addressing the predecessor's sealed work from its own stream.
//!
//! Planning a handoff and carrying it out both have to say exactly
//! which record a piece of evidence came from, and both have to say it
//! the same way or a retry would seal a different plan. One place
//! answers it, so the two cannot drift.

use made_core::entities::{AuditRecord, CeremonyEvent, CeremonyInstance};
use made_core::error::DomainError;
use made_core::value_objects::{SourceRecordRef, StepExecutionRecord, StepId, StepStatus};

/// The record this ceremony sealed for one completed step, addressed
/// so another instance can point at it.
///
/// The step's current execution record says which visit and attempt
/// count; the journal says which sealed record that was. Matching on
/// both is what keeps a repeated step from being addressed by its
/// first pass — the reference has to name the work that is actually
/// standing, not the first time the step ran.
pub(crate) fn completed_source(
    instance: &CeremonyInstance,
    records: &[AuditRecord],
    step_id: &StepId,
) -> Result<Option<SourceRecordRef>, DomainError> {
    let Some(record) = instance.step_record(step_id) else {
        return Ok(None);
    };
    if record.status() != StepStatus::Completed {
        return Ok(None);
    }
    let sealed = records
        .iter()
        .rev()
        .find(|audit| completes(audit, step_id, record));
    let Some(sealed) = sealed else {
        return Err(DomainError::InvalidDocument {
            reason: format!(
                "step `{}` is completed but its completion is not in the journal",
                step_id.as_str()
            ),
        });
    };
    Ok(Some(SourceRecordRef::new(
        instance.id().clone(),
        step_id.clone(),
        sealed.event_id().clone(),
        sealed.record_hash(),
        record.state_visit(),
        record.attempt(),
    )))
}

fn completes(audit: &AuditRecord, step_id: &StepId, record: &StepExecutionRecord) -> bool {
    matches!(
        audit.event(),
        Some(CeremonyEvent::StepCompleted(completed))
            if &completed.step_id == step_id
                && completed.state_visit() == record.state_visit()
                && completed.attempt == record.attempt()
                && completed.iteration == record.iteration()
    )
}
