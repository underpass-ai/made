//! Designing a protocol: proto ↔ application.
//!
//! The request mirrors the authoring intent field for field, and this
//! reads it into [`ProtocolDesignDocument`] without deciding anything:
//! what an omitted field means belongs to
//! [`made_app::usecases::DesignProtocolUseCase`], which is why an
//! absent number arrives here as `None` rather than as a zero this
//! mapper would have to interpret.
//!
//! The answer carries the analysis of the rendered draft in the same
//! fields `ValidateProtocolDraft` answers with, from the same
//! [`ProtocolDraftView`]: a designed draft is analysed exactly like a
//! hand-authored one, so a client can read either the same way.

use made_app::usecases::{
    DesignedProtocol, ProtocolDesignDocument, ProtocolDesignExitGuard, ProtocolDesignFinalApproval,
    ProtocolDesignOutputFieldGuard, ProtocolDesignParticipant,
    ProtocolDesignStepRepeatExhaustedGuard, ProtocolDraftView, ProtocolParticipantCapability,
    ProtocolPatternPreset,
};
use made_core::error::DomainError;
use made_core::value_objects::{
    ChildJoin, ChildQuorum, ChildrenCompletedCondition, DurationMs, GuardName, InputName,
    MaxBounces, MaxParallel, MaxTransitions, OutputName, ProtocolDescription, ProtocolName,
    ProtocolVersion, RoleId, StepAttempt, StepId, StepOutputField, StepTimeout, TransitionTrigger,
};
use made_proto::v1 as pb;

use super::attributes::pb_value_to_json;
use super::protocol_authoring::{count, finding_from};
use super::protocol_design_stage::stage_entry_from_proto;

pub fn protocol_design_document_from_proto(
    request: pb::DesignProtocolRequest,
) -> Result<ProtocolDesignDocument, DomainError> {
    let participants = request
        .participants
        .into_iter()
        .map(participant_from_proto)
        .collect::<Result<Vec<_>, _>>()?;
    let stages = request
        .stages
        .into_iter()
        .map(stage_entry_from_proto)
        .collect::<Result<Vec<_>, _>>()?;
    let final_approval = request
        .final_approval
        .map(final_approval_from_proto)
        .transpose()?;

    let mut document = ProtocolDesignDocument::new(
        ProtocolName::new(request.name)?,
        named(request.version, ProtocolVersion::new)?,
        ProtocolDescription::new(request.objective)?,
        each(request.required_inputs, InputName::new)?,
        each(request.optional_inputs, InputName::new)?,
        each(request.outputs, OutputName::new)?,
        participants,
        Vec::new(),
        final_approval,
        request
            .step_timeout_seconds
            .map(|seconds| StepTimeout::new(DurationMs::from_millis(seconds.saturating_mul(1_000))))
            .transpose()?,
        request.max_attempts.map(StepAttempt::new).transpose()?,
        request
            .backoff_seconds
            .map(|seconds| DurationMs::from_millis(seconds.saturating_mul(1_000))),
    )
    .with_stage_entries(stages)
    .with_max_parallel(match request.max_parallel {
        Some(value) => MaxParallel::new(u8::try_from(value).unwrap_or(u8::MAX))?,
        None => MaxParallel::default(),
    });
    if let Some(limit) = request.max_transitions {
        document = document.with_max_transitions(MaxTransitions::new(limit)?);
    }
    if let Some(limit) = request.max_bounces {
        document = document.with_max_bounces(MaxBounces::new(limit)?);
    }
    let pattern = named(request.pattern, |value| {
        ProtocolPatternPreset::parse(&value)
    })?;
    Ok(pattern.map_or(document.clone(), |pattern| document.with_pattern(pattern)))
}

pub fn design_protocol_response_from(
    designed: &DesignedProtocol,
    definition_yaml: String,
    view: &ProtocolDraftView<'_>,
) -> pb::DesignProtocolResponse {
    pb::DesignProtocolResponse {
        protocol: view.name().as_str().to_owned(),
        version: view.version().as_str().to_owned(),
        definition_yaml,
        publishable: view.is_publishable(),
        design: Some(pb::ProtocolDesignOutline {
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
    participant: pb::ProtocolDesignParticipant,
) -> Result<ProtocolDesignParticipant, DomainError> {
    Ok(ProtocolDesignParticipant::new(
        RoleId::new(participant.role_id)?,
        participant
            .capabilities
            .iter()
            .map(|capability| ProtocolParticipantCapability::parse(capability))
            .collect::<Result<Vec<_>, _>>()?,
    ))
}

pub(super) fn exit_guard_from_proto(
    guard: pb::ProtocolDesignExitGuard,
) -> Result<ProtocolDesignExitGuard, DomainError> {
    let guard = guard.guard.ok_or_else(|| DomainError::InvalidDocument {
        reason: "field `stages[].exit_guards[].kind` is required".to_owned(),
    })?;
    match guard {
        pb::protocol_design_exit_guard::Guard::OutputField(guard) => {
            let expected = guard.equals.ok_or_else(|| DomainError::InvalidDocument {
                reason: "field `stages[].exit_guards[].equals` is required".to_owned(),
            })?;
            Ok(ProtocolDesignExitGuard::OutputField(
                ProtocolDesignOutputFieldGuard::new(
                    StepId::new(guard.step_id)?,
                    StepOutputField::new(guard.output_field)?,
                    pb_value_to_json(expected)?,
                ),
            ))
        }
        pb::protocol_design_exit_guard::Guard::StepRepeatExhausted(guard) => {
            Ok(ProtocolDesignExitGuard::StepRepeatExhausted(
                ProtocolDesignStepRepeatExhaustedGuard::new(StepId::new(guard.step_id)?),
            ))
        }
        pb::protocol_design_exit_guard::Guard::ChildrenCompleted(condition) => {
            let join = match condition.join.ok_or_else(|| DomainError::InvalidDocument {
                reason: "field `children_completed.join` is required".to_owned(),
            })? {
                pb::children_completed_condition::Join::All(true) => ChildJoin::All,
                pb::children_completed_condition::Join::Any(true) => ChildJoin::Any,
                pb::children_completed_condition::Join::Quorum(count) => ChildJoin::Quorum {
                    count: ChildQuorum::new(u16::try_from(count).unwrap_or(u16::MAX))?,
                },
                _ => {
                    return Err(DomainError::InvalidDocument {
                        reason: "children_completed all/any join must be true".to_owned(),
                    })
                }
            };
            Ok(ProtocolDesignExitGuard::ChildrenCompleted(
                ChildrenCompletedCondition::new(StepId::new(condition.step_id)?, join),
            ))
        }
    }
}

fn final_approval_from_proto(
    approval: pb::ProtocolDesignFinalApproval,
) -> Result<ProtocolDesignFinalApproval, DomainError> {
    Ok(ProtocolDesignFinalApproval::new(
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
    use made_app::usecases::{ProtocolDesignJoin, ProtocolDesignStageEntry};
    use made_core::value_objects::{StateExecution, StateIteration};
    use prost_types::value::Kind;

    use super::*;

    #[test]
    fn direct_grpc_output_guard_distinguishes_absent_from_explicit_null() {
        let absent = pb::ProtocolDesignExitGuard {
            guard: Some(pb::protocol_design_exit_guard::Guard::OutputField(
                pb::ProtocolDesignOutputFieldGuard {
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

        let explicit_null = pb::ProtocolDesignExitGuard {
            guard: Some(pb::protocol_design_exit_guard::Guard::OutputField(
                pb::ProtocolDesignOutputFieldGuard {
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
            ProtocolDesignExitGuard::OutputField(guard)
                if guard.expected() == &serde_json::Value::Null
        ));
    }

    #[test]
    fn direct_grpc_maps_concurrent_group_join_and_definition_limit() {
        let request = pb::DesignProtocolRequest {
            name: "parallel_review".to_owned(),
            version: String::new(),
            objective: "Review independently".to_owned(),
            required_inputs: Vec::new(),
            optional_inputs: Vec::new(),
            outputs: vec!["decision".to_owned()],
            participants: vec![
                pb::ProtocolDesignParticipant {
                    role_id: "A".to_owned(),
                    capabilities: Vec::new(),
                },
                pb::ProtocolDesignParticipant {
                    role_id: "B".to_owned(),
                    capabilities: Vec::new(),
                },
            ],
            stages: vec![pb::ProtocolDesignStage {
                id: "review".to_owned(),
                owner_role_id: String::new(),
                instructions: String::new(),
                handler: String::new(),
                see_prior: None,
                num_agents: None,
                review_rounds: 0,
                repeat: None,
                role_from: String::new(),
                allowed_roles: Vec::new(),
                context_writes: std::collections::BTreeMap::new(),
                aggregate: None,
                pattern_stage: None,
                spawn: None,
                timeout_seconds: None,
                group: Some(pb::ProtocolDesignGroup {
                    execution: "concurrent".to_owned(),
                    steps: ["A", "B"]
                        .into_iter()
                        .enumerate()
                        .map(|(index, owner)| pb::ProtocolDesignGroupStep {
                            id: format!("review_{}", index + 1),
                            owner_role_id: owner.to_owned(),
                            instructions: "Review".to_owned(),
                            handler: String::new(),
                            see_prior: None,
                            num_agents: None,
                            review_rounds: 0,
                            repeat: None,
                            role_from: String::new(),
                            allowed_roles: Vec::new(),
                            context_writes: std::collections::BTreeMap::new(),
                            aggregate: None,
                            spawn: None,
                            timeout_seconds: None,
                        })
                        .collect(),
                    join: Some(pb::ProtocolDesignGroupJoin {
                        condition: "steps_completed".to_owned(),
                        count: Some(1),
                    }),
                    repeat: Some(pb::ProtocolDesignGroupRepeat {
                        max_iterations: 3,
                        until: Some(pb::ProtocolDesignGroupRepeatUntil {
                            step_id: "review_2".to_owned(),
                            output_field: "ready".to_owned(),
                            equals: Some(prost_types::Value {
                                kind: Some(Kind::BoolValue(true)),
                            }),
                        }),
                        on_exhausted: Some(pb::ProtocolDesignGroupRepeatExhaustion {
                            terminal: "exhausted".to_owned(),
                        }),
                    }),
                }),
                exit_guards: Vec::new(),
            }],
            final_approval: None,
            step_timeout_seconds: None,
            max_attempts: None,
            backoff_seconds: None,
            max_parallel: Some(2),
            pattern: String::new(),
            max_transitions: Some(12),
            max_bounces: Some(3),
        };

        let document = protocol_design_document_from_proto(request).unwrap();
        assert_eq!(document.max_parallel(), MaxParallel::new(2).unwrap());
        assert_eq!(document.max_transitions().unwrap().get(), 12);
        assert_eq!(document.max_bounces().unwrap().get(), 3);
        assert!(matches!(
            &document.stage_entries()[0],
            ProtocolDesignStageEntry::Group(group)
                if group.execution() == StateExecution::Concurrent
                    && matches!(group.join(), ProtocolDesignJoin::StepsCompleted(count) if count.get() == 1)
                    && group.repeat().is_some_and(|repeat|
                        repeat.max_iterations() == StateIteration::new(3).unwrap()
                            && repeat.until().step_id() == &StepId::new("review_2").unwrap()
                            && repeat.until().output_field().as_str() == "ready"
                            && repeat.until().equals() == &serde_json::json!(true)
                            && repeat.exhausted_terminal() == Some(&StepId::new("exhausted").unwrap()))
        ));
    }

    #[test]
    fn direct_grpc_rejects_zero_transition_budgets() {
        let request = pb::DesignProtocolRequest {
            name: "invalid_budget".to_owned(),
            objective: "Reject zero".to_owned(),
            max_transitions: Some(0),
            ..pb::DesignProtocolRequest::default()
        };
        assert!(matches!(
            protocol_design_document_from_proto(request),
            Err(DomainError::MustBeNonZero {
                field: "max_transitions"
            })
        ));

        let request = pb::DesignProtocolRequest {
            name: "invalid_budget".to_owned(),
            objective: "Reject zero".to_owned(),
            max_bounces: Some(0),
            ..pb::DesignProtocolRequest::default()
        };
        assert!(matches!(
            protocol_design_document_from_proto(request),
            Err(DomainError::MustBeNonZero {
                field: "max_bounces"
            })
        ));
    }
}
