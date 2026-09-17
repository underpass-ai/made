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
    CeremonyDesignDocument, CeremonyDesignExitGuard, CeremonyDesignFinalApproval,
    CeremonyDesignOutputFieldGuard, CeremonyDesignParticipant, CeremonyDesignRepeat,
    CeremonyDesignStage, CeremonyDesignStepRepeatExhaustedGuard, CeremonyDraftView,
    CeremonyParticipantCapability, CeremonyPatternPreset, DesignedCeremony,
};
use made_core::error::DomainError;
use made_core::value_objects::{
    CeremonyDescription, CeremonyName, CeremonyVersion, DurationMs, GuardName, InputName,
    NumAgents, OutputName, PriorContext, RoleId, Rounds, StepAttempt, StepHandlerKind, StepId,
    StepInstructions, StepIteration, StepOutputField, StepTimeout, TransitionTrigger,
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

    let document = CeremonyDesignDocument::new(
        CeremonyName::new(request.name)?,
        named(request.version, CeremonyVersion::new)?,
        CeremonyDescription::new(request.objective)?,
        each(request.required_inputs, InputName::new)?,
        each(request.optional_inputs, InputName::new)?,
        each(request.outputs, OutputName::new)?,
        participants,
        stages,
        final_approval,
        request
            .step_timeout_seconds
            .map(|seconds| StepTimeout::new(DurationMs::from_millis(seconds.saturating_mul(1_000))))
            .transpose()?,
        request.max_attempts.map(StepAttempt::new).transpose()?,
        request
            .backoff_seconds
            .map(|seconds| DurationMs::from_millis(seconds.saturating_mul(1_000))),
    );
    let pattern = named(request.pattern, |value| {
        CeremonyPatternPreset::parse(&value)
    })?;
    Ok(pattern.map_or(document.clone(), |pattern| document.with_pattern(pattern)))
}

pub fn design_ceremony_response_from(
    designed: &DesignedCeremony,
    definition_yaml: String,
    view: &CeremonyDraftView<'_>,
) -> pb::DesignCeremonyResponse {
    pb::DesignCeremonyResponse {
        ceremony: view.name().as_str().to_owned(),
        version: view.version().as_str().to_owned(),
        definition_yaml,
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
    let exit_guards = stage
        .exit_guards
        .into_iter()
        .map(exit_guard_from_proto)
        .collect::<Result<Vec<_>, _>>()?;
    Ok(CeremonyDesignStage::new(
        StepId::new(stage.id)?,
        RoleId::new(stage.owner_role_id)?,
        StepInstructions::new(stage.instructions)?,
        named(stage.handler, StepHandlerKind::new)?,
        stage.see_prior.map(PriorContext::from_visible),
        stage
            .num_agents
            .map(|value| NumAgents::new(u32::try_from(value).unwrap_or(u32::MAX)))
            .transpose()?,
        Rounds::new(u32::try_from(stage.review_rounds).unwrap_or(u32::MAX))?,
        stage.repeat.map(repeat_from_proto).transpose()?,
    )
    .with_exit_guards(exit_guards))
}

fn exit_guard_from_proto(
    guard: pb::CeremonyDesignExitGuard,
) -> Result<CeremonyDesignExitGuard, DomainError> {
    let guard = guard.guard.ok_or_else(|| DomainError::InvalidDocument {
        reason: "field `stages[].exit_guards[].kind` is required".to_owned(),
    })?;
    match guard {
        pb::ceremony_design_exit_guard::Guard::OutputField(guard) => {
            let expected = guard.equals.ok_or_else(|| DomainError::InvalidDocument {
                reason: "field `stages[].exit_guards[].equals` is required".to_owned(),
            })?;
            Ok(CeremonyDesignExitGuard::OutputField(
                CeremonyDesignOutputFieldGuard::new(
                    StepId::new(guard.step_id)?,
                    StepOutputField::new(guard.output_field)?,
                    pb_value_to_json(expected)?,
                ),
            ))
        }
        pb::ceremony_design_exit_guard::Guard::StepRepeatExhausted(guard) => {
            Ok(CeremonyDesignExitGuard::StepRepeatExhausted(
                CeremonyDesignStepRepeatExhaustedGuard::new(StepId::new(guard.step_id)?),
            ))
        }
    }
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
            .map_or(Ok(serde_json::Value::Null), pb_value_to_json)?,
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

#[cfg(test)]
mod tests {
    use prost_types::value::Kind;

    use super::*;

    #[test]
    fn direct_grpc_output_guard_distinguishes_absent_from_explicit_null() {
        let absent = pb::CeremonyDesignExitGuard {
            guard: Some(pb::ceremony_design_exit_guard::Guard::OutputField(
                pb::CeremonyDesignOutputFieldGuard {
                    step_id: "compose".to_owned(),
                    output_field: "answer".to_owned(),
                    equals: None,
                },
            )),
        };
        assert!(exit_guard_from_proto(absent)
            .unwrap_err()
            .to_string()
            .contains("equals"));

        let explicit_null = pb::CeremonyDesignExitGuard {
            guard: Some(pb::ceremony_design_exit_guard::Guard::OutputField(
                pb::CeremonyDesignOutputFieldGuard {
                    step_id: "compose".to_owned(),
                    output_field: "answer".to_owned(),
                    equals: Some(prost_types::Value {
                        kind: Some(Kind::NullValue(0)),
                    }),
                },
            )),
        };
        assert!(matches!(
            exit_guard_from_proto(explicit_null).unwrap(),
            CeremonyDesignExitGuard::OutputField(guard)
                if guard.expected() == &serde_json::Value::Null
        ));
    }
}
