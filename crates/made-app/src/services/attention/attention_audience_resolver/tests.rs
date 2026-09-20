//! What a binding turns out to cover.

use std::sync::Arc;

use async_trait::async_trait;
use made_core::entities::{AgenticSystem, AgenticSystemExecution};
use made_core::error::DomainError;
use made_core::ports::{
    AgenticSystemExecutionCreation, AgenticSystemExecutionStorePort, AgenticSystemExecutionUpdate,
    AgenticSystemPage, AgenticSystemQuery, AgenticSystemRepositoryPort, AgenticSystemSaveOutcome,
};
use made_core::value_objects::{
    AgenticSystemDigest, AgenticSystemExecutionId, AgenticSystemId, AgenticSystemRevision,
    CeremonyDefinitionDigest, CeremonyExecutionLink, CeremonyId, CeremonyName, CeremonyVersion,
    DefinitionPin, HostActivationMode, HostAddress, HostAgentIncarnation, HostDestination,
    HostKind, IntegratorBinding, IntegratorBindingId, IntegratorScope, LoopRound, RoleId,
    SystemCeremonyId, SystemPin,
};
use time::OffsetDateTime;

use super::AttentionAudienceResolver;

fn now() -> OffsetDateTime {
    OffsetDateTime::UNIX_EPOCH
}

fn binding(scope: IntegratorScope) -> IntegratorBinding {
    IntegratorBinding::new(
        IntegratorBindingId::new("b-1").unwrap(),
        scope,
        RoleId::new("INTEGRATOR").unwrap(),
        HostDestination::new(
            HostKind::new("claude-code").unwrap(),
            HostAddress::new("session-1").unwrap(),
            HostActivationMode::None,
        ),
        HostAgentIncarnation::new("run-1").unwrap(),
        now(),
    )
}

fn execution(links: &[(&str, Option<&str>)]) -> AgenticSystemExecution {
    let pin = DefinitionPin::new(
        CeremonyName::new("review").unwrap(),
        CeremonyVersion::v1(),
        CeremonyDefinitionDigest::from_bytes([0; 32]),
    );
    AgenticSystemExecution::plan(
        AgenticSystemExecutionId::new("x-1").unwrap(),
        SystemPin::new(
            AgenticSystemId::new("delivery").unwrap(),
            AgenticSystemRevision::INITIAL,
            AgenticSystemDigest::of_canonical_form(b"{}"),
        ),
        links.iter().map(|(system_id, instance)| {
            let link = CeremonyExecutionLink::pending(pin.clone());
            let link = match instance {
                Some(instance) => {
                    link.started(CeremonyId::new(*instance).unwrap(), LoopRound::ZERO)
                }
                None => link,
            };
            (SystemCeremonyId::new(system_id).unwrap(), link)
        }),
        [],
        [],
        now(),
    )
    .unwrap()
}

fn resolver(executions: ExecutionStoreFake) -> AttentionAudienceResolver {
    AttentionAudienceResolver::new(Arc::new(executions), Arc::new(SystemRepositoryFake))
}

#[tokio::test]
async fn a_ceremony_binding_covers_that_ceremony_and_nothing_else() {
    let bound = binding(IntegratorScope::ceremony(
        CeremonyId::new("loop-1").unwrap(),
    ));

    let audience = resolver(ExecutionStoreFake { execution: None })
        .resolve(&bound)
        .await
        .unwrap();

    assert!(audience.covers(&CeremonyId::new("loop-1").unwrap()));
    assert!(!audience.covers(&CeremonyId::new("loop-2").unwrap()));
    assert!(audience.system_execution_id().is_none());
}

#[tokio::test]
async fn a_system_binding_covers_every_ceremony_its_run_has_opened() {
    let bound = binding(IntegratorScope::system_execution(
        AgenticSystemExecutionId::new("x-1").unwrap(),
    ));
    let store = ExecutionStoreFake {
        execution: Some(execution(&[
            ("design", Some("loop-1")),
            ("build", Some("loop-2")),
            ("ship", None),
        ])),
    };

    let audience = resolver(store).resolve(&bound).await.unwrap();

    assert!(audience.covers(&CeremonyId::new("loop-1").unwrap()));
    assert!(audience.covers(&CeremonyId::new("loop-2").unwrap()));
    assert_eq!(
        audience.ceremonies().len(),
        2,
        "a composition nothing has been started for is not a ceremony yet"
    );
    assert_eq!(
        audience
            .system_execution_id()
            .map(AgenticSystemExecutionId::as_str),
        Some("x-1")
    );
}

#[tokio::test]
async fn a_binding_whose_run_is_gone_covers_nothing_and_does_not_fail() {
    let bound = binding(IntegratorScope::system_execution(
        AgenticSystemExecutionId::new("x-1").unwrap(),
    ));

    let audience = resolver(ExecutionStoreFake { execution: None })
        .resolve(&bound)
        .await
        .unwrap();

    assert!(
        audience.ceremonies().is_empty(),
        "a run that ended underneath a binding is not an error; it is nothing to do"
    );
}

struct ExecutionStoreFake {
    execution: Option<AgenticSystemExecution>,
}

#[async_trait]
impl AgenticSystemExecutionStorePort for ExecutionStoreFake {
    async fn create(
        &self,
        _execution: AgenticSystemExecution,
    ) -> Result<AgenticSystemExecutionCreation, DomainError> {
        unimplemented!("the resolver only reads")
    }

    async fn get(
        &self,
        _id: &AgenticSystemExecutionId,
    ) -> Result<Option<AgenticSystemExecution>, DomainError> {
        Ok(self.execution.clone())
    }

    async fn update(
        &self,
        _execution: AgenticSystemExecution,
        _expected_updated_at: OffsetDateTime,
    ) -> Result<AgenticSystemExecutionUpdate, DomainError> {
        unimplemented!("the resolver only reads")
    }

    async fn list_by_system(
        &self,
        _id: &AgenticSystemId,
    ) -> Result<Vec<AgenticSystemExecution>, DomainError> {
        unimplemented!("the resolver asks for one run")
    }
}

struct SystemRepositoryFake;

#[async_trait]
impl AgenticSystemRepositoryPort for SystemRepositoryFake {
    async fn save(
        &self,
        _system: AgenticSystem,
        _expected: Option<AgenticSystemRevision>,
    ) -> Result<AgenticSystemSaveOutcome, DomainError> {
        unimplemented!("the resolver only reads")
    }

    async fn get(
        &self,
        _id: &AgenticSystemId,
        _revision: Option<AgenticSystemRevision>,
    ) -> Result<Option<AgenticSystem>, DomainError> {
        Ok(None)
    }

    async fn list(&self, _query: &AgenticSystemQuery) -> Result<AgenticSystemPage, DomainError> {
        unimplemented!("the resolver asks for one design")
    }
}
