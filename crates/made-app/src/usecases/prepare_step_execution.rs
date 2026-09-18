use std::collections::BTreeMap;

use made_core::entities::CeremonyDefinition;
use made_core::error::DomainError;
use made_core::value_objects::{
    Attributes, CeremonyStep, CeremonyStepAggregation, CeremonyStepContribution,
    CeremonyTranscript, StateVisit, StepOutput, StepResult,
};
use serde_json::Value;

use super::PreparedStepExecution;

/// Apply a claimed step's optional aggregation policy before handler execution.
///
/// The caller must still durably finish the accepted claim for every returned
/// result or error. In particular, an error here follows the same fenced
/// `StepFailed` path as a handler failure.
pub fn prepare_step_execution(
    definition: &CeremonyDefinition,
    step: &CeremonyStep,
    destination_visit: StateVisit,
    transcript: CeremonyTranscript,
) -> Result<PreparedStepExecution, DomainError> {
    let Some(aggregation) = step.aggregation() else {
        return Ok(PreparedStepExecution::Handler { transcript });
    };
    let siblings = predecessor_siblings(definition, step, destination_visit, &transcript)?;
    match aggregation {
        CeremonyStepAggregation::Synthesize => Ok(PreparedStepExecution::Handler {
            transcript: CeremonyTranscript::new(siblings),
        }),
        CeremonyStepAggregation::Vote { output_field } => {
            let mut candidates: Vec<(Value, usize)> = Vec::new();
            for sibling in &siblings {
                let value = sibling
                    .output()
                    .attributes()
                    .get(output_field.as_str())
                    .ok_or_else(|| DomainError::InvalidDocument {
                        reason: format!(
                            "aggregate vote sibling `{}` has no `{output_field}` output field",
                            sibling.step_id()
                        ),
                    })?;
                if let Some((_, count)) = candidates
                    .iter_mut()
                    .find(|(candidate, _)| candidate == value)
                {
                    *count += 1;
                } else {
                    candidates.push((value.clone(), 1));
                }
            }
            let threshold = siblings.len() / 2;
            let winner = candidates
                .into_iter()
                .find(|(_, count)| *count > threshold)
                .map(|(value, _)| value)
                .ok_or_else(|| DomainError::InvalidDocument {
                    reason: format!("aggregate vote on `{output_field}` has no strict majority"),
                })?;
            let output = StepOutput::new(Attributes::new(BTreeMap::from([(
                output_field.as_str().to_owned(),
                winner,
            )]))?);
            Ok(PreparedStepExecution::Deterministic {
                result: StepResult::completed(output)?,
            })
        }
    }
}

fn predecessor_siblings(
    definition: &CeremonyDefinition,
    step: &CeremonyStep,
    destination_visit: StateVisit,
    transcript: &CeremonyTranscript,
) -> Result<Vec<CeremonyStepContribution>, DomainError> {
    let mut incoming = definition
        .transitions()
        .iter()
        .filter(|transition| transition.to() == step.state_id());
    let transition = incoming
        .next()
        .ok_or_else(|| DomainError::InvalidDocument {
            reason: format!(
                "aggregate step `{}` has no predecessor transition",
                step.id()
            ),
        })?;
    if incoming.next().is_some() {
        return Err(DomainError::InvalidDocument {
            reason: format!(
                "aggregate step `{}` has more than one predecessor transition",
                step.id()
            ),
        });
    }
    let predecessor_visit = destination_visit
        .get()
        .checked_sub(1)
        .and_then(|visit| StateVisit::new(visit).ok())
        .ok_or_else(|| DomainError::InvalidDocument {
            reason: format!(
                "aggregate step `{}` cannot read a predecessor from the first state visit",
                step.id()
            ),
        })?;
    let predecessor_steps = definition
        .steps_in_declaration_order()
        .filter(|candidate| candidate.state_id() == transition.from())
        .collect::<Vec<_>>();
    if predecessor_steps.is_empty() {
        return Err(DomainError::InvalidDocument {
            reason: format!(
                "aggregate step `{}` predecessor has no sibling steps",
                step.id()
            ),
        });
    }
    let latest_iteration = transcript
        .contributions()
        .iter()
        .filter(|contribution| {
            contribution.state_visit() == predecessor_visit
                && predecessor_steps
                    .iter()
                    .any(|candidate| candidate.id() == contribution.step_id())
        })
        .map(CeremonyStepContribution::state_iteration)
        .max()
        .ok_or_else(|| DomainError::InvalidDocument {
            reason: format!(
                "aggregate step `{}` has no outputs from its predecessor visit",
                step.id()
            ),
        })?;

    predecessor_steps
        .into_iter()
        .map(|sibling| {
            transcript
                .contributions()
                .iter()
                .rev()
                .find(|contribution| {
                    contribution.step_id() == sibling.id()
                        && contribution.state_visit() == predecessor_visit
                        && contribution.state_iteration() == latest_iteration
                })
                .cloned()
                .ok_or_else(|| DomainError::InvalidDocument {
                    reason: format!(
                        "aggregate step `{}` is missing sibling `{}` in predecessor visit {} iteration {}",
                        step.id(),
                        sibling.id(),
                        predecessor_visit.get(),
                        latest_iteration.get()
                    ),
                })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use made_core::value_objects::{
        CeremonyGuard, CeremonyName, CeremonyRole, CeremonyState, CeremonyTransition,
        CeremonyVersion, GuardCondition, GuardName, RetryPolicy, RoleAction, RoleId,
        StateIteration, StepHandlerConfig, StepHandlerKind, StepId, StepOutputField, StepStatus,
        TransitionTrigger,
    };
    use serde_json::json;

    fn id(raw: &str) -> StepId {
        StepId::new(raw).unwrap()
    }

    fn output(field: &str, value: Value) -> StepOutput {
        StepOutput::new(Attributes::new(BTreeMap::from([(field.to_owned(), value)])).unwrap())
    }

    fn contribution(
        step: &str,
        role: &str,
        visit: u32,
        iteration: u32,
        value: Value,
    ) -> CeremonyStepContribution {
        CeremonyStepContribution::at_state_iteration(
            id(step),
            StateIteration::new(iteration).unwrap(),
            RoleId::new(role).unwrap(),
            output("choice", value),
        )
        .with_state_visit(StateVisit::new(visit).unwrap())
    }

    fn guards(sibling_ids: &[StepId]) -> Vec<CeremonyGuard> {
        sibling_ids
            .iter()
            .map(|step_id| {
                CeremonyGuard::new(
                    GuardName::new(format!("{step_id}_done")).unwrap(),
                    GuardCondition::StepStatus {
                        step_id: step_id.clone(),
                        status: StepStatus::Completed,
                    },
                )
            })
            .chain(std::iter::once(CeremonyGuard::new(
                GuardName::new("aggregate_done").unwrap(),
                GuardCondition::StepStatus {
                    step_id: id("aggregate"),
                    status: StepStatus::Completed,
                },
            )))
            .collect()
    }

    fn roles(
        sibling_ids: &[StepId],
        review_trigger: &TransitionTrigger,
        finish_trigger: &TransitionTrigger,
    ) -> Vec<CeremonyRole> {
        ["ALPHA", "BETA", "GAMMA"]
            .into_iter()
            .zip(sibling_ids)
            .map(|(role, step_id)| {
                let mut actions = vec![RoleAction::step(step_id.clone())];
                if role == "ALPHA" {
                    actions.push(RoleAction::transition(review_trigger.clone()));
                }
                CeremonyRole::new(RoleId::new(role).unwrap(), actions).unwrap()
            })
            .chain(std::iter::once(
                CeremonyRole::new(
                    RoleId::new("DECIDER").unwrap(),
                    [
                        RoleAction::step(id("aggregate")),
                        RoleAction::transition(finish_trigger.clone()),
                    ],
                )
                .unwrap(),
            ))
            .collect()
    }

    fn definition(aggregation: CeremonyStepAggregation) -> CeremonyDefinition {
        let review = made_core::value_objects::StateId::new("review").unwrap();
        let decide = made_core::value_objects::StateId::new("decide").unwrap();
        let done = made_core::value_objects::StateId::new("done").unwrap();
        let handler = StepHandlerKind::new("host_callback").unwrap();
        let sibling = |raw: &str| {
            CeremonyStep::new(
                id(raw),
                review.clone(),
                handler.clone(),
                StepHandlerConfig::empty(),
                RetryPolicy::single_attempt(),
                None,
            )
        };
        let aggregate = CeremonyStep::new(
            id("aggregate"),
            decide.clone(),
            handler.clone(),
            StepHandlerConfig::new(
                Attributes::new(BTreeMap::from([("see_prior".to_owned(), json!(true))])).unwrap(),
            ),
            RetryPolicy::single_attempt(),
            None,
        )
        .with_aggregation(aggregation);
        let sibling_ids = [id("alpha"), id("beta"), id("gamma")];
        let review_trigger = TransitionTrigger::new("reviews_done").unwrap();
        let finish_trigger = TransitionTrigger::new("finish").unwrap();
        let guards = guards(&sibling_ids);
        let roles = roles(&sibling_ids, &review_trigger, &finish_trigger);
        CeremonyDefinition::new(
            CeremonyName::new("aggregate_review").unwrap(),
            CeremonyVersion::v1(),
            None,
            Vec::new(),
            Vec::new(),
            vec![
                CeremonyState::initial(review.clone())
                    .with_execution(made_core::value_objects::StateExecution::Concurrent),
                CeremonyState::intermediate(decide.clone()),
                CeremonyState::terminal(done.clone()),
            ],
            vec![
                CeremonyTransition::new(
                    review.clone(),
                    decide.clone(),
                    review_trigger,
                    sibling_ids
                        .iter()
                        .map(|step_id| GuardName::new(format!("{step_id}_done")).unwrap())
                        .collect::<Vec<_>>(),
                )
                .unwrap(),
                CeremonyTransition::new(
                    decide,
                    done,
                    finish_trigger,
                    vec![GuardName::new("aggregate_done").unwrap()],
                )
                .unwrap(),
            ],
            vec![
                sibling("alpha"),
                sibling("beta"),
                sibling("gamma"),
                aggregate,
            ],
            guards,
            roles,
        )
        .unwrap()
    }

    #[test]
    fn synthesize_receives_every_latest_sibling_in_declaration_order() {
        let definition = definition(CeremonyStepAggregation::synthesize());
        let transcript = CeremonyTranscript::new(vec![
            contribution("gamma", "GAMMA", 1, 1, json!("old")),
            contribution("alpha", "ALPHA", 1, 2, json!("ship")),
            contribution("gamma", "GAMMA", 1, 2, json!("hold")),
            contribution("beta", "BETA", 1, 2, json!("ship")),
            contribution("alpha", "ALPHA", 1, 1, json!("old")),
            contribution("beta", "BETA", 3, 1, json!("other visit")),
        ]);

        let PreparedStepExecution::Handler { transcript } = prepare_step_execution(
            &definition,
            definition.step(&id("aggregate")).unwrap(),
            StateVisit::new(2).unwrap(),
            transcript,
        )
        .unwrap() else {
            panic!("synthesize must invoke the existing handler")
        };

        assert_eq!(
            transcript
                .contributions()
                .iter()
                .map(|entry| entry.step_id().as_str())
                .collect::<Vec<_>>(),
            vec!["alpha", "beta", "gamma"]
        );
        assert!(transcript
            .contributions()
            .iter()
            .all(|entry| entry.state_iteration().get() == 2));
    }

    #[test]
    fn vote_is_order_independent_and_accepts_null_as_a_present_value() {
        let definition = definition(CeremonyStepAggregation::vote(
            StepOutputField::new("choice").unwrap(),
        ));
        for values in [
            vec![
                ("alpha", "ALPHA", Value::Null),
                ("beta", "BETA", json!("x")),
                ("gamma", "GAMMA", Value::Null),
            ],
            vec![
                ("gamma", "GAMMA", Value::Null),
                ("alpha", "ALPHA", Value::Null),
                ("beta", "BETA", json!("x")),
            ],
        ] {
            let transcript = CeremonyTranscript::new(
                values
                    .into_iter()
                    .map(|(step, role, value)| contribution(step, role, 1, 1, value))
                    .collect(),
            );
            let PreparedStepExecution::Deterministic { result } = prepare_step_execution(
                &definition,
                definition.step(&id("aggregate")).unwrap(),
                StateVisit::new(2).unwrap(),
                transcript,
            )
            .unwrap() else {
                panic!("vote must skip the handler")
            };
            assert_eq!(
                result.output().attributes().get("choice"),
                Some(&Value::Null)
            );
        }
    }

    #[test]
    fn vote_fails_closed_on_missing_field_or_no_strict_majority() {
        let definition = definition(CeremonyStepAggregation::vote(
            StepOutputField::new("choice").unwrap(),
        ));
        let aggregate = definition.step(&id("aggregate")).unwrap();
        let no_majority = CeremonyTranscript::new(vec![
            contribution("alpha", "ALPHA", 1, 1, json!("a")),
            contribution("beta", "BETA", 1, 1, json!("b")),
            contribution("gamma", "GAMMA", 1, 1, json!("c")),
        ]);
        assert!(prepare_step_execution(
            &definition,
            aggregate,
            StateVisit::new(2).unwrap(),
            no_majority
        )
        .is_err());

        let missing = CeremonyTranscript::new(vec![
            contribution("alpha", "ALPHA", 1, 1, json!("a")),
            contribution("beta", "BETA", 1, 1, json!("a")),
            CeremonyStepContribution::new(
                id("gamma"),
                RoleId::new("GAMMA").unwrap(),
                StepOutput::empty(),
            ),
        ]);
        assert!(prepare_step_execution(
            &definition,
            aggregate,
            StateVisit::new(2).unwrap(),
            missing
        )
        .is_err());
    }
}
