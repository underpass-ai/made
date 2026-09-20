//! Two appends, and what happens when the second one never landed.

use std::sync::Arc;

use made_core::entities::{CeremonyCommand, CeremonyDefinition, CeremonyInstance};
use made_core::error::DomainError;
use made_core::value_objects::{
    AuditActorId, AuditActorKind, AuditEventType, BudgetDisposition, CeremonyContext, CeremonyId,
    CeremonyVersion, ClaimDisposition, ClaimDispositionKind, IdempotencyKey, LifecycleReason,
    StepClaimFence, StepId, SuccessorCeremonyId,
};

use crate::services::{session_facts, SessionStream};
use crate::usecases::ceremony_test_support::{
    ceremony_id, definition, definition_name, now, resolver_with, started_instance, stream_over,
    DefinitionRepositoryFake, EventStoreFake, FixedClock, PublicationsFake,
};
use crate::usecases::{
    PauseCeremonyInput, PauseCeremonyUseCase, StartCeremonySuccessorInput,
    StartCeremonySuccessorUseCase as UseCase,
};

const PLAN_ID: &str = "handoff-1";

/// The same ceremony at a version the catalogue holds, so the
/// successor has something to run.
fn successor_definition() -> CeremonyDefinition {
    let mut value = serde_json::to_value(definition()).unwrap();
    value["version"] = serde_json::json!("2.0");
    serde_json::from_value(value).unwrap()
}

fn input() -> StartCeremonySuccessorInput {
    StartCeremonySuccessorInput {
        instance_id: ceremony_id(),
        plan_id: IdempotencyKey::new(PLAN_ID).unwrap(),
        definition_name: definition_name(),
        definition_version: CeremonyVersion::new("2.0").unwrap(),
        carried: Vec::new(),
        dispositions: Vec::new(),
        budget: BudgetDisposition::Fresh,
        context_overrides: None,
        actor_id: AuditActorId::new("operator-1"),
        actor_kind: AuditActorKind::Human,
    }
}

fn successor_id() -> CeremonyId {
    SuccessorCeremonyId::derive(&ceremony_id(), &IdempotencyKey::new(PLAN_ID).unwrap())
        .unwrap()
        .into_ceremony_id()
}

struct Fixture {
    usecase: UseCase,
    store: Arc<EventStoreFake>,
    stream: Arc<SessionStream>,
}

/// A paused session, its successor's version published, and the use
/// case wired over the same store the test can read.
async fn paused_session() -> Fixture {
    let definition = definition();
    let definitions = Arc::new(DefinitionRepositoryFake::new(definition.clone()));
    let publications = Arc::new(PublicationsFake::default());
    publications.seed(successor_definition()).await;
    let store = Arc::new(EventStoreFake::default());
    store.save(&started_instance(&definition)).await.unwrap();
    let (stream, store) = stream_over(store);
    let resolver = resolver_with(definitions, publications.clone());
    let clock = Arc::new(FixedClock::new(now()));

    PauseCeremonyUseCase::new(resolver.clone(), stream.clone(), clock.clone())
        .execute(PauseCeremonyInput::new(
            ceremony_id(),
            "operator-1",
            AuditActorKind::Human,
            LifecycleReason::new("the definition is wrong").unwrap(),
        ))
        .await
        .unwrap();

    Fixture {
        usecase: UseCase::new(resolver, publications, stream.clone(), clock),
        store,
        stream,
    }
}

async fn sealed_handoffs(store: &EventStoreFake) -> usize {
    store
        .records(&ceremony_id())
        .await
        .iter()
        .filter(|record| record.event_type() == AuditEventType::SuccessorPlanned)
        .count()
}

#[tokio::test]
async fn the_handoff_is_sealed_before_the_successor_is_opened() {
    let fixture = paused_session().await;

    let outcome = fixture.usecase.execute(input()).await.unwrap();

    assert_eq!(outcome.successor.id(), &successor_id());
    assert_eq!(outcome.plan.plan_id().as_str(), PLAN_ID);
    assert_eq!(sealed_handoffs(&fixture.store).await, 1);
    let successor = fixture.stream.load(&successor_id()).await.unwrap();
    assert_eq!(
        successor
            .instance
            .succession()
            .map(made_core::value_objects::CeremonySuccession::predecessor_id),
        Some(&ceremony_id())
    );
    assert_eq!(
        successor.instance.definition_version(),
        &CeremonyVersion::new("2.0").unwrap()
    );
}

/// The one that matters: the seal landed, the process died, and the
/// retry has to find the opening it never made — or verify the one it
/// did — without sealing or opening anything twice.
#[tokio::test]
async fn a_retry_after_a_crash_between_the_two_appends_duplicates_nothing() {
    let fixture = paused_session().await;

    let first = fixture.usecase.execute(input()).await.unwrap();
    let records_after_first = fixture.stream.records(&successor_id()).await.unwrap();

    let again = fixture.usecase.execute(input()).await.unwrap();

    assert_eq!(again.successor.id(), first.successor.id());
    assert_eq!(again.plan, first.plan);
    assert_eq!(sealed_handoffs(&fixture.store).await, 1);
    assert_eq!(
        fixture.stream.records(&successor_id()).await.unwrap().len(),
        records_after_first.len(),
        "the retry opened the successor a second time"
    );
    assert_eq!(
        fixture
            .stream
            .records(&successor_id())
            .await
            .unwrap()
            .iter()
            .map(|record| record.event_id().as_str().to_owned())
            .collect::<Vec<_>>(),
        records_after_first
            .iter()
            .map(|record| record.event_id().as_str().to_owned())
            .collect::<Vec<_>>(),
    );
}

/// The crash the other way round: the seal landed and the opening did
/// not. The retry opens it, and still seals nothing new.
#[tokio::test]
async fn a_seal_with_no_opening_is_completed_by_the_retry() {
    let fixture = paused_session().await;
    let plan = {
        // Seal by hand, exactly as the use case would, and stop there.
        let outcome = fixture.usecase.execute(input()).await.unwrap();
        fixture.store.drop_stream(&successor_id()).await;
        outcome.plan
    };

    let again = fixture.usecase.execute(input()).await.unwrap();

    assert_eq!(&again.plan, &plan);
    assert_eq!(sealed_handoffs(&fixture.store).await, 1);
    assert!(fixture.stream.load(&successor_id()).await.is_ok());
}

/// A stream already holding that id with different content is not a
/// half-finished retry; it is somebody else's ceremony.
#[tokio::test]
async fn a_foreign_ceremony_under_the_successor_id_is_refused() {
    let fixture = paused_session().await;
    let opening = CeremonyInstance::decide_start(
        successor_id(),
        &definition(),
        CeremonyContext::empty(),
        None,
        now(),
    )
    .unwrap();
    fixture
        .stream
        .open(
            opening,
            session_facts::party("someone-else", AuditActorKind::Service).unwrap(),
            now(),
        )
        .await
        .unwrap();

    let refused = fixture.usecase.execute(input()).await.unwrap_err();

    assert!(
        matches!(
            refused,
            DomainError::AlreadyExists {
                what: "foreign_successor_ceremony"
            }
        ),
        "{refused:?}"
    );
}

/// A step the caller names but this ceremony never completed is a
/// mistake said out loud, not a step quietly dropped.
#[tokio::test]
async fn carrying_work_this_ceremony_never_completed_is_refused() {
    let fixture = paused_session().await;
    let mut input = input();
    input.carried = vec![StepId::new("gather_voices").unwrap()];

    let refused = fixture.usecase.execute(input).await.unwrap_err();

    let DomainError::InvalidDocument { reason } = refused else {
        panic!("carrying unfinished work is a document defect: {refused:?}");
    };
    assert!(reason.contains("has not completed it"), "{reason}");
    assert_eq!(sealed_handoffs(&fixture.store).await, 0);
}

/// A disposition for a claim nobody holds fails the plan by name.
#[tokio::test]
async fn a_disposition_for_a_step_with_no_claim_is_refused() {
    let fixture = paused_session().await;
    let mut input = input();
    input.dispositions = vec![ClaimDisposition::new(
        StepId::new("gather_voices").unwrap(),
        StepClaimFence::new("a".repeat(64)).unwrap(),
        ClaimDispositionKind::RetryInSuccessor,
    )];

    let refused = fixture.usecase.execute(input).await.unwrap_err();

    let DomainError::InvalidDocument { reason } = refused else {
        panic!("an unclaimed disposition is a document defect: {refused:?}");
    };
    assert!(reason.contains("no outstanding claim"), "{reason}");
}

/// The origin cannot be resumed once it has named its successor, and
/// the refusal names why.
#[tokio::test]
async fn the_origin_refuses_to_resume_once_it_has_handed_off() {
    let fixture = paused_session().await;
    fixture.usecase.execute(input()).await.unwrap();
    let session = fixture.stream.load(&ceremony_id()).await.unwrap();

    let refused = session
        .instance
        .decide(
            &CeremonyCommand::ResumeCeremony(
                made_core::entities::ceremony_commands::ResumeCeremony { now: now() },
            ),
            &definition(),
        )
        .unwrap_err();

    assert!(matches!(
        refused,
        DomainError::LifecycleRefused {
            operation: "superseded_by_successor",
            ..
        }
    ));
}
