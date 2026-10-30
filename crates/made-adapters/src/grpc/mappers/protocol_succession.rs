//! Succession over the wire, in both directions.
//!
//! The plan view and the sealed plan are rendered here from the same
//! domain values the embedded surface renders, so a caller gets one
//! answer whichever transport it asked over.

use made_app::usecases::{
    PlanProtocolSuccessorInput, ProtocolSuccessorPlanView, StartProtocolSuccessorInput,
};
use made_core::value_objects::{
    AuditActorId, BudgetDisposition, CarriedEvidence, ClaimDisposition, ClaimDispositionKind,
    DefinitionPin, EvidenceReference, ExecutionReceiptId, IdempotencyKey, ProtocolContext,
    ProtocolId, ProtocolName, ProtocolSuccession, ProtocolVersion, SourceRecordRef, StepClaimFence,
    StepId, SuccessionPlan,
};
use made_core::DomainError;
use made_proto::v1 as pb;

use super::actor_kind::actor_kind_from_proto;
use super::attributes::{attributes_from_struct, attributes_to_struct};
use super::protocol_authoring::diff_protocol_definitions_response_from;
use super::protocol_instance::moment;
use super::protocol_preflight::preflight_to_proto;

pub(crate) fn plan_input(
    request: pb::PlanProtocolSuccessorRequest,
) -> Result<PlanProtocolSuccessorInput, DomainError> {
    Ok(PlanProtocolSuccessorInput {
        instance_id: ProtocolId::new(request.protocol_id)?,
        definition_name: ProtocolName::new(request.definition_name)?,
        definition_version: ProtocolVersion::new(request.definition_version)?,
    })
}

pub(crate) fn start_input(
    request: pb::StartProtocolSuccessorRequest,
) -> Result<StartProtocolSuccessorInput, DomainError> {
    let context_overrides = request
        .context_overrides
        .map(|context| attributes_from_struct(Some(context)).map(ProtocolContext::new))
        .transpose()?;
    Ok(StartProtocolSuccessorInput {
        instance_id: ProtocolId::new(request.protocol_id)?,
        plan_id: IdempotencyKey::new(request.plan_id)?,
        definition_name: ProtocolName::new(request.definition_name)?,
        definition_version: ProtocolVersion::new(request.definition_version)?,
        carried: request
            .carried
            .into_iter()
            .map(StepId::new)
            .collect::<Result<Vec<_>, _>>()?,
        dispositions: request
            .dispositions
            .into_iter()
            .map(disposition_from)
            .collect::<Result<Vec<_>, _>>()?,
        budget: budget_from(&request.budget)?,
        context_overrides,
        actor_id: AuditActorId::new(request.actor_id),
        actor_kind: actor_kind_from_proto(&request.actor_kind, "successor.actor_kind")?,
    })
}

/// Only `fresh` and the empty default are honoured. `transfer_remaining`
/// is spelled out so a caller who asks for it is told why it is refused
/// rather than quietly given something else.
fn budget_from(raw: &str) -> Result<BudgetDisposition, DomainError> {
    match raw {
        "" | "fresh" => Ok(BudgetDisposition::Fresh),
        "transfer_remaining" => Ok(BudgetDisposition::TransferRemaining),
        _ => Err(DomainError::InvalidCharacters {
            field: "successor.budget",
        }),
    }
}

fn disposition_from(
    disposition: pb::ProtocolClaimDisposition,
) -> Result<ClaimDisposition, DomainError> {
    let kind = match disposition.kind.as_str() {
        "abandon_no_external_effect" => ClaimDispositionKind::AbandonNoExternalEffect,
        "abandon_effect_reconciled" => ClaimDispositionKind::AbandonEffectReconciled {
            evidence: EvidenceReference::new(disposition.evidence)?,
        },
        "carry_receipt" => ClaimDispositionKind::CarryReceipt {
            receipt_id: ExecutionReceiptId::new(disposition.receipt_id)?,
        },
        "retry_in_successor" => ClaimDispositionKind::RetryInSuccessor,
        _ => {
            return Err(DomainError::InvalidCharacters {
                field: "claim_disposition.kind",
            })
        }
    };
    Ok(ClaimDisposition::new(
        StepId::new(disposition.step_id)?,
        StepClaimFence::new(disposition.claim_fence)?,
        kind,
    ))
}

pub(crate) fn plan_view_to_proto(view: ProtocolSuccessorPlanView) -> pb::ProtocolSuccessorPlanView {
    pb::ProtocolSuccessorPlanView {
        predecessor_definition: Some(pin_to_proto(&view.predecessor_definition)),
        successor_definition: Some(pin_to_proto(&view.successor_definition)),
        diff: Some(diff_protocol_definitions_response_from(&view.diff)),
        preflight: Some(preflight_to_proto(view.preflight)),
        proposed_carried: view
            .proposed_carried
            .iter()
            .map(|proposed| pb::ProtocolProposedCarriedStep {
                step_id: proposed.step_id.as_str().to_owned(),
                source: Some(source_to_proto(&proposed.source)),
            })
            .collect(),
        required_dispositions: view
            .required_dispositions
            .iter()
            .map(|required| pb::ProtocolRequiredDisposition {
                step_id: required.step_id.as_str().to_owned(),
                claim_fence: required.claim_fence.as_str().to_owned(),
                phase: label(required.phase),
            })
            .collect(),
        strands: view
            .strands
            .iter()
            .map(|change| pb::ProtocolDefinitionChange {
                kind: change.kind().as_label().to_owned(),
                locus: serde_json::to_value(change.locus())
                    .ok()
                    .as_ref()
                    .and_then(super::attributes::struct_from_json),
                impact: change.impact().as_label().to_owned(),
                detail: change.detail().to_owned(),
            })
            .collect(),
        blockers: view
            .blockers
            .iter()
            .map(|blocker| blocker.as_str().to_owned())
            .collect(),
        ready: view.ready,
    }
}

pub(crate) fn plan_to_proto(plan: &SuccessionPlan) -> pb::ProtocolSuccessionPlan {
    pb::ProtocolSuccessionPlan {
        plan_id: plan.plan_id().as_str().to_owned(),
        successor_id: plan.successor_id().as_str().to_owned(),
        successor_definition: Some(pin_to_proto(plan.successor_definition())),
        carried: plan.carried().iter().map(carried_to_proto).collect(),
        dispositions: plan
            .dispositions()
            .iter()
            .map(disposition_to_proto)
            .collect(),
        budget: plan.budget().as_label().to_owned(),
        planned_by: plan.planned_by().as_str().to_owned(),
        planned_at: moment(plan.planned_at()),
    }
}

pub(crate) fn succession_to_proto(succession: &ProtocolSuccession) -> pb::ProtocolSuccessionState {
    pb::ProtocolSuccessionState {
        predecessor_id: succession.predecessor_id().as_str().to_owned(),
        predecessor_head: succession.predecessor_head().to_hex(),
        predecessor_version: succession.predecessor_version().value(),
        predecessor_definition: Some(pin_to_proto(succession.predecessor_definition())),
        successor_definition: Some(pin_to_proto(succession.successor_definition())),
        plan_id: succession.plan_id().as_str().to_owned(),
    }
}

pub(crate) fn source_to_proto(source: &SourceRecordRef) -> pb::ProtocolSourceRecordRef {
    pb::ProtocolSourceRecordRef {
        protocol_id: source.protocol_id().as_str().to_owned(),
        step_id: source.step_id().as_str().to_owned(),
        event_id: source.event_id().as_str().to_owned(),
        record_hash: source.record_hash().to_hex(),
        state_visit: source.state_visit().get(),
        attempt: source.attempt().get(),
    }
}

fn carried_to_proto(carried: &CarriedEvidence) -> pb::ProtocolCarriedEvidence {
    pb::ProtocolCarriedEvidence {
        successor_step_id: carried.successor_step_id().as_str().to_owned(),
        source: Some(source_to_proto(carried.source())),
        output: Some(attributes_to_struct(carried.output().attributes())),
    }
}

fn disposition_to_proto(disposition: &ClaimDisposition) -> pb::ProtocolClaimDisposition {
    let (evidence, receipt_id) = match disposition.kind() {
        ClaimDispositionKind::AbandonEffectReconciled { evidence } => {
            (evidence.as_str().to_owned(), String::new())
        }
        ClaimDispositionKind::CarryReceipt { receipt_id } => {
            (String::new(), receipt_id.as_str().to_owned())
        }
        ClaimDispositionKind::AbandonNoExternalEffect | ClaimDispositionKind::RetryInSuccessor => {
            (String::new(), String::new())
        }
    };
    pb::ProtocolClaimDisposition {
        step_id: disposition.step_id().as_str().to_owned(),
        claim_fence: disposition.claim_fence().as_str().to_owned(),
        kind: disposition.kind().as_label().to_owned(),
        evidence,
        receipt_id,
    }
}

fn pin_to_proto(pin: &DefinitionPin) -> pb::ProtocolDefinitionPin {
    pb::ProtocolDefinitionPin {
        name: pin.name().as_str().to_owned(),
        version: pin.version().as_str().to_owned(),
        digest: pin.digest().to_hex(),
    }
}

fn label(phase: made_core::value_objects::ProtocolClaimPhase) -> String {
    serde_json::to_value(phase)
        .ok()
        .and_then(|value| value.as_str().map(ToOwned::to_owned))
        .unwrap_or_default()
}
