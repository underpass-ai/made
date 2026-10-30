use std::sync::Arc;

use made_adapters::clock::SystemClock;
use made_adapters::memory::InMemoryProtocolDefinitionRepository;
use made_adapters::postgres::PostgresProtocolStore;
use made_app::services::SessionStream;
use made_app::usecases::{
    ResolveProtocolDefinitionUseCase, StartProtocolInput, StartProtocolStepInput,
    StartProtocolStepOutput, StartProtocolStepUseCase, StartProtocolUseCase,
};
use made_core::entities::ProtocolDefinition;
use made_core::ports::{NoopProtocolEventSubscriber, ProtocolDefinitionRepositoryPort};
use made_core::value_objects::{
    AuditActorKind, DurationMs, IdempotencyKey, LeaseOwnerId, ProtocolContext, ProtocolId,
    ProtocolName, ProtocolRole, ProtocolState, ProtocolStep, ProtocolVersion, RetryPolicy,
    RoleAction, RoleId, StateId, StepHandlerConfig, StepHandlerKind, StepId,
};
use made_core::DomainError;

async fn definitions() -> Arc<InMemoryProtocolDefinitionRepository> {
    let definitions = Arc::new(InMemoryProtocolDefinitionRepository::new());
    let state = StateId::new("WORK").unwrap();
    let step = StepId::new("work").unwrap();
    let definition = ProtocolDefinition::new(
        ProtocolName::new("ha_claim").unwrap(),
        ProtocolVersion::v1(),
        None,
        Vec::new(),
        Vec::new(),
        vec![ProtocolState::initial(state.clone())],
        Vec::new(),
        vec![ProtocolStep::new(
            step.clone(),
            state,
            StepHandlerKind::new("noop").unwrap(),
            StepHandlerConfig::empty(),
            RetryPolicy::single_attempt(),
            None,
        )],
        Vec::new(),
        vec![ProtocolRole::new(RoleId::new("DRIVER").unwrap(), [RoleAction::step(step)]).unwrap()],
    )
    .unwrap();
    definitions.save(&definition).await.unwrap();
    definitions
}

fn stream(store: Arc<PostgresProtocolStore>) -> Arc<SessionStream> {
    Arc::new(SessionStream::new(
        store.clone(),
        store,
        Arc::new(NoopProtocolEventSubscriber),
    ))
}

pub(super) async fn start(store: Arc<PostgresProtocolStore>, id: &str) {
    StartProtocolUseCase::new(
        definitions().await,
        stream(store.clone()),
        Arc::new(SystemClock::new()),
        store,
    )
    .execute(StartProtocolInput::new(
        ProtocolId::new(id).unwrap(),
        ProtocolName::new("ha_claim").unwrap(),
        ProtocolVersion::v1(),
        ProtocolContext::empty(),
        "ha-test",
        AuditActorKind::Service,
    ))
    .await
    .unwrap();
}

pub(super) async fn claim(
    store: Arc<PostgresProtocolStore>,
    id: &str,
    replica: &str,
) -> Result<StartProtocolStepOutput, DomainError> {
    StartProtocolStepUseCase::new(
        Arc::new(ResolveProtocolDefinitionUseCase::new(
            definitions().await,
            store.clone(),
        )),
        stream(store),
        Arc::new(SystemClock::new()),
    )
    .execute(StartProtocolStepInput::new(
        ProtocolId::new(id).unwrap(),
        RoleId::new("DRIVER").unwrap(),
        AuditActorKind::Agent,
        StepId::new("work").unwrap(),
        LeaseOwnerId::new(format!("replica-{replica}")).unwrap(),
        IdempotencyKey::new(format!("claim-{replica}")).unwrap(),
        DurationMs::from_millis(60_000),
    ))
    .await
}
