//! Real-broker proof for durable child recovery.

#![cfg(feature = "container-nats")]

use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use made_adapters::clock::SystemClock;
use made_adapters::memory::{
    ForgetfulMemory, InMemoryProtocolDefinitionPublications, InMemoryProtocolDefinitionRepository,
    InMemoryProtocolEventCursor, InMemoryProtocolEventStore,
};
use made_adapters::nats::{NatsProtocolRecoverySubscriber, NatsSubjects};
use made_adapters::yaml::ProtocolDefinitionYaml;
use made_app::services::SessionStream;
use made_app::usecases::{
    AcceptChildCompletionUseCase, ApplyProtocolTransitionInput, PrepareProtocolChildrenUseCase,
    RecoverProtocolChildrenUseCase, ResolveProtocolDefinitionUseCase, RunProtocolStepInput,
    StartProtocolInput,
};
use made_core::entities::{AuditFact, AuditRecord, ProtocolDefinition};
use made_core::error::DomainError;
use made_core::ports::{
    AppendOutcome, NoopProtocolEventSubscriber, PositionedRecord, ProtocolEventStorePort,
};
use made_core::value_objects::{
    AuditActorKind, DurationMs, GlobalPosition, IdempotencyKey, LeaseOwnerId, ProtocolContext,
    ProtocolEventConsumer, ProtocolEventPageLimit, ProtocolId, RoleId, StepId, StreamVersion,
    TransitionTrigger,
};
use made_embedded::EmbeddedMade;
use made_tests_integration::parity_step_handler::ParityStepHandler;
use testcontainers::{
    core::{IntoContainerPort, WaitFor},
    runners::AsyncRunner,
    GenericImage,
};

const CHILD_YAML: &str = r#"
version: "1.0"
name: recovery_child
states:
  - id: OPEN
    initial: true
  - id: DONE
    terminal: true
transitions:
  - from: OPEN
    to: DONE
    trigger: finish
    guards: [work_done]
steps:
  - id: work
    state: OPEN
    handler: parity_step
guards:
  work_done:
    type: automated
    check: "step_status:work:COMPLETED"
roles:
  - id: CHILD
    allowed_actions: [work, finish]
"#;

const PARENT_YAML: &str = r#"
version: "1.0"
name: recovery_parent
states:
  - id: OPEN
    initial: true
  - id: DONE
    terminal: true
transitions:
  - from: OPEN
    to: DONE
    trigger: finish
    guards: [child_done]
steps:
  - id: delegate
    state: OPEN
    handler: must_not_run
    spawn:
      children:
        - protocol: recovery_child
          version: "1.0"
          inputs: {}
      max_children: 1
      max_depth: 2
guards:
  child_done:
    type: automated
    check: "children_completed:delegate:all"
roles:
  - id: PARENT
    allowed_actions: [delegate, finish]
"#;

#[derive(Debug)]
struct FailFirstGlobalRead {
    inner: Arc<InMemoryProtocolEventStore>,
    fail: AtomicBool,
    reads: AtomicUsize,
}

impl FailFirstGlobalRead {
    fn new(inner: Arc<InMemoryProtocolEventStore>) -> Self {
        Self {
            inner,
            fail: AtomicBool::new(true),
            reads: AtomicUsize::new(0),
        }
    }
}

#[async_trait]
impl ProtocolEventStorePort for FailFirstGlobalRead {
    async fn append(
        &self,
        stream: &ProtocolId,
        expected: StreamVersion,
        facts: Vec<AuditFact>,
    ) -> Result<AppendOutcome, DomainError> {
        self.inner.append(stream, expected, facts).await
    }

    async fn read(
        &self,
        stream: &ProtocolId,
        after: StreamVersion,
        limit: ProtocolEventPageLimit,
    ) -> Result<Vec<AuditRecord>, DomainError> {
        self.inner.read(stream, after, limit).await
    }

    async fn read_all(
        &self,
        from: GlobalPosition,
        limit: ProtocolEventPageLimit,
    ) -> Result<Vec<PositionedRecord>, DomainError> {
        self.reads.fetch_add(1, Ordering::SeqCst);
        if self.fail.swap(false, Ordering::SeqCst) {
            return Err(DomainError::InvariantViolated {
                reason: "proof: transient global read failure",
            });
        }
        self.inner.read_all(from, limit).await
    }

    async fn head(&self, stream: &ProtocolId) -> Result<StreamVersion, DomainError> {
        self.inner.head(stream).await
    }

    async fn streams(&self) -> Result<Vec<ProtocolId>, DomainError> {
        self.inner.streams().await
    }
}

async fn start_nats() -> (
    async_nats::Client,
    testcontainers::ContainerAsync<GenericImage>,
) {
    let container = GenericImage::new("nats", "2")
        .with_exposed_port(4222_u16.tcp())
        .with_wait_for(WaitFor::message_on_stderr("Server is ready"))
        .start()
        .await
        .expect("nats container should start");
    let port = container.get_host_port_ipv4(4222_u16.tcp()).await.unwrap();
    let address = format!("nats://127.0.0.1:{port}");
    let mut last_error = None;
    for _ in 0..20 {
        match async_nats::connect(&address).await {
            Ok(client) => return (client, container),
            Err(error) => {
                last_error = Some(error);
                tokio::time::sleep(Duration::from_millis(100)).await;
            }
        }
    }
    panic!("nats did not accept a connection: {last_error:?}");
}

fn parse(raw: &str) -> ProtocolDefinition {
    ProtocolDefinitionYaml::parse_str(raw).unwrap()
}

fn step(id: &ProtocolId, step: &str, role: &str) -> RunProtocolStepInput {
    RunProtocolStepInput::new(
        id.clone(),
        RoleId::new(role).unwrap(),
        AuditActorKind::Agent,
        StepId::new(step).unwrap(),
        LeaseOwnerId::new("nats-proof").unwrap(),
        IdempotencyKey::new(format!("{id}-{step}")).unwrap(),
        DurationMs::from_millis(30_000),
    )
}

async fn leave_terminal_child_unaccepted(engine: &EmbeddedMade, parent: &str) -> ProtocolId {
    let parent_id = ProtocolId::new(parent).unwrap();
    engine
        .start_published(StartProtocolInput::new(
            parent_id.clone(),
            parse(PARENT_YAML).name().clone(),
            parse(PARENT_YAML).version().clone(),
            ProtocolContext::empty(),
            "nats-proof",
            AuditActorKind::Service,
        ))
        .await
        .unwrap();
    engine
        .run_step(step(&parent_id, "delegate", "PARENT"))
        .await
        .unwrap();
    let child_id = engine
        .instance(&parent_id)
        .await
        .unwrap()
        .child_groups()
        .values()
        .next()
        .unwrap()
        .plan()
        .children()[0]
        .child_id()
        .clone();
    engine
        .run_step(step(&child_id, "work", "CHILD"))
        .await
        .unwrap();
    engine
        .apply_transition(ApplyProtocolTransitionInput::new(
            child_id.clone(),
            RoleId::new("CHILD").unwrap(),
            AuditActorKind::Agent,
            TransitionTrigger::new("finish").unwrap(),
        ))
        .await
        .unwrap();
    child_id
}

async fn wait_for_one_completion(engine: &EmbeddedMade, parent: &str) {
    tokio::time::timeout(Duration::from_secs(4), async {
        loop {
            let instance = engine
                .instance(&ProtocolId::new(parent).unwrap())
                .await
                .unwrap();
            if instance
                .child_groups()
                .values()
                .next()
                .is_some_and(|group| group.completions().len() == 1)
            {
                break;
            }
            tokio::time::sleep(Duration::from_millis(25)).await;
        }
    })
    .await
    .expect("durable child recovery did not catch up");
}

#[tokio::test]
async fn real_nats_readiness_wake_and_periodic_catch_up_retry_durable_events() {
    let (client, _container) = start_nats().await;
    let store = Arc::new(InMemoryProtocolEventStore::new());
    let publications = Arc::new(InMemoryProtocolDefinitionPublications::new());
    let definitions = Arc::new(InMemoryProtocolDefinitionRepository::new());
    let cursors = Arc::new(InMemoryProtocolEventCursor::new());
    let clock = Arc::new(SystemClock::new());
    let memory = Arc::new(ForgetfulMemory::new());
    let engine = EmbeddedMade::builder()
        .with_definition_repository(definitions.clone())
        .with_definition_publications(publications.clone())
        .with_protocol_store(store.clone())
        .with_event_cursor(cursors.clone())
        .with_clock(clock.clone())
        .with_memory(memory.clone())
        .with_step_handler(ParityStepHandler::shared())
        .build();
    engine.publish_definition(parse(CHILD_YAML)).await.unwrap();
    engine.publish_definition(parse(PARENT_YAML)).await.unwrap();

    leave_terminal_child_unaccepted(&engine, "periodic-parent").await;
    let stream = Arc::new(SessionStream::new(
        store.clone(),
        store.clone(),
        Arc::new(NoopProtocolEventSubscriber),
    ));
    let resolve = Arc::new(ResolveProtocolDefinitionUseCase::new(
        definitions,
        publications.clone(),
    ));
    let prepare = Arc::new(PrepareProtocolChildrenUseCase::new(
        resolve.clone(),
        publications.clone(),
        stream.clone(),
        clock.clone(),
        memory,
    ));
    let accept = Arc::new(AcceptChildCompletionUseCase::new(
        resolve,
        publications,
        stream.clone(),
        clock.clone(),
    ));
    let flaky_reads = Arc::new(FailFirstGlobalRead::new(store));
    let recover = Arc::new(RecoverProtocolChildrenUseCase::new(
        flaky_reads.clone(),
        cursors,
        stream,
        prepare,
        accept,
        clock,
        ProtocolEventConsumer::new("children-proof.nats").unwrap(),
    ));
    let subjects = NatsSubjects::new("proof", "proof.trigger.>").unwrap();
    let task = NatsProtocolRecoverySubscriber::new(client.clone(), &subjects, recover)
        .spawn()
        .await
        .expect("readiness round trip should finish before spawn returns");

    wait_for_one_completion(&engine, "periodic-parent").await;
    assert!(
        flaky_reads.reads.load(Ordering::SeqCst) >= 2,
        "the periodic pass did not retry the failed initial catch-up"
    );

    leave_terminal_child_unaccepted(&engine, "woken-parent").await;
    client
        .publish(
            format!("{}.proof", subjects.protocol_prefix),
            b"payload-is-not-authority".as_slice().into(),
        )
        .await
        .unwrap();
    client.flush().await.unwrap();
    wait_for_one_completion(&engine, "woken-parent").await;

    task.abort();
}
