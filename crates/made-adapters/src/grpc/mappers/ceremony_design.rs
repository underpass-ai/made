//! Designing a ceremony: proto ↔ application.
//!
//! The request mirrors the authoring intent field for field, and this
//! reads it into [`CeremonyDesignDocument`] without deciding anything:
//! what an omitted field means belongs to
//! [`made_app::usecases::DesignCeremonyUseCase`], which is why an
//! absent number arrives here as `None` rather than as a zero this
//! mapper would have to interpret.
//!
//! The answer carries the analysis of the rendered draft in the same
//! fields `ValidateCeremonyDraft` answers with, from the same
//! [`CeremonyDraftView`]: a designed draft is analysed exactly like a
//! hand-authored one, so a client can read either the same way.

use made_app::usecases::{
    CeremonyDesignDocument, CeremonyDesignFinalApproval, CeremonyDesignParticipant,
    CeremonyDesignRepeat, CeremonyDesignStage, CeremonyDraftView, CeremonyParticipantCapability,
    DesignedCeremony,
};
use made_core::error::DomainError;
use made_core::value_objects::{
    CeremonyDescription, CeremonyName, CeremonyVersion, GuardName, InputName, OutputName, RoleId,
    StepHandlerKind, StepId, StepIteration, StepOutputField, TransitionTrigger,
};
use made_proto::v1 as pb;

use super::attributes::pb_value_to_json;
use super::ceremony_authoring::{count, finding_from};

pub fn ceremony_design_document_from_proto(
    request: pb::DesignCeremonyRequest,
) -> Result<CeremonyDesignDocument, DomainError> {
    let participants = request
        .participants
        .into_iter()
        .map(participant_from_proto)
        .collect::<Result<Vec<_>, _>>()?;
    let stages = request
        .stages
        .into_iter()
        .map(stage_from_proto)
        .collect::<Result<Vec<_>, _>>()?;
    let final_approval = request
        .final_approval
        .map(final_approval_from_proto)
        .transpose()?;

    Ok(CeremonyDesignDocument::new(
        CeremonyName::new(request.name)?,
        named(request.version, CeremonyVersion::new)?,
        CeremonyDescription::new(request.objective)?,
        each(request.required_inputs, InputName::new)?,
        each(request.optional_inputs, InputName::new)?,
        each(request.outputs, OutputName::new)?,
        participants,
        stages,
        final_approval,
        request.step_timeout_seconds,
        request.max_attempts,
        request.backoff_seconds,
    ))
}

pub fn design_ceremony_response_from(
    designed: &DesignedCeremony,
    view: &CeremonyDraftView<'_>,
) -> pb::DesignCeremonyResponse {
    pb::DesignCeremonyResponse {
        ceremony: view.name().as_str().to_owned(),
        version: view.version().as_str().to_owned(),
        definition_yaml: designed.definition_yaml().to_owned(),
        publishable: view.is_publishable(),
        design: Some(pb::CeremonyDesignOutline {
            topology: designed.topology().to_owned(),
            stages: count(designed.stage_count()),
            participants: count(designed.participant_count()),
            final_approval_required: designed.final_approval_required(),
        }),
        error_count: count(view.error_count()),
        warning_count: count(view.warning_count()),
        findings: view.findings().iter().map(finding_from).collect(),
    }
}

fn participant_from_proto(
    participant: pb::CeremonyDesignParticipant,
) -> Result<CeremonyDesignParticipant, DomainError> {
    Ok(CeremonyDesignParticipant::new(
        RoleId::new(participant.role_id)?,
        participant
            .capabilities
            .iter()
            .map(|capability| CeremonyParticipantCapability::parse(capability))
            .collect::<Result<Vec<_>, _>>()?,
    ))
}

fn stage_from_proto(stage: pb::CeremonyDesignStage) -> Result<CeremonyDesignStage, DomainError> {
    Ok(CeremonyDesignStage::new(
        StepId::new(stage.id)?,
        RoleId::new(stage.owner_role_id)?,
        stage.instructions,
        named(stage.handler, StepHandlerKind::new)?,
        stage.see_prior,
        stage.num_agents,
        stage.review_rounds,
        stage.repeat.map(repeat_from_proto).transpose()?,
    ))
}

fn repeat_from_proto(
    repeat: pb::CeremonyDesignRepeat,
) -> Result<CeremonyDesignRepeat, DomainError> {
    Ok(CeremonyDesignRepeat::new(
        StepIteration::new(repeat.max_iterations)?,
        StepOutputField::new(repeat.output_field)?,
        // Absent is null: a repeat that ends when the field is null is
        // a thing a caller can mean, and it is what "no value" means
        // on the wire.
        repeat
            .equals
            .map_or(serde_json::Value::Null, pb_value_to_json),
    ))
}

fn final_approval_from_proto(
    approval: pb::CeremonyDesignFinalApproval,
) -> Result<CeremonyDesignFinalApproval, DomainError> {
    Ok(CeremonyDesignFinalApproval::new(
        RoleId::new(approval.role_id)?,
        named(approval.guard_name, GuardName::new)?,
        named(approval.trigger, TransitionTrigger::new)?,
    ))
}

/// An empty string is an absent field on the wire, where proto3 has no
/// other way to say it. Every one of these is `minLength: 1` in the
/// tool schema that fronts it, so a caller who meant to say something
/// never says it with "".
fn named<T>(
    value: String,
    constructor: impl Fn(String) -> Result<T, DomainError>,
) -> Result<Option<T>, DomainError> {
    if value.is_empty() {
        return Ok(None);
    }
    constructor(value).map(Some)
}

fn each<T>(
    values: Vec<String>,
    constructor: impl Fn(String) -> Result<T, DomainError>,
) -> Result<Vec<T>, DomainError> {
    values.into_iter().map(constructor).collect()
}
