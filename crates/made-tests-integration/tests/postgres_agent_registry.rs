//! Integration test: [`PostgresAgentRegistry`] exercises
//! `AgentRegistryPort` (write) + `AgentResolverPort` (read, via
//! factory reconstruction) against a real Postgres container.
//!
//! Runs only when the `container-tests` feature is enabled (CI).

#![cfg(feature = "container-tests")]

use std::sync::Arc;

use made_adapters::noop::NoopAgentFactory;
use made_adapters::postgres::PostgresAgentRegistry;
use made_core::entities::TaskConstraints;
use made_core::error::DomainError;
use made_core::ports::{
    AgentDescriptor, AgentFactoryPort, AgentPort, AgentRegistryPort, AgentResolverPort, Critique,
    DraftRequest, Revision,
};
use made_core::value_objects::{AgentId, AgentKind, Attributes, ProposalContent, Specialty};
use made_tests_integration::postgres_fixture;

fn factory() -> Arc<dyn AgentFactoryPort> {
    Arc::new(NoopAgentFactory::new())
}

fn noop_descriptor(id: &str, specialty: &str) -> AgentDescriptor {
    AgentDescriptor {
        id: AgentId::new(id).unwrap(),
        specialty: Specialty::new(specialty).unwrap(),
        kind: AgentKind::new("noop").unwrap(),
        attributes: Attributes::empty(),
    }
}

#[derive(Debug)]
struct StubAgent {
    id: AgentId,
    specialty: Specialty,
}
#[async_trait::async_trait]
impl AgentPort for StubAgent {
    fn id(&self) -> &AgentId {
        &self.id
    }
    fn specialty(&self) -> &Specialty {
        &self.specialty
    }
    async fn generate(&self, _: DraftRequest) -> Result<Revision, DomainError> {
        Ok(Revision {
            content: String::new().into(),
        })
    }
    async fn critique(
        &self,
        _: &ProposalContent,
        _: &TaskConstraints,
    ) -> Result<Critique, DomainError> {
        Ok(Critique {
            feedback: String::new().into(),
        })
    }
    async fn revise(&self, own: &ProposalContent, _: &Critique) -> Result<Revision, DomainError> {
        Ok(Revision {
            content: own.clone(),
        })
    }
}

#[tokio::test]
async fn insert_descriptor_roundtrips_through_resolve() {
    let (pool, _container) = postgres_fixture::start().await;
    let registry = PostgresAgentRegistry::new(pool, factory());

    registry
        .insert_descriptor(&noop_descriptor("a1", "triage"))
        .await
        .unwrap();

    let resolved = registry
        .resolve(&AgentId::new("a1").unwrap())
        .await
        .unwrap();
    assert_eq!(resolved.id().as_str(), "a1");
    assert_eq!(resolved.specialty().as_str(), "triage");
}

#[tokio::test]
async fn duplicate_insert_is_already_exists() {
    let (pool, _container) = postgres_fixture::start().await;
    let registry = PostgresAgentRegistry::new(pool, factory());

    registry
        .insert_descriptor(&noop_descriptor("a1", "triage"))
        .await
        .unwrap();
    let err = registry
        .insert_descriptor(&noop_descriptor("a1", "triage"))
        .await
        .unwrap_err();
    assert!(matches!(err, DomainError::AlreadyExists { what: "agent" }));
}

#[tokio::test]
async fn register_via_port_trait_persists_then_unregister_removes() {
    let (pool, _container) = postgres_fixture::start().await;
    let registry = PostgresAgentRegistry::new(pool, factory());

    let agent: Arc<dyn AgentPort> = Arc::new(StubAgent {
        id: AgentId::new("a-port").unwrap(),
        specialty: Specialty::new("triage").unwrap(),
    });
    assert!(registry.register(agent.clone()).await.is_err());
    registry
        .register_described(noop_descriptor("a-port", "triage"), agent)
        .await
        .unwrap();
    // Post-register the descriptor resolves to a live NoopAgent (the
    // wired factory only knows the "noop" kind; the port contract
    // does not expose which concrete type is returned).
    let resolved = registry
        .resolve(&AgentId::new("a-port").unwrap())
        .await
        .unwrap();
    assert_eq!(resolved.id().as_str(), "a-port");

    registry
        .unregister(&AgentId::new("a-port").unwrap())
        .await
        .unwrap();
    let err = registry
        .resolve(&AgentId::new("a-port").unwrap())
        .await
        .unwrap_err();
    assert!(matches!(err, DomainError::NotFound { what: "agent" }));
}

#[tokio::test]
async fn unregister_missing_is_not_found() {
    let (pool, _container) = postgres_fixture::start().await;
    let registry = PostgresAgentRegistry::new(pool, factory());

    let err = registry
        .unregister(&AgentId::new("ghost").unwrap())
        .await
        .unwrap_err();
    assert!(matches!(err, DomainError::NotFound { what: "agent" }));
}

#[tokio::test]
async fn resolve_propagates_factory_rejection_for_unsupported_kind() {
    let (pool, _container) = postgres_fixture::start().await;
    let registry = PostgresAgentRegistry::new(pool, factory());

    let descriptor = AgentDescriptor {
        id: AgentId::new("a-vllm").unwrap(),
        specialty: Specialty::new("triage").unwrap(),
        kind: AgentKind::new("vllm").unwrap(),
        attributes: Attributes::empty(),
    };
    registry.insert_descriptor(&descriptor).await.unwrap();

    // The noop factory rejects every non-"noop" kind — a deliberate
    // honesty signal that the deployment's factory is not wired for
    // the registered kind.
    let err = registry
        .resolve(&AgentId::new("a-vllm").unwrap())
        .await
        .unwrap_err();
    assert!(matches!(err, DomainError::InvariantViolated { .. }));
}

#[tokio::test]
async fn described_registration_preserves_provider_and_refuses_credentials_without_inserting() {
    let (pool, _container) = postgres_fixture::start().await;
    let registry = PostgresAgentRegistry::new(pool.clone(), factory());
    let descriptor = AgentDescriptor {
        id: AgentId::new("actual-vllm").unwrap(),
        specialty: Specialty::new("triage").unwrap(),
        kind: AgentKind::new("vllm").unwrap(),
        attributes: Attributes::new(std::collections::BTreeMap::from([(
            "provider.model".into(),
            serde_json::json!("local-model"),
        )]))
        .unwrap(),
    };
    let agent: Arc<dyn AgentPort> = Arc::new(StubAgent {
        id: descriptor.id.clone(),
        specialty: descriptor.specialty.clone(),
    });
    registry
        .register_described(descriptor.clone(), agent)
        .await
        .unwrap();
    let recording = Arc::new(RecordingDescriptorFactory::default());
    PostgresAgentRegistry::new(pool.clone(), recording.clone())
        .resolve(&descriptor.id)
        .await
        .unwrap();
    assert_eq!(
        recording.0.lock().unwrap().as_slice(),
        std::slice::from_ref(&descriptor)
    );
    // A fresh process with an incompatible factory must refuse, not silently noop.
    assert!(PostgresAgentRegistry::new(pool.clone(), factory())
        .resolve(&descriptor.id)
        .await
        .is_err());

    let mut secret = noop_descriptor("refused-secret", "triage");
    secret.attributes = Attributes::new(std::collections::BTreeMap::from([(
        "provider.api_key".into(),
        serde_json::json!("synthetic-test-value"),
    )]))
    .unwrap();
    assert!(registry.insert_descriptor(&secret).await.is_err());
    assert!(matches!(
        registry.resolve(&secret.id).await,
        Err(DomainError::NotFound { what: "agent" })
    ));
}

#[derive(Debug, Default)]
struct RecordingDescriptorFactory(std::sync::Mutex<Vec<AgentDescriptor>>);
#[async_trait::async_trait]
impl AgentFactoryPort for RecordingDescriptorFactory {
    async fn create(&self, descriptor: AgentDescriptor) -> Result<Arc<dyn AgentPort>, DomainError> {
        self.0.lock().unwrap().push(descriptor.clone());
        Ok(Arc::new(StubAgent {
            id: descriptor.id,
            specialty: descriptor.specialty,
        }))
    }
}
