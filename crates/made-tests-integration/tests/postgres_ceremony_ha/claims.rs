use std::sync::Arc;

use made_adapters::clock::SystemClock;
use made_adapters::memory::InMemoryCeremonyDefinitionRepository;
use made_adapters::postgres::PostgresCeremonyStore;
use made_app::services::SessionStream;
use made_app::usecases::{
    ResolveCeremonyDefinitionUseCase, StartCeremonyInput, StartCeremonyStepInput,
    StartCeremonyStepOutput, StartCeremonyStepUseCase, StartCeremonyUseCase,
};
use made_core::entities::CeremonyDefinition;
use made_core::ports::{CeremonyDefinitionRepositoryPort, NoopCeremonyEventSubscriber};
use made_core::value_objects::{
    AuditActorKind, CeremonyContext, CeremonyId, CeremonyName, CeremonyRole, CeremonyState,
    CeremonyStep, CeremonyVersion, DurationMs, IdempotencyKey, LeaseOwnerId, RetryPolicy,
    RoleAction, RoleId, StateId, StepHandlerConfig, StepHandlerKind, StepId,
};
use made_core::DomainError;

async fn definitions() -> Arc<InMemoryCeremonyDefinitionRepository> {
    let definitions = Arc::new(InMemoryCeremonyDefinitionRepository::new());
    let state = StateId::new("WORK").unwrap();
    let step = StepId::new("work").unwrap();
    let definition = CeremonyDefinition::new(
        CeremonyName::new("ha_claim").unwrap(),
        CeremonyVersion::v1(),
        None,
        Vec::new(),
        Vec::new(),
        vec![CeremonyState::initial(state.clone())],
        Vec::new(),
        vec![CeremonyStep::new(
            step.clone(),
            state,
            StepHandlerKind::new("noop").unwrap(),
            StepHandlerConfig::empty(),
            RetryPolicy::single_attempt(),
            None,
        )],
        Vec::new(),
        vec![CeremonyRole::new(RoleId::new("DRIVER").unwrap(), [RoleAction::step(step)]).unwrap()],
    )
    .unwrap();
    definitions.save(&definition).await.unwrap();
    definitions
}

fn stream(store: Arc<PostgresCeremonyStore>) -> Arc<SessionStream> {
    Arc::new(SessionStream::new(
        store.clone(),
        store,
        Arc::new(NoopCeremonyEventSubscriber),
    ))
}

pub(super) async fn start(store: Arc<PostgresCeremonyStore>, id: &str) {
    StartCeremonyUseCase::new(
        definitions().await,
        stream(store.clone()),
        Arc::new(SystemClock::new()),
        store,
    )
    .execute(StartCeremonyInput::new(
        CeremonyId::new(id).unwrap(),
        CeremonyName::new("ha_claim").unwrap(),
        CeremonyVersion::v1(),
        CeremonyContext::empty(),
        "ha-test",
        AuditActorKind::Service,
    ))
    .await
    .unwrap();
}

pub(super) async fn claim(
    store: Arc<PostgresCeremonyStore>,
    id: &str,
    replica: &str,
) -> Result<StartCeremonyStepOutput, DomainError> {
    StartCeremonyStepUseCase::new(
        Arc::new(ResolveCeremonyDefinitionUseCase::new(
            definitions().await,
            store.clone(),
        )),
        stream(store),
        Arc::new(SystemClock::new()),
    )
    .execute(StartCeremonyStepInput::new(
        CeremonyId::new(id).unwrap(),
        RoleId::new("DRIVER").unwrap(),
        AuditActorKind::Agent,
        StepId::new("work").unwrap(),
        LeaseOwnerId::new(format!("replica-{replica}")).unwrap(),
        IdempotencyKey::new(format!("claim-{replica}")).unwrap(),
        DurationMs::from_millis(60_000),
    ))
    .await
}
