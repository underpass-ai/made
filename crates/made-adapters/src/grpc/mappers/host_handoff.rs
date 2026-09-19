use made_app::workers::{InspectCeremonyResumeInput, RecordCeremonyHostHandoffInput};
use made_core::entities::ceremony_events::HostHandoffRecorded;
use made_core::value_objects::{
    CeremonyId, EvidenceReference, ExecutionRecoveryPageLimit, HostAgentIncarnation,
    HostHandoffDeclaration, IdempotencyKey, LeaseOwnerId, StepClaimFence, StepId,
};
use made_core::DomainError;
use made_proto::v1 as pb;

pub(crate) fn handoff_input(
    request: pb::RecordCeremonyHostHandoffRequest,
) -> Result<RecordCeremonyHostHandoffInput, DomainError> {
    let d = request.declaration.ok_or(DomainError::EmptyField {
        field: "declaration",
    })?;
    let declaration = HostHandoffDeclaration {
        id: IdempotencyKey::new(d.id)?,
        step_id: StepId::new(d.step_id)?,
        claim_fence: StepClaimFence::new(d.claim_fence)?,
        owner: LeaseOwnerId::new(d.owner)?,
        incarnation: HostAgentIncarnation::new(d.incarnation)?,
        state: serde_json::from_value(serde_json::Value::String(d.state)).map_err(|_| {
            DomainError::InvalidCharacters {
                field: "host_handoff.state",
            }
        })?,
        observed_at: time::OffsetDateTime::parse(
            &d.observed_at,
            &time::format_description::well_known::Rfc3339,
        )
        .map_err(|_| DomainError::InvalidCharacters {
            field: "host_handoff.observed_at",
        })?,
        evidence: EvidenceReference::new(d.evidence)?,
    };
    declaration.validate()?;
    Ok(RecordCeremonyHostHandoffInput {
        ceremony_id: CeremonyId::new(request.ceremony_id)?,
        declaration,
    })
}

pub(crate) fn preflight_input(
    request: pb::InspectCeremonyResumeRequest,
) -> Result<InspectCeremonyResumeInput, DomainError> {
    let limit = if request.limit == 0 {
        ExecutionRecoveryPageLimit::DEFAULT
    } else {
        ExecutionRecoveryPageLimit::new(
            u16::try_from(request.limit)
                .map_err(|_| DomainError::InvalidCharacters { field: "limit" })?,
        )?
    };
    Ok(InspectCeremonyResumeInput {
        ceremony_id: CeremonyId::new(request.ceremony_id)?,
        after_claim: if request.after_claim.is_empty() {
            None
        } else {
            Some(StepClaimFence::new(request.after_claim)?)
        },
        limit,
    })
}

pub(crate) fn moment(at: time::OffsetDateTime) -> String {
    at.format(&time::format_description::well_known::Rfc3339)
        .unwrap_or_default()
}

pub(crate) fn label(value: impl serde::Serialize) -> String {
    serde_json::to_value(value)
        .expect("enum label")
        .as_str()
        .expect("enum string")
        .to_owned()
}

pub(crate) fn handoff_to_proto(recorded: HostHandoffRecorded) -> pb::HostHandoffRecorded {
    let d = recorded.declaration;
    pb::HostHandoffRecorded {
        declaration: Some(pb::HostHandoffDeclaration {
            id: d.id.as_str().into(),
            step_id: d.step_id.as_str().into(),
            claim_fence: d.claim_fence.as_str().into(),
            owner: d.owner.as_str().into(),
            incarnation: d.incarnation.as_str().into(),
            state: label(d.state),
            observed_at: moment(d.observed_at),
            evidence: d.evidence.as_str().into(),
        }),
        recorded_at: moment(recorded.recorded_at),
    }
}
