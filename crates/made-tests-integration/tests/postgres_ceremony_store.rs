//! Shared ceremony store conformance against a real Postgres server.

#![cfg(feature = "container-tests")]

use std::sync::Arc;

use made_adapters::postgres::PostgresCeremonyStore;
use made_core::conformance::{
    BudgetLedgerStoreConformance, CeremonyDefinitionPublicationConformance,
    CeremonyEventCursorConformance, CeremonyEventStoreConformance,
    CeremonySnapshotStoreConformance, MemoryConformance,
};
use made_core::ports::{
    ExecutionReceiptStorePort, RecordExecutionIntentOutcome, RecordExecutionReceiptOutcome,
};
use made_core::value_objects::{
    ArtifactSourceKind, AuditActorKind, CeremonyId, ExecutionConnectorId, ExecutionIntent,
    ExecutionOperation, ExecutionReceipt, ExecutionRecoveryCapability, ExecutionRecoveryPageLimit,
    ExecutionRequestBytes, StateIteration, StateVisit, StepClaimFence, StepId, StepIteration,
    StepOutput, StepResult,
};
use made_tests_integration::postgres_fixture;
use time::OffsetDateTime;

#[tokio::test]
async fn postgres_satisfies_every_existing_ceremony_store_contract() {
    let (pool, _container) = postgres_fixture::start().await;
    let store = PostgresCeremonyStore::new(pool);

    let publications = CeremonyDefinitionPublicationConformance::run(&store)
        .await
        .unwrap_or_else(|failure| panic!("{failure}"));
    let events = CeremonyEventStoreConformance::run(&store)
        .await
        .unwrap_or_else(|failure| panic!("{failure}"));
    let snapshots = CeremonySnapshotStoreConformance::run(&store)
        .await
        .unwrap_or_else(|failure| panic!("{failure}"));
    let cursors = CeremonyEventCursorConformance::run(&store)
        .await
        .unwrap_or_else(|failure| panic!("{failure}"));
    let memory = MemoryConformance::run(&store, &store)
        .await
        .unwrap_or_else(|failure| panic!("{failure}"));
    let budgets = BudgetLedgerStoreConformance::run(&store)
        .await
        .unwrap_or_else(|failure| panic!("{failure}"));

    assert_eq!(publications.len(), 5);
    assert_eq!(events.len(), 14);
    assert_eq!(snapshots.len(), 5);
    assert_eq!(cursors.len(), 7);
    assert_eq!(memory.len(), 9);
    assert_eq!(budgets.len(), 3);
}

#[tokio::test]
async fn three_replicas_choose_one_first_intent_and_reopen_the_receipt() {
    let (pool, _container) = postgres_fixture::start().await;
    let replicas = [
        Arc::new(PostgresCeremonyStore::new(pool.clone())),
        Arc::new(PostgresCeremonyStore::new(pool.clone())),
        Arc::new(PostgresCeremonyStore::new(pool.clone())),
    ];
    let operation = operation("shared", b"semantic request");
    let intents = [
        intent(operation.clone(), fence('1')),
        intent(operation.clone(), fence('2')),
        intent(operation.clone(), fence('3')),
    ];
    let (first, second, third) = tokio::join!(
        replicas[0].record_intent(intents[0].clone()),
        replicas[1].record_intent(intents[1].clone()),
        replicas[2].record_intent(intents[2].clone()),
    );
    let outcomes = [first.unwrap(), second.unwrap(), third.unwrap()];
    assert_eq!(
        outcomes
            .iter()
            .filter(|outcome| **outcome == RecordExecutionIntentOutcome::RecordedFirst)
            .count(),
        1
    );
    assert_eq!(
        outcomes
            .iter()
            .filter(|outcome| **outcome == RecordExecutionIntentOutcome::RecordedAdditional)
            .count(),
        2
    );

    let receipt = receipt(&operation, fence('1'));
    assert_eq!(
        replicas[1].record_receipt(receipt.clone()).await.unwrap(),
        RecordExecutionReceiptOutcome::Recorded
    );
    drop(replicas);
    let reopened = PostgresCeremonyStore::new(pool);
    assert_eq!(
        reopened.receipt(operation.operation_id()).await.unwrap(),
        Some(receipt)
    );
    let page = reopened
        .recoverable(None, ExecutionRecoveryPageLimit::new(1).unwrap())
        .await
        .unwrap();
    assert_eq!(page.operations(), &[operation]);
}

fn operation(step: &str, request: &[u8]) -> ExecutionOperation {
    ExecutionOperation::new(
        CeremonyId::new("postgres-ha").unwrap(),
        StepId::new(step).unwrap(),
        StateVisit::FIRST,
        StateIteration::FIRST,
        StepIteration::FIRST,
        ExecutionRequestBytes::new(request.to_vec()).unwrap(),
    )
}

fn fence(digit: char) -> StepClaimFence {
    StepClaimFence::new(digit.to_string().repeat(64)).unwrap()
}

fn intent(operation: ExecutionOperation, claim_fence: StepClaimFence) -> ExecutionIntent {
    ExecutionIntent::new(
        operation,
        claim_fence,
        ExecutionConnectorId::new("postgres.test").unwrap(),
        ExecutionRecoveryCapability::QueryableByOperationId,
        ArtifactSourceKind::ExternalExecution,
        AuditActorKind::Engine,
        OffsetDateTime::UNIX_EPOCH,
    )
    .unwrap()
}

fn receipt(operation: &ExecutionOperation, claim_fence: StepClaimFence) -> ExecutionReceipt {
    ExecutionReceipt::new(
        operation.operation_id().clone(),
        operation.request_digest().clone(),
        claim_fence,
        ExecutionConnectorId::new("postgres.test").unwrap(),
        None,
        ExecutionRecoveryCapability::QueryableByOperationId,
        ArtifactSourceKind::ExternalExecution,
        StepResult::completed(StepOutput::empty()).unwrap(),
        Vec::new(),
        OffsetDateTime::UNIX_EPOCH,
    )
    .unwrap()
}
