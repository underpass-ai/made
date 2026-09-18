use std::sync::atomic::{AtomicI64, Ordering};
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use made_adapters::memory::InMemoryExecutionReceiptStore;
use made_app::workers::{ExecuteCeremonyOperationInput, ExecuteCeremonyOperationUseCase};
use made_core::error::DomainError;
use made_core::ports::{
    CeremonyExecutionConnectorPort, CeremonyExecutionObservation, CeremonyExecutionRequest,
    CeremonyStepHandlerRequest, ClockPort, ExecutionReceiptStorePort,
};
use made_core::value_objects::{
    ArtifactSourceKind, Attributes, AuditActorKind, CeremonyContext, CeremonyId, CeremonyName,
    CeremonyVersion, ExecutionConnectorId, ExecutionRecoveryCapability, StateId, StateIteration,
    StateVisit, StepAttempt, StepClaimFence, StepHandlerConfig, StepHandlerKind, StepId,
    StepIteration, StepOutput, StepResult,
};
use time::OffsetDateTime;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FailurePoint {
    None,
    BeforeEffect,
    AfterEffect,
}

#[derive(Debug)]
struct ConnectorState {
    failure: FailurePoint,
    calls: usize,
    effects: usize,
    observation: Option<CeremonyExecutionObservation>,
}

#[derive(Debug)]
struct FailureInjectingConnector {
    id: ExecutionConnectorId,
    capability: ExecutionRecoveryCapability,
    state: Mutex<ConnectorState>,
}

impl FailureInjectingConnector {
    fn new(failure: FailurePoint, capability: ExecutionRecoveryCapability) -> Self {
        Self {
            id: ExecutionConnectorId::new("test.no-op").unwrap(),
            capability,
            state: Mutex::new(ConnectorState {
                failure,
                calls: 0,
                effects: 0,
                observation: None,
            }),
        }
    }

    fn calls(&self) -> usize {
        self.state.lock().unwrap().calls
    }

    fn effects(&self) -> usize {
        self.state.lock().unwrap().effects
    }
}

#[async_trait]
impl CeremonyExecutionConnectorPort for FailureInjectingConnector {
    fn connector_id(&self) -> &ExecutionConnectorId {
        &self.id
    }

    fn recovery_capability(&self) -> ExecutionRecoveryCapability {
        self.capability
    }

    fn source_kind(&self) -> ArtifactSourceKind {
        ArtifactSourceKind::NoOp
    }

    async fn execute_or_recover(
        &self,
        request: CeremonyExecutionRequest,
    ) -> Result<CeremonyExecutionObservation, DomainError> {
        let mut state = self.state.lock().unwrap();
        state.calls += 1;
        if let Some(observation) = &state.observation {
            return Ok(observation.clone());
        }
        if state.failure == FailurePoint::BeforeEffect {
            state.failure = FailurePoint::None;
            return Err(DomainError::InvariantViolated {
                reason: "injected before external effect",
            });
        }
        state.effects += 1;
        let observation = CeremonyExecutionObservation::new(
            request.intent().claim_fence().clone(),
            None,
            StepResult::completed(StepOutput::empty()).unwrap(),
            Vec::new(),
            OffsetDateTime::UNIX_EPOCH,
        );
        state.observation = Some(observation.clone());
        if state.failure == FailurePoint::AfterEffect {
            state.failure = FailurePoint::None;
            return Err(DomainError::InvariantViolated {
                reason: "injected after external effect",
            });
        }
        Ok(observation)
    }
}

#[derive(Debug)]
struct AdvancingClock {
    seconds: AtomicI64,
}

impl AdvancingClock {
    const fn new() -> Self {
        Self {
            seconds: AtomicI64::new(0),
        }
    }
}

impl ClockPort for AdvancingClock {
    fn now(&self) -> OffsetDateTime {
        OffsetDateTime::UNIX_EPOCH
            + time::Duration::seconds(self.seconds.fetch_add(1, Ordering::Relaxed))
    }
}

fn fence(digit: char) -> StepClaimFence {
    StepClaimFence::new(digit.to_string().repeat(64)).unwrap()
}

fn input(attempt: u32, claim_fence: StepClaimFence) -> ExecuteCeremonyOperationInput {
    ExecuteCeremonyOperationInput {
        handler_request: CeremonyStepHandlerRequest::new(
            CeremonyId::new("ceremony").unwrap(),
            CeremonyName::new("recoverable").unwrap(),
            CeremonyVersion::v1(),
            StateId::new("OPEN").unwrap(),
            StepId::new("work").unwrap(),
            StepHandlerKind::new("no_op").unwrap(),
            StepHandlerConfig::new(Attributes::empty()),
            CeremonyContext::empty(),
            StepAttempt::new(attempt).unwrap(),
        ),
        state_visit: StateVisit::FIRST,
        state_iteration: StateIteration::FIRST,
        step_iteration: StepIteration::FIRST,
        claim_fence,
        actor_kind: AuditActorKind::Engine,
    }
}

fn usecase(
    store: Arc<InMemoryExecutionReceiptStore>,
    connector: Arc<FailureInjectingConnector>,
) -> ExecuteCeremonyOperationUseCase {
    ExecuteCeremonyOperationUseCase::new(store, connector, Arc::new(AdvancingClock::new()))
}

#[tokio::test]
async fn retrying_the_same_fence_reuses_the_first_intent_timestamp() {
    let store = Arc::new(InMemoryExecutionReceiptStore::new());
    let connector = Arc::new(FailureInjectingConnector::new(
        FailurePoint::BeforeEffect,
        ExecutionRecoveryCapability::IdempotentByOperationId,
    ));
    let worker = usecase(store.clone(), connector.clone());

    assert!(worker.execute(input(1, fence('1'))).await.is_err());
    let first_intent = store
        .intent(
            &made_core::value_objects::ExecutionOperationId::for_step(
                &CeremonyId::new("ceremony").unwrap(),
                &StepId::new("work").unwrap(),
                StateVisit::FIRST,
                StateIteration::FIRST,
                StepIteration::FIRST,
            ),
            &fence('1'),
        )
        .await
        .unwrap()
        .unwrap();

    worker.execute(input(2, fence('1'))).await.unwrap();
    let retried_intent = store
        .intent(first_intent.operation().operation_id(), &fence('1'))
        .await
        .unwrap()
        .unwrap();

    assert_eq!(retried_intent.recorded_at(), first_intent.recorded_at());
    assert_eq!(connector.calls(), 2);
    assert_eq!(connector.effects(), 1);
}

#[tokio::test]
async fn a_failure_before_effect_recovers_without_changing_the_operation() {
    let store = Arc::new(InMemoryExecutionReceiptStore::new());
    let connector = Arc::new(FailureInjectingConnector::new(
        FailurePoint::BeforeEffect,
        ExecutionRecoveryCapability::IdempotentByOperationId,
    ));
    let worker = usecase(store, connector.clone());

    assert!(worker.execute(input(1, fence('1'))).await.is_err());
    let receipt = worker.execute(input(2, fence('2'))).await.unwrap();

    assert_eq!(connector.calls(), 2);
    assert_eq!(connector.effects(), 1);
    assert_eq!(receipt.producer_claim_fence(), &fence('2'));
}

#[tokio::test]
async fn a_failure_after_effect_recovers_the_original_fence_without_a_second_effect() {
    let store = Arc::new(InMemoryExecutionReceiptStore::new());
    let connector = Arc::new(FailureInjectingConnector::new(
        FailurePoint::AfterEffect,
        ExecutionRecoveryCapability::IdempotentByOperationId,
    ));
    let worker = usecase(store.clone(), connector.clone());

    assert!(worker.execute(input(1, fence('1'))).await.is_err());
    let receipt = worker.execute(input(2, fence('2'))).await.unwrap();

    assert_eq!(connector.calls(), 2);
    assert_eq!(connector.effects(), 1);
    assert_eq!(receipt.producer_claim_fence(), &fence('1'));
    assert_eq!(
        store.receipt(receipt.operation_id()).await.unwrap(),
        Some(receipt)
    );
}

#[tokio::test]
async fn reconciliation_required_never_reinvokes_an_ambiguous_intent() {
    let store = Arc::new(InMemoryExecutionReceiptStore::new());
    let connector = Arc::new(FailureInjectingConnector::new(
        FailurePoint::AfterEffect,
        ExecutionRecoveryCapability::ReconciliationRequired,
    ));
    let worker = usecase(store, connector.clone());

    assert!(worker.execute(input(1, fence('1'))).await.is_err());
    assert!(worker.execute(input(2, fence('2'))).await.is_err());

    assert_eq!(connector.calls(), 1);
    assert_eq!(connector.effects(), 1);
}

#[tokio::test]
async fn a_receipt_survives_a_crash_before_ceremony_completion() {
    let store = Arc::new(InMemoryExecutionReceiptStore::new());
    let connector = Arc::new(FailureInjectingConnector::new(
        FailurePoint::None,
        ExecutionRecoveryCapability::IdempotentByOperationId,
    ));
    let first_worker = usecase(store.clone(), connector.clone());
    let receipt = first_worker.execute(input(1, fence('1'))).await.unwrap();
    drop(first_worker);

    let recovered = usecase(store, connector.clone())
        .execute(input(2, fence('2')))
        .await
        .unwrap();

    assert_eq!(recovered, receipt);
    assert_eq!(connector.calls(), 1);
    assert_eq!(connector.effects(), 1);
}
