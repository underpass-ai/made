//! The service's own fanout, over in-memory adapters.
//!
//! What this is about is the list, not the subscribers on it: each of
//! those is tested where it lives. A projection that was built and
//! never installed would pass every one of those tests and leave the
//! deployment answering with an empty queue for ever, which is exactly
//! what this slice was for.

use std::sync::Arc;

use made_adapters::memory::{
    InMemoryAgenticSystemExecutions, InMemoryAgenticSystemPublications,
    InMemoryAgenticSystemRepository,
};
use made_adapters::memory::{
    InMemoryHostDeliveryLedger, InMemoryIntegratorBindings, InMemoryProtocolAgentStatus,
    InMemoryProtocolEventCursor, InMemoryProtocolEventStore,
};
use made_adapters::metrics::PrometheusMetricsRecorder;
use made_core::entities::protocol_events::StepCompleted;
use made_core::entities::{AuditFact, ProtocolEvent};
use made_core::ports::{
    AppendOutcome, BindReplacement, HostDeliveryQuery, NoopProtocolEventSubscriber,
};
use made_core::value_objects::{
    AuditActor, AuditActorKind, EventId, GlobalPosition, HostActivationMode, HostAddress,
    HostAgentIncarnation, HostDeliveryItemKind, HostDestination, HostKind, IntegratorBinding,
    IntegratorBindingId, IntegratorScope, ProtocolEventPageLimit, ProtocolId, ProtocolName,
    ProtocolVersion, RoleId, StepAttempt, StepId, StepIteration, StepOutput, StepResult,
    StreamVersion,
};
use time::OffsetDateTime;

use super::*;
use crate::compose::integrator_loop;
use crate::{AgenticSystemHandles, HostDeliveryHandles};

fn protocol() -> ProtocolId {
    ProtocolId::new("loop-1").expect("a valid protocol id")
}

fn binding() -> IntegratorBinding {
    IntegratorBinding::new(
        IntegratorBindingId::new("b-1").expect("a valid binding id"),
        IntegratorScope::protocol(protocol()),
        RoleId::new("INTEGRATOR").expect("a valid role"),
        HostDestination::new(
            HostKind::new("claude-code").expect("a valid host kind"),
            HostAddress::new("session-1").expect("a valid address"),
            HostActivationMode::None,
        ),
        HostAgentIncarnation::new("run-1").expect("a valid incarnation"),
        OffsetDateTime::UNIX_EPOCH,
    )
}

fn sealed_result() -> AuditFact {
    AuditFact {
        event_id: EventId::new("e-1").expect("a valid event id"),
        event: ProtocolEvent::StepCompleted(StepCompleted {
            step_id: StepId::new("implement").expect("a valid step id"),
            state_visit: None,
            state_iteration: None,
            iteration: StepIteration::FIRST,
            attempt: StepAttempt::FIRST,
            result: StepResult::completed(StepOutput::default()).expect("a valid result"),
            next_iteration: None,
            finished_by: RoleId::new("IMPLEMENTER").expect("a valid role"),
            finished_at: OffsetDateTime::UNIX_EPOCH,
        }),
        protocol_id: protocol(),
        definition_name: ProtocolName::new("review").expect("a valid name"),
        definition_version: ProtocolVersion::v1(),
        occurred_at: OffsetDateTime::UNIX_EPOCH,
        actor: AuditActor::new("test", AuditActorKind::Engine, None).expect("a valid actor"),
        correlation_id: None,
        causation_id: None,
        trace: None,
    }
}

/// The subscriber has to be on the service's list too.
///
/// The append path is asked directly — the fanout the service installs
/// is handed one sealed result — and the ledger is read through its own
/// port. The read paths project before they answer, so asking a use
/// case here would prove nothing about whether the append woke anybody.
#[tokio::test]
async fn the_service_fanout_offers_a_sealed_result_to_the_bound_integrator() {
    let events = Arc::new(InMemoryProtocolEventStore::new());
    let host_delivery = HostDeliveryHandles {
        ledger: Arc::new(InMemoryHostDeliveryLedger::new()),
        bindings: Arc::new(InMemoryIntegratorBindings::new()),
        activation: Arc::new(made_adapters::activation::NoHostActivation::new()),
    };
    let agentic_system = AgenticSystemHandles {
        repository: Arc::new(InMemoryAgenticSystemRepository::new()),
        publications: Arc::new(InMemoryAgenticSystemPublications::new()),
        executions: Arc::new(InMemoryAgenticSystemExecutions::new()),
        diagrams: Arc::new(made_adapters::mermaid::AgenticSystemMermaidDiagram::new()),
    };
    host_delivery
        .bindings
        .bind(binding(), BindReplacement::Refuse)
        .await
        .expect("the binding is taken");

    let attention = integrator_loop::recovery(
        events.clone(),
        Arc::new(InMemoryProtocolEventCursor::new()),
        integrator_loop::definition_lookup(
            &(events.clone() as Arc<dyn made_core::ports::ProtocolEventStorePort>),
            &(events.clone() as Arc<dyn made_core::ports::ProtocolSnapshotStorePort>),
            &(Arc::new(made_adapters::memory::InMemoryProtocolDefinitionRepository::new())
                as Arc<dyn made_core::ports::ProtocolDefinitionRepositoryPort>),
            &(Arc::new(made_adapters::memory::InMemoryProtocolDefinitionPublications::new())
                as Arc<dyn made_core::ports::ProtocolDefinitionPublicationPort>),
        ),
        &host_delivery,
        &agentic_system,
        Arc::new(made_adapters::clock::SystemClock::new()),
    );
    let fanout = subscribers(
        EngineProjections {
            memory: Arc::new(NoopProtocolEventSubscriber),
            progress: Arc::new(NoopProtocolEventSubscriber),
            events: events.clone(),
            snapshots: events.clone(),
            metrics: Arc::new(
                PrometheusMetricsRecorder::new().expect("the metrics registry is fresh"),
            ),
            deliveries: host_delivery.ledger.clone(),
            agent_status: Arc::new(InMemoryProtocolAgentStatus::new()),
            attention,
        },
        None,
    );

    let AppendOutcome::Appended { .. } = events
        .append(&protocol(), StreamVersion::EMPTY, vec![sealed_result()])
        .await
        .expect("the result is sealed")
    else {
        panic!("an empty stream does not conflict");
    };
    let records = events
        .read_all(
            GlobalPosition::FIRST,
            ProtocolEventPageLimit::new(10).expect("a valid limit"),
        )
        .await
        .expect("the feed reads");
    fanout.observe(&records).await;

    let held = host_delivery
        .ledger
        .list(&HostDeliveryQuery::new())
        .await
        .expect("the ledger reads");
    assert_eq!(
        held.records().len(),
        1,
        "the attention projection is not on the list the service installs"
    );
    assert_eq!(
        held.records()[0].item().kind(),
        HostDeliveryItemKind::Attention
    );
}
