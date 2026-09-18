use std::collections::BTreeMap;
use std::sync::Arc;

use made_core::entities::{CeremonyDefinition, CeremonyEvent};
use made_core::value_objects::{
    Attributes, AuditActorKind, CeremonyContext, CeremonyGuard, CeremonyName, CeremonyRole,
    CeremonyState, CeremonyStep, CeremonyStepAggregation, CeremonyTransition, CeremonyVersion,
    GuardCondition, GuardName, JoinStepCount, MaxParallel, RetryPolicy, RoleAction, RoleId,
    StateExecution, StateId, StepHandlerConfig, StepHandlerKind, StepId, StepOutput,
    StepOutputField, StepResult, StepStatus, TransitionTrigger,
};
use serde_json::{json, Value};

use super::*;
use crate::usecases::ceremony_test_support::{
    ceremony_id, definition_resolver, idempotency_key, lease_owner, lease_ttl, now,
    started_instance, stream, stream_over, DefinitionRepositoryFake, EventStoreFake, FixedClock,
    SequenceStepHandlerFake,
};
use crate::usecases::{
    ApplyCeremonyTransitionInput, ApplyCeremonyTransitionUseCase, RunCeremonyStepInput,
    RunCeremonyStepUseCase,
};

fn step_id(raw: &str) -> StepId {
    StepId::new(raw).unwrap()
}

fn role_id(raw: &str) -> RoleId {
    RoleId::new(raw).unwrap()
}

fn choice(value: Value) -> StepResult {
    StepResult::completed(StepOutput::new(
        Attributes::new(BTreeMap::from([("choice".to_owned(), value)])).unwrap(),
    ))
    .unwrap()
}

fn aggregation_guards(sibling_ids: &[StepId], counted_join: bool) -> Vec<CeremonyGuard> {
    let mut guards = sibling_ids
        .iter()
        .map(|id| {
            CeremonyGuard::new(
                GuardName::new(format!("{id}_done")).unwrap(),
                GuardCondition::StepStatus {
                    step_id: id.clone(),
                    status: StepStatus::Completed,
                },
            )
        })
        .collect::<Vec<_>>();
    if counted_join {
        guards = vec![CeremonyGuard::new(
            GuardName::new("all_reviews_done").unwrap(),
            GuardCondition::StepsCompleted(
                JoinStepCount::new(u32::try_from(sibling_ids.len()).unwrap()).unwrap(),
            ),
        )];
    }
    guards.push(CeremonyGuard::new(
        GuardName::new("aggregate_done").unwrap(),
        GuardCondition::StepStatus {
            step_id: step_id("aggregate"),
            status: StepStatus::Completed,
        },
    ));
    guards
}

fn aggregation_roles(
    sibling_ids: &[StepId],
    review_trigger: &TransitionTrigger,
    finish_trigger: &TransitionTrigger,
) -> Vec<CeremonyRole> {
    ["ALPHA", "BETA", "GAMMA"]
        .into_iter()
        .zip(sibling_ids)
        .map(|(role, id)| {
            let mut actions = vec![RoleAction::step(id.clone())];
            if role == "ALPHA" {
                actions.push(RoleAction::transition(review_trigger.clone()));
            }
            CeremonyRole::new(role_id(role), actions).unwrap()
        })
        .chain(std::iter::once(
            CeremonyRole::new(
                role_id("DECIDER"),
                [
                    RoleAction::step(step_id("aggregate")),
                    RoleAction::transition(finish_trigger.clone()),
                ],
            )
            .unwrap(),
        ))
        .collect()
}

fn aggregation_definition(aggregation: CeremonyStepAggregation) -> CeremonyDefinition {
    aggregation_definition_with_join(aggregation, false)
}

fn aggregation_definition_with_join(
    aggregation: CeremonyStepAggregation,
    counted_join: bool,
) -> CeremonyDefinition {
    let review = StateId::new("review").unwrap();
    let decide = StateId::new("decide").unwrap();
    let done = StateId::new("done").unwrap();
    let sibling_ids = [step_id("alpha"), step_id("beta"), step_id("gamma")];
    let handler = StepHandlerKind::new("controlled").unwrap();
    let sibling = |id: &StepId| {
        CeremonyStep::new(
            id.clone(),
            review.clone(),
            handler.clone(),
            StepHandlerConfig::empty(),
            RetryPolicy::single_attempt(),
            None,
        )
    };
    let aggregate = CeremonyStep::new(
        step_id("aggregate"),
        decide.clone(),
        handler.clone(),
        StepHandlerConfig::new(
            Attributes::new(BTreeMap::from([("see_prior".to_owned(), json!(true))])).unwrap(),
        ),
        RetryPolicy::single_attempt(),
        None,
    )
    .with_aggregation(aggregation);
    let review_trigger = TransitionTrigger::new("reviews_done").unwrap();
    let finish_trigger = TransitionTrigger::new("finish").unwrap();
    let guards = aggregation_guards(&sibling_ids, counted_join);
    let roles = aggregation_roles(&sibling_ids, &review_trigger, &finish_trigger);
    CeremonyDefinition::new(
        CeremonyName::new("aggregate_review").unwrap(),
        CeremonyVersion::v1(),
        None,
        Vec::new(),
        Vec::new(),
        [
            CeremonyState::initial(review.clone()).with_execution(StateExecution::Concurrent),
            CeremonyState::intermediate(decide.clone()),
            CeremonyState::terminal(done.clone()),
        ],
        [
            CeremonyTransition::new(
                review.clone(),
                decide,
                review_trigger,
                if counted_join {
                    vec![GuardName::new("all_reviews_done").unwrap()]
                } else {
                    sibling_ids
                        .iter()
                        .map(|id| GuardName::new(format!("{id}_done")).unwrap())
                        .collect()
                },
            )
            .unwrap(),
            CeremonyTransition::new(
                StateId::new("decide").unwrap(),
                done,
                finish_trigger,
                [GuardName::new("aggregate_done").unwrap()],
            )
            .unwrap(),
        ],
        sibling_ids
            .iter()
            .map(sibling)
            .chain(std::iter::once(aggregate)),
        guards,
        roles,
    )
    .unwrap()
    .with_max_parallel(MaxParallel::new(3).unwrap())
}

async fn run(
    definition: CeremonyDefinition,
    handler: Arc<SequenceStepHandlerFake>,
) -> (
    Arc<EventStoreFake>,
    Result<RunCeremonyOutput, made_core::error::DomainError>,
) {
    let store = Arc::new(EventStoreFake::default());
    let (stream, _) = stream_over(store.clone());
    let usecase = RunCeremonyUseCase::new(
        Arc::new(DefinitionRepositoryFake::new(definition.clone())),
        stream,
        handler,
        Arc::new(FixedClock::new(now())),
    );
    let result = usecase
        .execute(RunCeremonyInput::new(
            ceremony_id(),
            definition,
            CeremonyContext::empty(),
            lease_owner(),
            lease_ttl(),
            "aggregation-test",
            AuditActorKind::Agent,
        ))
        .await;
    (store, result)
}

#[tokio::test]
async fn counted_all_sibling_join_votes_without_invoking_the_aggregate_handler() {
    let definition = aggregation_definition_with_join(
        CeremonyStepAggregation::vote(StepOutputField::new("choice").unwrap()),
        true,
    );
    assert!(definition.analyze().errors().next().is_none());
    let handler = Arc::new(SequenceStepHandlerFake::new([
        choice(json!("ship")),
        choice(json!("hold")),
        choice(json!("ship")),
    ]));

    let (store, output) = run(definition.clone(), handler.clone()).await;
    let output = output.unwrap();

    assert!(output.instance().is_completed(&definition));
    assert_eq!(handler.requests().await.len(), 3);
    assert_eq!(
        output
            .instance()
            .step_record(&step_id("aggregate"))
            .unwrap()
            .output()
            .attributes()
            .get("choice"),
        Some(&json!("ship"))
    );
    let reopened = store.saved(&ceremony_id()).await;
    assert_eq!(
        reopened
            .step_record(&step_id("aggregate"))
            .unwrap()
            .status(),
        StepStatus::Completed
    );
}

#[tokio::test]
async fn synthesize_handler_receives_every_sibling_in_declaration_order() {
    let definition = aggregation_definition(CeremonyStepAggregation::synthesize());
    let handler = Arc::new(SequenceStepHandlerFake::new([
        choice(json!("alpha output")),
        choice(json!("beta output")),
        choice(json!("gamma output")),
        choice(json!("summary")),
    ]));

    let (_, output) = run(definition.clone(), handler.clone()).await;
    assert!(output.unwrap().instance().is_completed(&definition));
    let requests = handler.requests().await;
    assert_eq!(requests.len(), 4);
    let synthesis = requests.last().unwrap();
    assert_eq!(synthesis.step_id(), &step_id("aggregate"));
    assert_eq!(
        synthesis
            .transcript()
            .contributions()
            .iter()
            .map(|contribution| contribution.step_id().as_str())
            .collect::<Vec<_>>(),
        ["alpha", "beta", "gamma"]
    );
    assert!(synthesis
        .transcript()
        .contributions()
        .iter()
        .all(|contribution| contribution.output().attributes().get("choice").is_some()));
}

#[tokio::test]
async fn invalid_vote_is_durably_failed_after_claim_without_handler_invocation() {
    let definition = aggregation_definition(CeremonyStepAggregation::vote(
        StepOutputField::new("choice").unwrap(),
    ));
    let handler = Arc::new(SequenceStepHandlerFake::new([
        choice(json!("a")),
        choice(json!("b")),
        choice(json!("c")),
    ]));

    let (store, output) = run(definition, handler.clone()).await;
    assert!(output.unwrap_err().to_string().contains("did not complete"));
    assert_eq!(handler.requests().await.len(), 3);
    let reopened = store.saved(&ceremony_id()).await;
    assert_eq!(
        reopened
            .step_record(&step_id("aggregate"))
            .unwrap()
            .status(),
        StepStatus::Failed
    );
    assert!(store.records(&ceremony_id()).await.iter().any(|record| {
        matches!(
            record.event(),
            Some(CeremonyEvent::StepFailed(failed))
                if failed.step_id == step_id("aggregate")
        )
    }));
}

#[tokio::test]
async fn explicit_step_runner_uses_the_same_deterministic_vote_and_reopens_it() {
    let definition = aggregation_definition(CeremonyStepAggregation::vote(
        StepOutputField::new("choice").unwrap(),
    ));
    let definitions = Arc::new(DefinitionRepositoryFake::new(definition.clone()));
    let store = Arc::new(EventStoreFake::default());
    store.save(&started_instance(&definition)).await.unwrap();
    let session_stream = stream(store.clone());
    let resolver = definition_resolver(definitions);
    let handler = Arc::new(SequenceStepHandlerFake::new([
        choice(json!("ship")),
        choice(json!("ship")),
        choice(json!("hold")),
    ]));
    let clock = Arc::new(FixedClock::new(now()));
    let runner = RunCeremonyStepUseCase::new(
        resolver.clone(),
        session_stream.clone(),
        handler.clone(),
        clock.clone(),
    );

    for (id, role) in [("alpha", "ALPHA"), ("beta", "BETA"), ("gamma", "GAMMA")] {
        runner
            .execute(RunCeremonyStepInput::new(
                ceremony_id(),
                role_id(role),
                AuditActorKind::Agent,
                step_id(id),
                lease_owner(),
                idempotency_key(&format!("run-{id}")),
                lease_ttl(),
            ))
            .await
            .unwrap();
    }
    ApplyCeremonyTransitionUseCase::new(resolver, session_stream, clock)
        .execute(ApplyCeremonyTransitionInput::new(
            ceremony_id(),
            role_id("ALPHA"),
            AuditActorKind::Agent,
            TransitionTrigger::new("reviews_done").unwrap(),
        ))
        .await
        .unwrap();

    let aggregate = runner
        .execute(RunCeremonyStepInput::new(
            ceremony_id(),
            role_id("DECIDER"),
            AuditActorKind::Agent,
            step_id("aggregate"),
            lease_owner(),
            idempotency_key("run-aggregate"),
            lease_ttl(),
        ))
        .await
        .unwrap();

    assert_eq!(handler.requests().await.len(), 3);
    assert_eq!(
        aggregate.result().output().attributes().get("choice"),
        Some(&json!("ship"))
    );
    assert_eq!(
        store
            .saved(&ceremony_id())
            .await
            .step_record(&step_id("aggregate"))
            .unwrap()
            .status(),
        StepStatus::Completed
    );
}
