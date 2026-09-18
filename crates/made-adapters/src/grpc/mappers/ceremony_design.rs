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
    CeremonyDesignGroup, CeremonyDesignGroupRepeat, CeremonyDesignGroupRepeatUntil,
    CeremonyDesignGroupStep, CeremonyDesignJoin, CeremonyDesignOutputFieldGuard,
    CeremonyDesignParticipant, CeremonyDesignRepeat, CeremonyDesignStage, CeremonyDesignStageEntry,
    CeremonyDesignStepRepeatExhaustedGuard, CeremonyDraftView, CeremonyParticipantCapability,
    CeremonyPatternPreset, DesignedCeremony,
};
use made_core::error::DomainError;
use made_core::value_objects::{
    CeremonyDescription, CeremonyName, CeremonyVersion, ContextKey, ContextWrites, DurationMs,
    DynamicRoleBinding, GuardName, InputName, JoinStepCount, MaxBounces, MaxParallel,
    MaxTransitions, NumAgents, OutputName, PriorContext, RoleId, Rounds, StateExecution,
    StateIteration, StepAttempt, StepHandlerKind, StepId, StepInstructions, StepIteration,
    StepOutputField, StepTimeout, TransitionTrigger,
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
        .map(stage_entry_from_proto)
        .collect::<Result<Vec<_>, _>>()?;
    let final_approval = request
        .final_approval
        .map(final_approval_from_proto)
        .transpose()?;

    let mut document = CeremonyDesignDocument::new(
        CeremonyName::new(request.name)?,
        named(request.version, CeremonyVersion::new)?,
        CeremonyDescription::new(request.objective)?,
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
    let role_from = stage.role_from.clone();
    let allowed_roles = stage.allowed_roles.clone();
    let context_writes = stage.context_writes.clone();
    let designed = CeremonyDesignStage::new(
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
    .with_exit_guards(exit_guards);
    apply_dynamic_fields(designed, role_from, allowed_roles, context_writes)
}

fn stage_entry_from_proto(
    stage: pb::CeremonyDesignStage,
) -> Result<CeremonyDesignStageEntry, DomainError> {
    if let Some(group) = stage.group {
        if !stage.owner_role_id.is_empty()
            || !stage.instructions.is_empty()
            || !stage.handler.is_empty()
            || stage.see_prior.is_some()
            || stage.num_agents.is_some()
            || stage.review_rounds != 0
            || stage.repeat.is_some()
            || !stage.exit_guards.is_empty()
            || !stage.role_from.is_empty()
            || !stage.allowed_roles.is_empty()
            || !stage.context_writes.is_empty()
        {
            return Err(DomainError::InvalidDocument {
                reason: format!("group stage `{}` cannot also declare leaf fields", stage.id),
            });
        }
        let execution = match group.execution.as_str() {
            "" | "sequential" => StateExecution::Sequential,
            "concurrent" => StateExecution::Concurrent,
            _ => {
                return Err(DomainError::InvalidDocument {
                    reason: "group execution must be `sequential` or `concurrent`".to_owned(),
                })
            }
        };
        let join = match group.join {
            None => CeremonyDesignJoin::AllStepsCompleted,
            Some(join) => match join.condition.as_str() {
                "" | "all_steps_completed" if join.count.is_none() => {
                    CeremonyDesignJoin::AllStepsCompleted
                }
                "any_step_completed" if join.count.is_none() => {
                    CeremonyDesignJoin::AnyStepCompleted
                }
                "steps_completed" => CeremonyDesignJoin::StepsCompleted(JoinStepCount::new(
                    join.count.ok_or_else(|| DomainError::InvalidDocument {
                        reason: "steps_completed join requires count".to_owned(),
                    })?,
                )?),
                _ => {
                    return Err(DomainError::InvalidDocument {
                        reason: "invalid group join condition or count".to_owned(),
                    })
                }
            },
        };
        let steps = group
            .steps
            .into_iter()
            .map(group_step_from_proto)
            .collect::<Result<Vec<_>, _>>()?;
        let designed_group =
            CeremonyDesignGroup::new(StepId::new(stage.id)?, execution, steps, join);
        let designed_group = match group.repeat {
            Some(repeat) => designed_group.with_repeat(group_repeat_from_proto(repeat)?),
            None => designed_group,
        };
        return Ok(CeremonyDesignStageEntry::Group(designed_group));
    }
    stage_from_proto(stage).map(CeremonyDesignStageEntry::Leaf)
}

fn group_repeat_from_proto(
    repeat: pb::CeremonyDesignGroupRepeat,
) -> Result<CeremonyDesignGroupRepeat, DomainError> {
    let until = repeat.until.ok_or_else(|| DomainError::InvalidDocument {
        reason: "field `stages[].group.repeat.until` is required".to_owned(),
    })?;
    let equals = until.equals.ok_or_else(|| DomainError::InvalidDocument {
        reason: "field `stages[].group.repeat.until.equals` is required".to_owned(),
    })?;
    Ok(CeremonyDesignGroupRepeat::new(
        StateIteration::new(repeat.max_iterations)?,
        CeremonyDesignGroupRepeatUntil::new(
            StepId::new(until.step_id)?,
            StepOutputField::new(until.output_field)?,
            pb_value_to_json(equals)?,
        ),
    ))
}

fn group_step_from_proto(
    step: pb::CeremonyDesignGroupStep,
) -> Result<CeremonyDesignGroupStep, DomainError> {
    let designed = CeremonyDesignStage::new(
        StepId::new(step.id)?,
        RoleId::new(step.owner_role_id)?,
        StepInstructions::new(step.instructions)?,
        named(step.handler, StepHandlerKind::new)?,
        step.see_prior.map(PriorContext::from_visible),
        step.num_agents
            .map(|value| NumAgents::new(u32::try_from(value).unwrap_or(u32::MAX)))
            .transpose()?,
        Rounds::new(u32::try_from(step.review_rounds).unwrap_or(u32::MAX))?,
        step.repeat.map(repeat_from_proto).transpose()?,
    );
    Ok(CeremonyDesignGroupStep::new(apply_dynamic_fields(
        designed,
        step.role_from,
        step.allowed_roles,
        step.context_writes,
    )?))
}

fn apply_dynamic_fields(
    mut stage: CeremonyDesignStage,
    role_from: String,
    allowed_roles: Vec<String>,
    context_writes: std::collections::HashMap<String, String>,
) -> Result<CeremonyDesignStage, DomainError> {
    match (role_from.is_empty(), allowed_roles.is_empty()) {
        (true, true) => {}
        (false, false) => {
            stage = stage.with_dynamic_role_binding(DynamicRoleBinding::new(
                ContextKey::from_role_from(&role_from)?,
                allowed_roles
                    .into_iter()
                    .map(RoleId::new)
                    .collect::<Result<Vec<_>, _>>()?,
            )?);
        }
        _ => {
            return Err(DomainError::InvalidDocument {
                reason: "role_from and allowed_roles must be declared together".to_owned(),
            });
        }
    }
    let writes = context_writes
        .into_iter()
        .map(|(destination, source)| {
            Ok((ContextKey::new(destination)?, StepOutputField::new(source)?))
        })
        .collect::<Result<std::collections::BTreeMap<_, _>, DomainError>>()?;
    Ok(stage.with_context_writes(ContextWrites::new(writes)))
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

    #[test]
    fn direct_grpc_maps_concurrent_group_join_and_definition_limit() {
        let request = pb::DesignCeremonyRequest {
            name: "parallel_review".to_owned(),
            version: String::new(),
            objective: "Review independently".to_owned(),
            required_inputs: Vec::new(),
            optional_inputs: Vec::new(),
            outputs: vec!["decision".to_owned()],
            participants: vec![
                pb::CeremonyDesignParticipant {
                    role_id: "A".to_owned(),
                    capabilities: Vec::new(),
                },
                pb::CeremonyDesignParticipant {
                    role_id: "B".to_owned(),
                    capabilities: Vec::new(),
                },
            ],
            stages: vec![pb::CeremonyDesignStage {
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
                context_writes: std::collections::HashMap::new(),
                group: Some(pb::CeremonyDesignGroup {
                    execution: "concurrent".to_owned(),
                    steps: ["A", "B"]
                        .into_iter()
                        .enumerate()
                        .map(|(index, owner)| pb::CeremonyDesignGroupStep {
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
                            context_writes: std::collections::HashMap::new(),
                        })
                        .collect(),
                    join: Some(pb::CeremonyDesignGroupJoin {
                        condition: "steps_completed".to_owned(),
                        count: Some(1),
                    }),
                    repeat: Some(pb::CeremonyDesignGroupRepeat {
                        max_iterations: 3,
                        until: Some(pb::CeremonyDesignGroupRepeatUntil {
                            step_id: "review_2".to_owned(),
                            output_field: "ready".to_owned(),
                            equals: Some(prost_types::Value {
                                kind: Some(Kind::BoolValue(true)),
                            }),
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

        let document = ceremony_design_document_from_proto(request).unwrap();
        assert_eq!(document.max_parallel(), MaxParallel::new(2).unwrap());
        assert_eq!(document.max_transitions().unwrap().get(), 12);
        assert_eq!(document.max_bounces().unwrap().get(), 3);
        assert!(matches!(
            &document.stage_entries()[0],
            CeremonyDesignStageEntry::Group(group)
                if group.execution() == StateExecution::Concurrent
                    && matches!(group.join(), CeremonyDesignJoin::StepsCompleted(count) if count.get() == 1)
                    && group.repeat().is_some_and(|repeat|
                        repeat.max_iterations() == StateIteration::new(3).unwrap()
                            && repeat.until().step_id() == &StepId::new("review_2").unwrap()
                            && repeat.until().output_field().as_str() == "ready"
                            && repeat.until().equals() == &serde_json::json!(true))
        ));
    }

    #[test]
    fn direct_grpc_rejects_zero_transition_budgets() {
        let request = pb::DesignCeremonyRequest {
            name: "invalid_budget".to_owned(),
            objective: "Reject zero".to_owned(),
            max_transitions: Some(0),
            ..pb::DesignCeremonyRequest::default()
        };
        assert!(matches!(
            ceremony_design_document_from_proto(request),
            Err(DomainError::MustBeNonZero {
                field: "max_transitions"
            })
        ));

        let request = pb::DesignCeremonyRequest {
            name: "invalid_budget".to_owned(),
            objective: "Reject zero".to_owned(),
            max_bounces: Some(0),
            ..pb::DesignCeremonyRequest::default()
        };
        assert!(matches!(
            ceremony_design_document_from_proto(request),
            Err(DomainError::MustBeNonZero {
                field: "max_bounces"
            })
        ));
    }
}
