use std::sync::Arc;

use made_core::entities::CeremonyDefinition;
use made_core::value_objects::{
    Attributes, AuditActorKind, DurationMs, StepId, StepOutput, StepResult,
};

use super::ceremony_test_support::{
    ceremony_id, context_writing_definition, definition_resolver, idempotency_key, lease_owner,
    now, role_id, started_instance, state_repeating_definition, step_id, stream,
    DefinitionRepositoryFake, EventStoreFake, FixedClock,
};
use super::{
    CompleteCeremonyStepInput, CompleteCeremonyStepUseCase, StartCeremonyStepInput,
    StartCeremonyStepUseCase,
};

struct Fixture {
    store: Arc<EventStoreFake>,
    claim: StartCeremonyStepUseCase,
    complete: CompleteCeremonyStepUseCase,
}

impl Fixture {
    async fn new(definition: CeremonyDefinition) -> Self {
        let store = Arc::new(EventStoreFake::default());
        store.save(&started_instance(&definition)).await.unwrap();
        let resolver = definition_resolver(Arc::new(DefinitionRepositoryFake::new(definition)));
        let clock = Arc::new(FixedClock::new(now()));
        Self {
            claim: StartCeremonyStepUseCase::new(
                resolver.clone(),
                stream(store.clone()),
                clock.clone(),
            ),
            complete: CompleteCeremonyStepUseCase::new(resolver, stream(store.clone()), clock),
            store,
        }
    }

    async fn claim(
        &self,
        step: StepId,
        key: &str,
        result: StepResult,
    ) -> CompleteCeremonyStepInput {
        let claim = self
            .claim
            .execute(StartCeremonyStepInput::new(
                ceremony_id(),
                role_id(),
                AuditActorKind::Agent,
                step.clone(),
                lease_owner(),
                idempotency_key(key),
                DurationMs::from_millis(60_000),
            ))
            .await
            .unwrap();
        CompleteCeremonyStepInput::new(
            ceremony_id(),
            step,
            result,
            AuditActorKind::Agent,
            claim.claim_fence().clone(),
        )
    }
}

fn result(key: &str, value: serde_json::Value) -> StepResult {
    StepResult::completed(StepOutput::new(
        Attributes::new([(key.to_owned(), value)].into_iter().collect()).unwrap(),
    ))
    .unwrap()
}

#[tokio::test]
async fn completion_retry_recovers_context_and_refuses_changed_result_or_actor() {
    let f = Fixture::new(context_writing_definition()).await;
    let input = f
        .claim(step_id(), "first", result("summary", "accepted".into()))
        .await;
    let accepted = f.complete.execute(input.clone()).await.unwrap();
    let records = f.store.records(&ceremony_id()).await;
    assert_eq!(f.complete.execute(input.clone()).await.unwrap(), accepted);
    let mut changed = input.clone();
    changed.result = result("summary", "changed".into());
    assert!(f.complete.execute(changed).await.is_err());
    let mut changed = input;
    changed.actor_kind = AuditActorKind::Human;
    assert!(f.complete.execute(changed).await.is_err());
    assert_eq!(f.store.records(&ceremony_id()).await, records);
}

#[tokio::test]
async fn failed_completion_retry_returns_original_response_after_a_new_claim() {
    let f = Fixture::new(context_writing_definition()).await;
    let input = f
        .claim(
            step_id(),
            "first",
            StepResult::failed(
                made_core::value_objects::StepErrorMessage::new("provider unavailable").unwrap(),
            )
            .unwrap(),
        )
        .await;
    let accepted = f.complete.execute(input.clone()).await.unwrap();
    let next = f
        .claim(step_id(), "second", result("summary", "new".into()))
        .await;
    assert_ne!(input.claim_fence, next.claim_fence);
    let records = f.store.records(&ceremony_id()).await;
    assert_eq!(f.complete.execute(input.clone()).await.unwrap(), accepted);
    let mut changed = input;
    changed.result = next.result.clone();
    assert!(f.complete.execute(changed).await.is_err());
    assert_eq!(f.store.records(&ceremony_id()).await, records);
    f.complete.execute(next).await.unwrap();
}

#[tokio::test]
async fn completion_retry_includes_reopened_state_and_excludes_later_claims() {
    let f = Fixture::new(state_repeating_definition(3)).await;
    let open = StepId::new("open").unwrap();
    let check = StepId::new("check").unwrap();
    let first = f
        .claim(open.clone(), "open-1", result("ready", false.into()))
        .await;
    f.complete.execute(first).await.unwrap();
    let input = f
        .claim(check, "check-1", result("ready", false.into()))
        .await;
    let accepted = f.complete.execute(input.clone()).await.unwrap();
    assert_eq!(accepted.current_state_iteration().get(), 2);
    f.claim(open, "open-2", result("ready", true.into())).await;
    let records = f.store.records(&ceremony_id()).await;
    assert_eq!(f.complete.execute(input).await.unwrap(), accepted);
    assert_eq!(f.store.records(&ceremony_id()).await, records);
}

#[tokio::test]
async fn concurrent_identical_completion_recovers_the_winners_response() {
    use super::ceremony_test_support::{definition, stream_overtaken_once};
    use crate::services::session_facts;
    use made_core::entities::ceremony_commands::ApplyStepResult;
    use made_core::entities::CeremonyCommand;
    use made_core::value_objects::AuditEventType;

    let definition = definition();
    let f = Fixture::new(definition.clone()).await;
    let input = f
        .claim(step_id(), "first", result("ready", true.into()))
        .await;
    let before = f.store.saved(&ceremony_id()).await;
    let events = before
        .decide(
            &CeremonyCommand::ApplyStepResult(ApplyStepResult {
                step_id: input.step_id.clone(),
                result: input.result.clone(),
                claim_fence: input.claim_fence.clone(),
                now: now(),
            }),
            &definition,
        )
        .unwrap();
    let actor = session_facts::step_result_seat(&events, AuditActorKind::Agent).unwrap();
    let fact = session_facts::facts(&before, events, &actor, now())
        .unwrap()
        .remove(0);
    let complete = CompleteCeremonyStepUseCase::new(
        definition_resolver(Arc::new(DefinitionRepositoryFake::new(definition))),
        stream_overtaken_once(f.store.clone(), fact),
        Arc::new(FixedClock::new(now())),
    );
    let accepted = complete.execute(input.clone()).await.unwrap();
    assert_eq!(f.complete.execute(input).await.unwrap(), accepted);
    assert_eq!(
        f.store
            .records(&ceremony_id())
            .await
            .iter()
            .filter(|record| record.event_type() == AuditEventType::StepCompleted)
            .count(),
        1
    );
}
