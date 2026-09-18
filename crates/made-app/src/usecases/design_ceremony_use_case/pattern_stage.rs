use std::collections::BTreeMap;

use made_core::error::DomainError;
use made_core::value_objects::{
    CeremonyStepAggregation, ContextKey, ContextWrites, DynamicRoleBinding, MaxBounces,
    PriorContext, RoleId, Rounds, StateExecution, StepId, StepInstructions, StepOutputField,
};

use super::pattern_stage_routes::{
    human_handoff_route, output_route, string_route, two_output_route,
};
use crate::usecases::ceremony_design_route::CeremonyDesignRoute;
use crate::usecases::{
    CeremonyDesignDocument, CeremonyDesignGroup, CeremonyDesignGroupStep, CeremonyDesignJoin,
    CeremonyDesignPatternStage, CeremonyDesignStage, CeremonyDesignStageEntry,
    CeremonyStagePatternKind,
};

struct Expansion {
    entries: Vec<CeremonyDesignStageEntry>,
    routes: Vec<CeremonyDesignRoute>,
}

pub(super) fn materialize_stage_patterns(
    document: &CeremonyDesignDocument,
) -> Result<CeremonyDesignDocument, DomainError> {
    let mut entries = Vec::new();
    let mut routes = Vec::new();
    let mut patterns = BTreeMap::new();
    let mut bounce_cap: Option<u32> = document.max_bounces().map(MaxBounces::get);
    for entry in document.stage_entries() {
        match entry {
            CeremonyDesignStageEntry::Pattern(pattern) => {
                validate(pattern)?;
                let expansion = expand(pattern)?;
                for entry in &expansion.entries {
                    patterns.insert(entry_id(entry).clone(), pattern.kind());
                }
                if pattern.kind() == CeremonyStagePatternKind::Handoff {
                    let cap = pattern.max_iterations().expect("validated cap").get();
                    bounce_cap = Some(bounce_cap.map_or(cap, |current| current.min(cap)));
                }
                entries.extend(expansion.entries);
                routes.extend(expansion.routes);
            }
            other => entries.push(other.clone()),
        }
    }
    let mut materialized = document.materialized_with_entries(entries, patterns, routes);
    if let Some(cap) = bounce_cap {
        materialized = materialized.with_max_bounces(MaxBounces::new(cap)?);
    }
    Ok(materialized)
}

fn validate(pattern: &CeremonyDesignPatternStage) -> Result<(), DomainError> {
    if pattern.roles().is_empty() {
        return Err(invalid(format!(
            "stage pattern `{}` requires at least one role",
            pattern.id()
        )));
    }
    if pattern.kind().requires_cap() && pattern.max_iterations().is_none() {
        return Err(invalid(format!(
            "stage pattern `{}` requires `max_iterations`",
            pattern.id()
        )));
    }
    if pattern.kind() == CeremonyStagePatternKind::Handoff {
        let mut state_ids = std::collections::BTreeSet::new();
        for role in pattern.roles() {
            if !state_ids.insert(role.as_str().to_ascii_lowercase()) {
                return Err(invalid(format!(
                    "handoff pattern `{}` has roles that collide after state-id normalization",
                    pattern.id()
                )));
            }
        }
    }
    Ok(())
}

fn expand(pattern: &CeremonyDesignPatternStage) -> Result<Expansion, DomainError> {
    match pattern.kind() {
        CeremonyStagePatternKind::Sequential => linear(pattern, StateExecution::Sequential),
        CeremonyStagePatternKind::Concurrent => linear(pattern, StateExecution::Concurrent),
        CeremonyStagePatternKind::BroadcastCollect => broadcast_collect(pattern),
        CeremonyStagePatternKind::GroupChat => group_chat(pattern),
        CeremonyStagePatternKind::MakerChecker => maker_checker(pattern),
        CeremonyStagePatternKind::Handoff => handoff(pattern),
        CeremonyStagePatternKind::Magentic => magentic(pattern),
    }
}

fn linear(
    pattern: &CeremonyDesignPatternStage,
    execution: StateExecution,
) -> Result<Expansion, DomainError> {
    let steps = pattern
        .roles()
        .iter()
        .enumerate()
        .map(|(i, role)| {
            leaf(
                &format!("{}_{}", pattern.id(), i + 1),
                role,
                pattern.instructions(),
                i > 0,
            )
            .map(CeremonyDesignGroupStep::new)
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok(Expansion {
        entries: vec![CeremonyDesignStageEntry::Group(CeremonyDesignGroup::new(
            pattern.id().clone(),
            execution,
            steps,
            pattern.join(),
        ))],
        routes: Vec::new(),
    })
}

fn broadcast_collect(pattern: &CeremonyDesignPatternStage) -> Result<Expansion, DomainError> {
    let manager = manager(pattern)?;
    let steps = pattern
        .roles()
        .iter()
        .enumerate()
        .map(|(i, role)| {
            leaf(
                &format!("{}_review_{}", pattern.id(), i + 1),
                role,
                pattern.instructions(),
                false,
            )
            .map(CeremonyDesignGroupStep::new)
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok(Expansion {
        entries: vec![
            CeremonyDesignStageEntry::Group(CeremonyDesignGroup::new(
                id(pattern, "broadcast")?,
                StateExecution::Concurrent,
                steps,
                CeremonyDesignJoin::AllStepsCompleted,
            )),
            CeremonyDesignStageEntry::Leaf(
                leaf(
                    &format!("{}_collect", pattern.id()),
                    manager,
                    "Synthesize every independent contribution into one result.",
                    true,
                )?
                .with_aggregation(CeremonyStepAggregation::synthesize()),
            ),
        ],
        routes: Vec::new(),
    })
}

fn group_chat(pattern: &CeremonyDesignPatternStage) -> Result<Expansion, DomainError> {
    let manager = manager(pattern)?;
    let fallback = fallback(pattern)?;
    let cap = pattern.max_iterations().expect("validated cap").get();
    let key = ContextKey::new(format!("{}_next_speaker", pattern.id()))?;
    let record_id = id(pattern, "record")?;
    let fallback_id = id(pattern, "fallback")?;
    let mut entries = Vec::new();
    let mut routes = Vec::new();
    for iteration in 1..=cap {
        let manage_id = StepId::new(format!("{}_manage_{iteration}", pattern.id()))?;
        let select_id = StepId::new(format!("{}_select_{iteration}", pattern.id()))?;
        let speak_id = StepId::new(format!("{}_speak_{iteration}", pattern.id()))?;
        let manage = leaf(
            manage_id.as_str(),
            manager,
            "Decide whether the discussion is done and return done plus instructions.",
            true,
        )?;
        let select = leaf(
            select_id.as_str(),
            manager,
            "Choose the next speaker and return next_speaker.",
            true,
        )?
        .with_context_writes(ContextWrites::new(BTreeMap::from([(
            key.clone(),
            StepOutputField::new("next_speaker")?,
        )])));
        let speak = leaf(
            speak_id.as_str(),
            &pattern.roles()[0],
            pattern.instructions(),
            true,
        )?
        .with_dynamic_role_binding(DynamicRoleBinding::new(
            key.clone(),
            pattern.roles().iter().cloned(),
        )?);
        entries.push(CeremonyDesignStageEntry::Leaf(manage));
        entries.push(CeremonyDesignStageEntry::Leaf(select));
        entries.push(CeremonyDesignStageEntry::Leaf(speak));
        routes.push(output_route(
            &manage_id, &record_id, manager, &manage_id, &manage_id, "done", true, "finished",
        )?);
        routes.push(output_route(
            &manage_id, &select_id, manager, &manage_id, &manage_id, "done", false, "continue",
        )?);
    }
    entries.push(CeremonyDesignStageEntry::Leaf(leaf(
        fallback_id.as_str(),
        fallback,
        "Close a discussion that reached its turn cap with an explicit fallback.",
        true,
    )?));
    entries.push(CeremonyDesignStageEntry::Leaf(leaf(
        record_id.as_str(),
        manager,
        "Record the discussion outcome and minutes.",
        true,
    )?));
    Ok(Expansion { entries, routes })
}

fn maker_checker(pattern: &CeremonyDesignPatternStage) -> Result<Expansion, DomainError> {
    if pattern.roles().len() != 2 {
        return Err(invalid(format!(
            "maker-checker pattern `{}` requires exactly maker and checker roles",
            pattern.id()
        )));
    }
    let fallback = fallback(pattern)?;
    let cap = pattern.max_iterations().expect("validated cap").get();
    let deliver_id = id(pattern, "deliver")?;
    let fallback_id = id(pattern, "fallback")?;
    let mut entries = Vec::new();
    let mut routes = Vec::new();
    for iteration in 1..=cap {
        let state_id = StepId::new(format!("{}_iteration_{iteration}", pattern.id()))?;
        let make_id = StepId::new(format!("{}_make_{iteration}", pattern.id()))?;
        let check_id = StepId::new(format!("{}_check_{iteration}", pattern.id()))?;
        entries.push(group(
            state_id.clone(),
            vec![
                leaf(
                    make_id.as_str(),
                    &pattern.roles()[0],
                    pattern.instructions(),
                    true,
                )?,
                leaf(
                    check_id.as_str(),
                    &pattern.roles()[1],
                    "Check the result and return approved plus findings.",
                    true,
                )?,
            ],
        ));
        routes.push(output_route(
            &state_id,
            &deliver_id,
            &pattern.roles()[1],
            &check_id,
            &check_id,
            "approved",
            true,
            "approved",
        )?);
        let next = if iteration == cap {
            fallback_id.clone()
        } else {
            StepId::new(format!("{}_iteration_{}", pattern.id(), iteration + 1))?
        };
        routes.push(output_route(
            &state_id,
            &next,
            &pattern.roles()[1],
            &check_id,
            &check_id,
            "approved",
            false,
            "revise",
        )?);
    }
    entries.push(CeremonyDesignStageEntry::Leaf(leaf(
        fallback_id.as_str(),
        fallback,
        "Escalate work that reached the declared maker-checker cap.",
        true,
    )?));
    entries.push(CeremonyDesignStageEntry::Leaf(leaf(
        deliver_id.as_str(),
        &pattern.roles()[0],
        "Deliver the checked result, retaining any findings.",
        true,
    )?));
    Ok(Expansion { entries, routes })
}

fn handoff(pattern: &CeremonyDesignPatternStage) -> Result<Expansion, DomainError> {
    if pattern.roles().len() < 2 {
        return Err(invalid(format!(
            "handoff pattern `{}` requires at least two working roles",
            pattern.id()
        )));
    }
    let human = fallback(pattern)?;
    let human_id = id(pattern, "human")?;
    let record_id = id(pattern, "record")?;
    let role_states = pattern
        .roles()
        .iter()
        .map(|role| {
            Ok((
                role.clone(),
                StepId::new(format!(
                    "{}_{}",
                    pattern.id(),
                    role.as_str().to_ascii_lowercase()
                ))?,
            ))
        })
        .collect::<Result<Vec<_>, DomainError>>()?;
    let mut entries = Vec::new();
    let mut routes = Vec::new();
    for (role, state_id) in &role_states {
        entries.push(CeremonyDesignStageEntry::Leaf(leaf(
            state_id.as_str(),
            role,
            "Resolve the work or return resolved=false and handoff_to with the next role.",
            true,
        )?));
        routes.push(output_route(
            state_id, &record_id, role, state_id, state_id, "resolved", true, "resolved",
        )?);
        for (target_role, target_id) in &role_states {
            routes.push(string_route(
                state_id,
                target_id,
                role,
                state_id,
                "handoff_to",
                target_role.as_str(),
            )?);
        }
        routes.push(human_handoff_route(
            state_id,
            &human_id,
            role,
            state_id,
            "handoff_to",
            human.as_str(),
        )?);
    }
    entries.push(CeremonyDesignStageEntry::Leaf(leaf(
        human_id.as_str(),
        human,
        "Review the bounded handoff chain and decide its exit.",
        true,
    )?));
    entries.push(CeremonyDesignStageEntry::Leaf(leaf(
        record_id.as_str(),
        &pattern.roles()[0],
        "Record the resolved or human-approved handoff outcome.",
        true,
    )?));
    Ok(Expansion { entries, routes })
}

fn magentic(pattern: &CeremonyDesignPatternStage) -> Result<Expansion, DomainError> {
    let manager = manager(pattern)?;
    let fallback = fallback(pattern)?;
    let cap = pattern.max_iterations().expect("validated cap").get();
    let ledger_key = ContextKey::new(format!("{}_task_ledger", pattern.id()))?;
    let owner_key = ContextKey::new(format!("{}_next_owner", pattern.id()))?;
    let record_id = id(pattern, "record")?;
    let fallback_id = id(pattern, "fallback")?;
    let plan_id = id(pattern, "plan")?;
    let plan = leaf(
        plan_id.as_str(),
        manager,
        "Create a task ledger with tasks, owners, goals and status.",
        true,
    )?
    .with_context_writes(ContextWrites::new(BTreeMap::from([(
        ledger_key.clone(),
        StepOutputField::new("ledger")?,
    )])));
    let mut entries = vec![CeremonyDesignStageEntry::Leaf(plan)];
    let mut routes = Vec::new();
    for iteration in 1..=cap {
        let state_id = StepId::new(format!("{}_work_{iteration}", pattern.id()))?;
        let pick_id = StepId::new(format!("{}_pick_{iteration}", pattern.id()))?;
        let do_id = StepId::new(format!("{}_do_{iteration}", pattern.id()))?;
        let update_id = StepId::new(format!("{}_update_{iteration}", pattern.id()))?;
        let pick = leaf(
            pick_id.as_str(),
            manager,
            "Select the next open task and return its owner role.",
            true,
        )?
        .with_context_writes(ContextWrites::new(BTreeMap::from([(
            owner_key.clone(),
            StepOutputField::new("owner")?,
        )])));
        let do_task = leaf(
            do_id.as_str(),
            &pattern.roles()[0],
            pattern.instructions(),
            true,
        )?
        .with_dynamic_role_binding(DynamicRoleBinding::new(
            owner_key.clone(),
            pattern.roles().iter().cloned(),
        )?);
        let update = leaf(
            update_id.as_str(),
            manager,
            "Update the ledger and return ledger, done and stalled.",
            true,
        )?
        .with_context_writes(ContextWrites::new(BTreeMap::from([(
            ledger_key.clone(),
            StepOutputField::new("ledger")?,
        )])));
        entries.push(group(state_id.clone(), vec![pick, do_task, update]));
        routes.push(output_route(
            &state_id, &record_id, manager, &update_id, &update_id, "done", true, "done",
        )?);
        routes.push(output_route(
            &state_id,
            &fallback_id,
            manager,
            &update_id,
            &update_id,
            "stalled",
            true,
            "stalled",
        )?);
        let next = if iteration == cap {
            fallback_id.clone()
        } else {
            StepId::new(format!("{}_work_{}", pattern.id(), iteration + 1))?
        };
        routes.push(two_output_route(
            &state_id, &next, manager, &update_id, "done", false, "stalled", false, "continue",
        )?);
    }
    entries.push(CeremonyDesignStageEntry::Leaf(leaf(
        fallback_id.as_str(),
        fallback,
        "Review a stalled or capped task ledger.",
        true,
    )?));
    entries.push(CeremonyDesignStageEntry::Leaf(leaf(
        record_id.as_str(),
        manager,
        "Record the final ledger and its audit trail.",
        true,
    )?));
    Ok(Expansion { entries, routes })
}

fn group(id: StepId, steps: Vec<CeremonyDesignStage>) -> CeremonyDesignStageEntry {
    CeremonyDesignStageEntry::Group(CeremonyDesignGroup::new(
        id,
        StateExecution::Sequential,
        steps
            .into_iter()
            .map(CeremonyDesignGroupStep::new)
            .collect(),
        CeremonyDesignJoin::AllStepsCompleted,
    ))
}
fn leaf(
    id: &str,
    role: &RoleId,
    instructions: &str,
    see_prior: bool,
) -> Result<CeremonyDesignStage, DomainError> {
    Ok(CeremonyDesignStage::new(
        StepId::new(id)?,
        role.clone(),
        StepInstructions::new(instructions)?,
        None,
        Some(PriorContext::from_visible(see_prior)),
        None,
        Rounds::new(0)?,
        None,
    ))
}
fn manager(pattern: &CeremonyDesignPatternStage) -> Result<&RoleId, DomainError> {
    pattern.manager_role_id().ok_or_else(|| {
        invalid(format!(
            "stage pattern `{}` requires `manager_role_id`",
            pattern.id()
        ))
    })
}
fn fallback(pattern: &CeremonyDesignPatternStage) -> Result<&RoleId, DomainError> {
    pattern.fallback_role_id().ok_or_else(|| {
        invalid(format!(
            "stage pattern `{}` requires `fallback_role_id`",
            pattern.id()
        ))
    })
}
fn id(pattern: &CeremonyDesignPatternStage, suffix: &str) -> Result<StepId, DomainError> {
    StepId::new(format!("{}_{}", pattern.id(), suffix))
}
fn entry_id(entry: &CeremonyDesignStageEntry) -> &StepId {
    match entry {
        CeremonyDesignStageEntry::Leaf(stage) => stage.id(),
        CeremonyDesignStageEntry::Group(group) => group.id(),
        CeremonyDesignStageEntry::Pattern(pattern) => pattern.id(),
    }
}
fn invalid(reason: impl Into<String>) -> DomainError {
    DomainError::InvalidDocument {
        reason: reason.into(),
    }
}
