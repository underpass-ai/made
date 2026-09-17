use std::collections::{BTreeMap, BTreeSet};

use made_core::entities::CeremonyDefinitionDraft;
use made_core::error::DomainError;
use made_core::value_objects::{
    Attributes, CeremonyGuard, CeremonyInputDefinition, CeremonyOutputDefinition, CeremonyRole,
    CeremonyState, CeremonyStep, CeremonyTransition, CeremonyVersion, DurationMs, GuardCondition,
    GuardName, RepeatUntilCondition, RetryPolicy, RoleAction, StateId, StepAttempt,
    StepHandlerConfig, StepHandlerKind, StepRepeatPolicy, StepStatus, StepTimeout,
    TransitionTrigger,
};
use serde_json::{json, Value};

use super::{
    approval_guard_name, approval_trigger, completion_guard, num_agents, CeremonyDesignDocument,
    CeremonyDesignStage, COMPLETED_STATE, DEFAULT_BACKOFF_SECONDS, DEFAULT_HANDLER,
    DEFAULT_MAX_ATTEMPTS, DEFAULT_STEP_TIMEOUT_SECONDS, DEFAULT_VERSION,
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
        .stages()
        .iter()
        .map(|stage| StateId::new(stage.id().as_str().to_ascii_uppercase()))
        .collect::<Result<Vec<_>, _>>()?;
    let terminal = StateId::new(COMPLETED_STATE)?;
    let mut states = state_ids
        .iter()
        .enumerate()
        .map(|(index, id)| {
            if index == 0 {
                CeremonyState::initial(id.clone())
            } else {
                CeremonyState::intermediate(id.clone())
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
    for (index, stage) in document.stages().iter().enumerate() {
        let completion = GuardName::new(completion_guard(stage.id().as_str()))?;
        guards.push(CeremonyGuard::new(
            completion.clone(),
            GuardCondition::StepStatus {
                step_id: stage.id().clone(),
                status: StepStatus::Completed,
            },
        ));
        let (trigger, required_guards, owner) = match document.final_approval() {
            Some(approval) if index + 1 == document.stages().len() => {
                let human = GuardName::new(approval_guard_name(document))?;
                guards.push(CeremonyGuard::new(
                    human.clone(),
                    GuardCondition::HumanApproval,
                ));
                (
                    TransitionTrigger::new(approval_trigger(document))?,
                    vec![completion, human],
                    approval.role_id(),
                )
            }
            _ => (
                TransitionTrigger::new(completion.as_str())?,
                vec![completion],
                stage.owner_role_id(),
            ),
        };
        actions
            .get_mut(stage.owner_role_id())
            .expect("validated owner")
            .insert(RoleAction::step(stage.id().clone()));
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
        let handler = stage
            .handler()
            .cloned()
            .unwrap_or(StepHandlerKind::new(DEFAULT_HANDLER)?);
        let mut step = CeremonyStep::new(
            stage.id().clone(),
            state_ids[index].clone(),
            handler,
            StepHandlerConfig::new(Attributes::new(stage_config(stage, index))?),
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
        steps.push(step);
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
    Ok(CeremonyDefinitionDraft::new(
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
    ))
}

fn stage_config(stage: &CeremonyDesignStage, index: usize) -> BTreeMap<String, Value> {
    let mut config = BTreeMap::from([
        ("num_agents".to_owned(), json!(num_agents(stage))),
        ("prompt".to_owned(), json!(stage.instructions().trim())),
        (
            // Earlier stages are context by default for everything
            // after the first, which has nothing to see.
            "see_prior".to_owned(),
            json!(stage.prior_context().map_or(
                index > 0,
                made_core::value_objects::PriorContext::is_visible
            )),
        ),
    ]);
    if stage.review_rounds().get() > 0 {
        config.insert("rounds".to_owned(), json!(stage.review_rounds().get()));
    }
    config
}
