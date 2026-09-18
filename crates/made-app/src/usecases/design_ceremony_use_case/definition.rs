use std::collections::{BTreeMap, BTreeSet};

use made_core::entities::CeremonyDefinitionDraft;
use made_core::error::DomainError;
use made_core::value_objects::{
    Attributes, CeremonyGuard, CeremonyInputDefinition, CeremonyOutputDefinition, CeremonyRole,
    CeremonyState, CeremonyStep, CeremonyTransition, CeremonyVersion, DurationMs, GuardCondition,
    GuardName, OutputFieldGuardCondition, RepeatUntilCondition, RetryPolicy, RoleAction,
    StateExecution, StateId, StateRepeatPolicy, StateRepeatUntilCondition, StepAttempt,
    StepHandlerConfig, StepHandlerKind, StepRepeatExhaustedGuardCondition, StepRepeatPolicy,
    StepStatus, StepTimeout, TransitionTrigger,
};
use serde_json::json;

use super::{
    approval_guard_name, approval_trigger, completion_guard, exit_guard_name,
    stage_config::stage_config, CeremonyDesignDocument, CeremonyDesignStage, COMPLETED_STATE,
    DEFAULT_BACKOFF_SECONDS, DEFAULT_HANDLER, DEFAULT_MAX_ATTEMPTS, DEFAULT_STEP_TIMEOUT_SECONDS,
    DEFAULT_VERSION,
};
use crate::usecases::{
    CeremonyDesignExitGuard, CeremonyDesignGroupStep, CeremonyDesignJoin, CeremonyDesignStageEntry,
};

/// The linear topology, assembled atomically from one intent: one state
/// per stage plus the terminal one, one automated completion guard per
/// stage, one transition out of each, and the final approval's human
/// guard where the author asked for it.
#[allow(clippy::too_many_lines)] // The linear topology is assembled atomically from one intent.
pub(super) fn build_definition(
    document: &CeremonyDesignDocument,
) -> Result<CeremonyDefinitionDraft, DomainError> {
    let mut actions = document
        .participants()
        .iter()
        .map(|participant| {
            let capabilities = participant
                .capabilities()
                .iter()
                .map(|capability| {
                    RoleAction::from_capability_label(capability.as_action())
                        .expect("known capability")
                })
                .collect::<BTreeSet<_>>();
            (participant.role_id().clone(), capabilities)
        })
        .collect::<BTreeMap<_, _>>();
    let state_ids = document
        .stage_entries()
        .iter()
        .map(|entry| StateId::new(entry_id(entry).as_str().to_ascii_uppercase()))
        .collect::<Result<Vec<_>, _>>()?;
    let terminal = StateId::new(COMPLETED_STATE)?;
    let mut states = state_ids
        .iter()
        .enumerate()
        .map(|(index, id)| {
            let state = if index == 0 {
                CeremonyState::initial(id.clone())
            } else {
                CeremonyState::intermediate(id.clone())
            };
            let state = state.with_execution(entry_execution(&document.stage_entries()[index]));
            let state = document
                .state_pattern(entry_id(&document.stage_entries()[index]))
                .map_or(state.clone(), |pattern| {
                    state.with_annotations(
                        Attributes::new(BTreeMap::from([(
                            "x-pattern".to_owned(),
                            json!(pattern.id()),
                        )]))
                        .expect("one fixed nonblank annotation"),
                    )
                });
            match &document.stage_entries()[index] {
                CeremonyDesignStageEntry::Group(group) => {
                    group.repeat().map_or(state.clone(), |repeat| {
                        state.with_repeat_policy(StateRepeatPolicy::new(
                            repeat.max_iterations(),
                            StateRepeatUntilCondition::new(
                                repeat.until().step_id().clone(),
                                repeat.until().output_field().clone(),
                                repeat.until().equals().clone(),
                            ),
                        ))
                    })
                }
                CeremonyDesignStageEntry::Leaf(_) => state,
                CeremonyDesignStageEntry::Pattern(_) => unreachable!("patterns are materialized"),
            }
        })
        .collect::<Vec<_>>();
    states.push(CeremonyState::terminal(terminal.clone()));
    let retry = RetryPolicy::new(
        document
            .max_attempts()
            .unwrap_or(StepAttempt::new(DEFAULT_MAX_ATTEMPTS)?),
        document.retry_backoff().unwrap_or(DurationMs::from_millis(
            DEFAULT_BACKOFF_SECONDS.saturating_mul(1_000),
        )),
    );
    let timeout = document
        .step_timeout()
        .unwrap_or(StepTimeout::new(DurationMs::from_millis(
            DEFAULT_STEP_TIMEOUT_SECONDS.saturating_mul(1_000),
        ))?);
    let mut guards = Vec::new();
    let mut transitions = Vec::new();
    let mut steps = Vec::new();
    for (index, entry) in document.stage_entries().iter().enumerate() {
        let entry_stages = entry_steps(entry);
        let entry_name = entry_id(entry).as_str();
        let completion = GuardName::new(completion_guard(entry_name))?;
        let mut required_guards = Vec::new();
        match entry {
            CeremonyDesignStageEntry::Leaf(stage) => {
                guards.push(CeremonyGuard::new(
                    completion.clone(),
                    GuardCondition::StepStatus {
                        step_id: stage.id().clone(),
                        status: StepStatus::Completed,
                    },
                ));
                required_guards.push(completion.clone());
            }
            CeremonyDesignStageEntry::Group(group) => match group.join() {
                CeremonyDesignJoin::AllStepsCompleted => {
                    for stage in &entry_stages {
                        let name =
                            GuardName::new(format!("{}_{}_completed", entry_name, stage.id()))?;
                        guards.push(CeremonyGuard::new(
                            name.clone(),
                            GuardCondition::StepStatus {
                                step_id: stage.id().clone(),
                                status: StepStatus::Completed,
                            },
                        ));
                        required_guards.push(name);
                    }
                }
                CeremonyDesignJoin::AnyStepCompleted => {
                    guards.push(CeremonyGuard::new(
                        completion.clone(),
                        GuardCondition::AnyStepCompleted,
                    ));
                    required_guards.push(completion.clone());
                }
                CeremonyDesignJoin::StepsCompleted(count) => {
                    guards.push(CeremonyGuard::new(
                        completion.clone(),
                        GuardCondition::StepsCompleted(count),
                    ));
                    required_guards.push(completion.clone());
                }
            },
            CeremonyDesignStageEntry::Pattern(_) => unreachable!("patterns are materialized"),
        }
        if let CeremonyDesignStageEntry::Leaf(stage) = entry {
            for (guard_index, guard) in stage.exit_guards().iter().enumerate() {
                let name = GuardName::new(exit_guard_name(stage.id().as_str(), guard_index))?;
                let condition = match guard {
                    CeremonyDesignExitGuard::OutputField(guard) => {
                        GuardCondition::OutputField(OutputFieldGuardCondition::new(
                            guard.step_id().clone(),
                            guard.output_field().clone(),
                            guard.expected().clone(),
                        ))
                    }
                    CeremonyDesignExitGuard::StepRepeatExhausted(guard) => {
                        GuardCondition::StepRepeatExhausted(StepRepeatExhaustedGuardCondition::new(
                            guard.step_id().clone(),
                        ))
                    }
                    CeremonyDesignExitGuard::ChildrenCompleted(condition) => {
                        GuardCondition::ChildrenCompleted(condition.clone())
                    }
                };
                guards.push(CeremonyGuard::new(name.clone(), condition));
                required_guards.push(name);
            }
        }
        let first_owner = entry_stages
            .first()
            .expect("validated non-empty group")
            .owner_role_id();
        let (trigger, owner) = match document.final_approval() {
            Some(approval) if index + 1 == document.stage_entries().len() => {
                let human = GuardName::new(approval_guard_name(document))?;
                guards.push(CeremonyGuard::new(
                    human.clone(),
                    GuardCondition::HumanApproval,
                ));
                required_guards.push(human);
                (
                    TransitionTrigger::new(approval_trigger(document))?,
                    match entry {
                        CeremonyDesignStageEntry::Group(_) => first_owner,
                        CeremonyDesignStageEntry::Leaf(_) => approval.role_id(),
                        CeremonyDesignStageEntry::Pattern(_) => {
                            unreachable!("patterns are materialized")
                        }
                    },
                )
            }
            _ => (TransitionTrigger::new(completion.as_str())?, first_owner),
        };
        for stage in &entry_stages {
            if let Some(binding) = stage.dynamic_role_binding() {
                for role_id in binding.allowed_roles() {
                    actions
                        .get_mut(role_id)
                        .expect("validated dynamic role")
                        .insert(RoleAction::step(stage.id().clone()));
                }
            } else {
                actions
                    .get_mut(stage.owner_role_id())
                    .expect("validated owner")
                    .insert(RoleAction::step(stage.id().clone()));
            }
        }
        if !document
            .routes()
            .iter()
            .any(|route| route.from() == entry_id(entry))
        {
            actions
                .get_mut(owner)
                .expect("validated transition owner")
                .insert(RoleAction::transition(trigger.clone()));
            transitions.push(CeremonyTransition::new(
                state_ids[index].clone(),
                state_ids.get(index + 1).unwrap_or(&terminal).clone(),
                trigger,
                required_guards,
            )?);
        }
        for stage in entry_stages {
            let handler = stage
                .handler()
                .cloned()
                .unwrap_or(StepHandlerKind::new(DEFAULT_HANDLER)?);
            let mut step = CeremonyStep::new(
                stage.id().clone(),
                state_ids[index].clone(),
                handler,
                StepHandlerConfig::new(Attributes::new(stage_config(
                    document,
                    entry_id(entry),
                    stage,
                    index,
                ))?),
                retry,
                Some(timeout),
            );
            if let Some(repeat) = stage.repeat() {
                step = step.with_repeat_policy(StepRepeatPolicy::new(
                    RepeatUntilCondition::output_field_equals(
                        repeat.output_field().clone(),
                        repeat.equals().clone(),
                    ),
                    repeat.max_iterations(),
                ));
            }
            if let Some(binding) = stage.dynamic_role_binding() {
                step = step.with_dynamic_role_binding(binding.clone());
            }
            step = step.with_context_writes(stage.context_writes().clone());
            if let Some(aggregation) = stage.aggregation() {
                step = step.with_aggregation(aggregation.clone());
            }
            if let Some(spawn) = stage.spawn() {
                step = step.with_spawn(spawn.clone());
            }
            steps.push(step);
        }
    }
    for route in document.routes() {
        let required_guards = route
            .guards()
            .iter()
            .map(|(name, condition)| {
                guards.push(CeremonyGuard::new(name.clone(), condition.clone()));
                name.clone()
            })
            .collect::<Vec<_>>();
        actions
            .get_mut(route.owner())
            .expect("validated pattern route owner")
            .insert(RoleAction::transition(route.trigger().clone()));
        transitions.push(CeremonyTransition::new(
            StateId::new(route.from().as_str().to_ascii_uppercase())?,
            StateId::new(route.to().as_str().to_ascii_uppercase())?,
            route.trigger().clone(),
            required_guards,
        )?);
    }
    let roles = document
        .participants()
        .iter()
        .map(|participant| {
            CeremonyRole::new(
                participant.role_id().clone(),
                actions
                    .remove(participant.role_id())
                    .expect("validated role"),
            )
        })
        .collect::<Result<Vec<_>, _>>()?;
    let inputs = document
        .required_inputs()
        .iter()
        .cloned()
        .map(CeremonyInputDefinition::required)
        .chain(
            document
                .optional_inputs()
                .iter()
                .cloned()
                .map(CeremonyInputDefinition::optional),
        );
    let mut draft = CeremonyDefinitionDraft::new(
        document.name().clone(),
        document
            .version()
            .cloned()
            .unwrap_or(CeremonyVersion::new(DEFAULT_VERSION)?),
        Some(document.objective().clone()),
        inputs,
        document
            .outputs()
            .iter()
            .cloned()
            .map(CeremonyOutputDefinition::new),
        states,
        transitions,
        steps,
        guards,
        roles,
    )
    .with_max_parallel(document.max_parallel());
    if let Some(limit) = document.max_transitions() {
        draft = draft.with_max_transitions(limit);
    }
    if let Some(limit) = document.max_bounces() {
        draft = draft.with_max_bounces(limit);
    }
    Ok(draft)
}

fn entry_id(entry: &CeremonyDesignStageEntry) -> &made_core::value_objects::StepId {
    match entry {
        CeremonyDesignStageEntry::Leaf(stage) => stage.id(),
        CeremonyDesignStageEntry::Group(group) => group.id(),
        CeremonyDesignStageEntry::Pattern(_) => unreachable!("patterns are materialized"),
    }
}

fn entry_execution(entry: &CeremonyDesignStageEntry) -> StateExecution {
    match entry {
        CeremonyDesignStageEntry::Leaf(_) => StateExecution::Sequential,
        CeremonyDesignStageEntry::Group(group) => group.execution(),
        CeremonyDesignStageEntry::Pattern(_) => unreachable!("patterns are materialized"),
    }
}

fn entry_steps(entry: &CeremonyDesignStageEntry) -> Vec<&CeremonyDesignStage> {
    match entry {
        CeremonyDesignStageEntry::Leaf(stage) => vec![stage],
        CeremonyDesignStageEntry::Group(group) => group
            .steps()
            .iter()
            .map(CeremonyDesignGroupStep::step)
            .collect(),
        CeremonyDesignStageEntry::Pattern(_) => unreachable!("patterns are materialized"),
    }
}
