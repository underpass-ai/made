use std::collections::BTreeMap;

use made_core::entities::{AuditRecord, ProtocolInstance};
use made_core::error::DomainError;
use made_core::value_objects::{
    Attributes, ChildGroupId, ChildSpawnPlan, ContextKey, InputName, ProtocolContext, ProtocolId,
    StepClaimFence, StepId, StepOutput, StepResult, StepStatus,
};

pub(super) fn same_spawn_request_is_sealed(
    instance: &ProtocolInstance,
    candidate: &ChildSpawnPlan,
    step_id: &StepId,
    claim_fence: &StepClaimFence,
) -> Result<bool, DomainError> {
    let coordinates = candidate.coordinates();
    if coordinates.step_id() != step_id
        || candidate.active_claim_fence() != claim_fence
        || candidate.group_id() != &ChildGroupId::derive(instance.id(), coordinates)
    {
        return Err(DomainError::InvariantViolated {
            reason: "child spawn request identity, coordinates, or fence are inconsistent",
        });
    }
    let Some(group) = instance.child_group(candidate.group_id()) else {
        return Ok(false);
    };
    let sealed = group.plan();
    if sealed.group_id() != candidate.group_id()
        || sealed.coordinates() != coordinates
        || sealed.active_claim_fence() != claim_fence
    {
        return Err(DomainError::InvariantViolated {
            reason: "existing child spawn plan belongs to another request",
        });
    }
    Ok(true)
}

pub(super) fn project_context(
    parent: &ProtocolContext,
    inputs: &BTreeMap<InputName, ContextKey>,
) -> Result<ProtocolContext, DomainError> {
    let mut projected = BTreeMap::new();
    for (input, source) in inputs {
        let value = parent
            .get(source)
            .cloned()
            .ok_or(DomainError::InvalidDocument {
                reason: format!("missing parent context input: {}", source.as_str()),
            })?;
        projected.insert(input.as_str().to_owned(), value);
    }
    Ok(ProtocolContext::new(Attributes::new(projected)?))
}

pub(super) fn spawn_result(
    group_id: &ChildGroupId,
    child_ids: &[ProtocolId],
) -> Result<StepResult, DomainError> {
    let output = Attributes::new(BTreeMap::from([
        (
            "child_group_id".to_owned(),
            serde_json::json!(group_id.as_str()),
        ),
        (
            "child_ids".to_owned(),
            serde_json::json!(child_ids.iter().map(ProtocolId::as_str).collect::<Vec<_>>()),
        ),
    ]))?;
    StepResult::completed(StepOutput::new(output))
}

pub(super) fn completed_spawn_is_same(
    instance: &ProtocolInstance,
    plan: &ChildSpawnPlan,
    step_id: &StepId,
    result: &StepResult,
) -> Result<bool, DomainError> {
    let coordinates = plan.coordinates();
    if coordinates.step_id() != step_id
        || plan.group_id() != &ChildGroupId::derive(instance.id(), coordinates)
    {
        return Err(DomainError::InvariantViolated {
            reason: "child spawn plan identity differs from its parent coordinates",
        });
    }
    let group = instance
        .child_group(plan.group_id())
        .ok_or(DomainError::NotFound {
            what: "child_spawn_group",
        })?;
    if group.plan() != plan {
        return Err(DomainError::InvariantViolated {
            reason: "child spawn group differs from its sealed plan",
        });
    }
    let record = instance
        .step_record(step_id)
        .into_iter()
        .chain(instance.step_record_history(step_id))
        .find(|record| {
            record.state_visit() == coordinates.state_visit()
                && record.state_iteration() == coordinates.state_iteration()
                && record.iteration() == coordinates.step_iteration()
        })
        .ok_or(DomainError::NotFound {
            what: "protocol_step_record",
        })?;
    if record.status() != StepStatus::Completed {
        return Ok(false);
    }
    if record.output() != result.output() {
        return Err(DomainError::InvariantViolated {
            reason: "completed child spawn output differs from its sealed plan",
        });
    }
    Ok(true)
}

pub(super) fn verify_chain(records: &[AuditRecord]) -> Result<(), DomainError> {
    if !made_core::entities::AuditChain::verify(records).is_intact() {
        return Err(DomainError::InvariantViolated {
            reason: "child protocol journal is not intact",
        });
    }
    Ok(())
}
