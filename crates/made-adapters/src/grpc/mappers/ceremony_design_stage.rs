use made_app::usecases::{
    CeremonyDesignGroup, CeremonyDesignGroupRepeat, CeremonyDesignGroupRepeatUntil,
    CeremonyDesignGroupStep, CeremonyDesignJoin, CeremonyDesignPatternStage, CeremonyDesignRepeat,
    CeremonyDesignStage, CeremonyDesignStageEntry, CeremonyStagePatternKind,
};
use made_core::error::DomainError;
use made_core::value_objects::{
    CeremonyChildSpawn, CeremonyChildSpec, CeremonyName, CeremonyStepAggregation, CeremonyVersion,
    ContextKey, ContextWrites, DynamicRoleBinding, InputName, JoinStepCount, MaxChildDepth,
    MaxChildren, NumAgents, PriorContext, RoleId, Rounds, StateExecution, StateIteration,
    StepHandlerKind, StepId, StepInstructions, StepIteration, StepOutputField,
};
use made_proto::v1 as pb;

use super::attributes::pb_value_to_json;
use super::ceremony_design::exit_guard_from_proto;

fn stage_from_proto(stage: pb::CeremonyDesignStage) -> Result<CeremonyDesignStage, DomainError> {
    let exit_guards = stage
        .exit_guards
        .into_iter()
        .map(exit_guard_from_proto)
        .collect::<Result<Vec<_>, _>>()?;
    let role_from = stage.role_from.clone();
    let allowed_roles = stage.allowed_roles.clone();
    let context_writes = stage.context_writes.clone();
    let aggregate = stage.aggregate.map(aggregation_from_proto).transpose()?;
    let spawn = stage.spawn.map(spawn_from_proto).transpose()?;
    let mut designed = CeremonyDesignStage::new(
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
    if let Some(aggregation) = aggregate {
        designed = designed.with_aggregation(aggregation);
    }
    if let Some(spawn) = spawn {
        designed = designed.with_spawn(spawn);
    }
    apply_dynamic_fields(designed, &role_from, allowed_roles, context_writes)
}

pub(super) fn stage_entry_from_proto(
    mut stage: pb::CeremonyDesignStage,
) -> Result<CeremonyDesignStageEntry, DomainError> {
    if let Some(pattern) = stage.pattern_stage.take() {
        if stage.group.is_some()
            || !stage.owner_role_id.is_empty()
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
            || stage.aggregate.is_some()
            || stage.spawn.is_some()
        {
            return Err(DomainError::InvalidDocument {
                reason: format!(
                    "pattern stage `{}` cannot also declare leaf/group fields",
                    stage.id
                ),
            });
        }
        let mut designed = CeremonyDesignPatternStage::new(
            StepId::new(stage.id)?,
            CeremonyStagePatternKind::parse(&pattern.kind)?,
            pattern
                .roles
                .into_iter()
                .map(RoleId::new)
                .collect::<Result<Vec<_>, _>>()?,
            StepInstructions::new(pattern.instructions)?,
        )
        .with_join(join_from_proto(pattern.join)?);
        if !pattern.manager_role_id.is_empty() {
            designed = designed.with_manager_role(RoleId::new(pattern.manager_role_id)?);
        }
        if let Some(cap) = pattern.max_iterations {
            designed = designed.with_max_iterations(StateIteration::new(cap)?);
        }
        if !pattern.fallback_role_id.is_empty() {
            designed = designed.with_fallback_role(RoleId::new(pattern.fallback_role_id)?);
        }
        return Ok(CeremonyDesignStageEntry::Pattern(designed));
    }
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
            || stage.aggregate.is_some()
            || stage.spawn.is_some()
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
        let join = join_from_proto(group.join)?;
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

fn join_from_proto(
    join: Option<pb::CeremonyDesignGroupJoin>,
) -> Result<CeremonyDesignJoin, DomainError> {
    match join {
        None => Ok(CeremonyDesignJoin::AllStepsCompleted),
        Some(join) => match join.condition.as_str() {
            "" | "all_steps_completed" if join.count.is_none() => {
                Ok(CeremonyDesignJoin::AllStepsCompleted)
            }
            "any_step_completed" if join.count.is_none() => {
                Ok(CeremonyDesignJoin::AnyStepCompleted)
            }
            "steps_completed" => Ok(CeremonyDesignJoin::StepsCompleted(JoinStepCount::new(
                join.count.ok_or_else(|| DomainError::InvalidDocument {
                    reason: "steps_completed join requires count".to_owned(),
                })?,
            )?)),
            _ => Err(DomainError::InvalidDocument {
                reason: "invalid group join condition or count".to_owned(),
            }),
        },
    }
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
    let aggregate = step.aggregate.map(aggregation_from_proto).transpose()?;
    let spawn = step.spawn.map(spawn_from_proto).transpose()?;
    let mut designed = CeremonyDesignStage::new(
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
    if let Some(aggregation) = aggregate {
        designed = designed.with_aggregation(aggregation);
    }
    if let Some(spawn) = spawn {
        designed = designed.with_spawn(spawn);
    }
    Ok(CeremonyDesignGroupStep::new(apply_dynamic_fields(
        designed,
        &step.role_from,
        step.allowed_roles,
        step.context_writes,
    )?))
}

fn aggregation_from_proto(
    aggregate: pb::CeremonyDesignAggregation,
) -> Result<CeremonyStepAggregation, DomainError> {
    match aggregate.strategy {
        Some(pb::ceremony_design_aggregation::Strategy::Synthesize(_)) => {
            Ok(CeremonyStepAggregation::synthesize())
        }
        Some(pb::ceremony_design_aggregation::Strategy::Vote(vote)) => Ok(
            CeremonyStepAggregation::vote(StepOutputField::new(vote.output_field)?),
        ),
        None => Err(DomainError::InvalidDocument {
            reason: "field `aggregate.strategy` is required".to_owned(),
        }),
    }
}

fn spawn_from_proto(spawn: pb::CeremonyChildSpawn) -> Result<CeremonyChildSpawn, DomainError> {
    let children = spawn
        .children
        .into_iter()
        .map(|child| {
            let inputs = child
                .inputs
                .into_iter()
                .map(|(input, context)| Ok((InputName::new(input)?, ContextKey::new(context)?)))
                .collect::<Result<std::collections::BTreeMap<_, _>, DomainError>>()?;
            Ok(CeremonyChildSpec::new(
                CeremonyName::new(child.ceremony)?,
                CeremonyVersion::new(child.version)?,
                inputs,
            ))
        })
        .collect::<Result<Vec<_>, DomainError>>()?;
    CeremonyChildSpawn::new(
        children,
        MaxChildren::new(u16::try_from(spawn.max_children).unwrap_or(u16::MAX))?,
        MaxChildDepth::new(u16::try_from(spawn.max_depth).unwrap_or(u16::MAX))?,
    )
}

fn apply_dynamic_fields(
    mut stage: CeremonyDesignStage,
    role_from: &str,
    allowed_roles: Vec<String>,
    context_writes: std::collections::BTreeMap<String, String>,
) -> Result<CeremonyDesignStage, DomainError> {
    match (role_from.is_empty(), allowed_roles.is_empty()) {
        (true, true) => {}
        (false, false) => {
            stage = stage.with_dynamic_role_binding(DynamicRoleBinding::new(
                ContextKey::from_role_from(role_from)?,
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

fn named<T>(
    value: String,
    constructor: impl Fn(String) -> Result<T, DomainError>,
) -> Result<Option<T>, DomainError> {
    if value.is_empty() {
        return Ok(None);
    }
    constructor(value).map(Some)
}
