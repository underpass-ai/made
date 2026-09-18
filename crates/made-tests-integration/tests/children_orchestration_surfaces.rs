//! Published child ceremonies across the Rust, gRPC and both MCP surfaces.

use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use async_trait::async_trait;
use made_adapters::sqlite::SqliteCeremonyStore;
use made_adapters::yaml::CeremonyDefinitionYaml;
use made_app::usecases::{
    AcceptChildCompletionInput, ApplyCeremonyTransitionInput, PauseCeremonyInput,
    PrepareCeremonyChildrenInput, RunCeremonyStepInput, StartCeremonyInput, StartCeremonyStepInput,
};
use made_core::entities::{AuditFact, CeremonyDefinition, CeremonyEvent};
use made_core::error::DomainError;
use made_core::ports::{
    AppendOutcome, CeremonyEventStorePort, CeremonySnapshot, CeremonySnapshotStorePort,
    PositionedRecord,
};
use made_core::value_objects::{
    AuditActorKind, CeremonyContext, CeremonyEventPageLimit, CeremonyId, DurationMs, EventId,
    GlobalPosition, IdempotencyKey, LeaseOwnerId, LifecycleReason, RoleId, StepId, StreamVersion,
    TransitionTrigger,
};
use made_embedded::EmbeddedMade;
use made_mcp::backend::MadeMcpGrpcTlsConfig;
use made_mcp::{EmbeddedMadeMcpBackend, GrpcMadeMcpBackend, MadeMcpServer};
use made_proto::v1::made_service_client::MadeServiceClient;
use made_proto::v1::{
    AcceptChildCompletionRequest, ApplyCeremonyTransitionRequest, PrepareCeremonyChildrenRequest,
    PublishCeremonyDefinitionRequest, ReadCeremonyEventsRequest, RunCeremonyStepRequest,
    StartPublishedCeremonyRequest,
};
use made_tests_integration::grpc_fixture::{GrpcFixture, GrpcFixtureWiring};
use made_tests_integration::parity_clock::{ParityClock, PARITY_INSTANT};
use made_tests_integration::parity_step_handler::ParityStepHandler;
use serde_json::{json, Value};
use tokio::sync::Notify;

const CHILD_YAML: &str = r#"
version: "1.0"
name: child_review
states:
  - id: REVIEWING
    initial: true
  - id: DONE
    terminal: true
transitions:
  - from: REVIEWING
    to: DONE
    trigger: finish
    guards: [work_done]
steps:
  - id: work
    state: REVIEWING
    handler: child_work
guards:
  work_done:
    type: automated
    check: "step_status:work:COMPLETED"
roles:
  - id: CHILD
    allowed_actions: [work, finish]
"#;

fn parent_yaml(join: &str, child_count: usize) -> String {
    let name = join.replace(':', "_");
    let children = (0..child_count)
        .map(|_| {
            "        - ceremony: child_review\n          version: \"1.0\"\n          inputs: {}"
        })
        .collect::<Vec<_>>()
        .join("\n");
    format!(
        r#"
version: "1.0"
name: parent_{name}
states:
  - id: DELEGATING
    initial: true
  - id: DONE
    terminal: true
transitions:
  - from: DELEGATING
    to: DONE
    trigger: finish
    guards: [children_done]
steps:
  - id: delegate
    state: DELEGATING
    handler: must_not_run
    spawn:
      children:
{children}
      max_children: {child_count}
      max_depth: 2
guards:
  children_done:
    type: automated
    check: "children_completed:delegate:{join}"
roles:
  - id: PARENT
    allowed_actions: [delegate, finish]
"#
    )
}

fn definition(raw: &str) -> CeremonyDefinition {
    CeremonyDefinitionYaml::parse_str(raw).expect("test ceremony should parse")
}

fn scratch() -> tempfile::TempDir {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tmp");
    std::fs::create_dir_all(&path).unwrap();
    tempfile::tempdir_in(path).unwrap()
}

fn durable_engine(path: &Path) -> EmbeddedMade {
    let store = Arc::new(SqliteCeremonyStore::open(path).unwrap());
    EmbeddedMade::builder()
        .with_ceremony_store(store.clone())
        .with_definition_publications(store)
        .with_clock(ParityClock::shared())
        .with_step_handler(ParityStepHandler::shared())
        .build()
}

#[derive(Debug, Default)]
struct AppendBarrier {
    paused: AtomicBool,
    arrived: Notify,
    release: Notify,
}

impl AppendBarrier {
    async fn wait_until_paused(&self) {
        if self.paused.load(Ordering::Acquire) {
            return;
        }
        self.arrived.notified().await;
    }

    fn resume(&self) {
        self.release.notify_one();
    }
}

#[derive(Debug)]
struct PauseBeforeCompletionStore {
    inner: Arc<SqliteCeremonyStore>,
    barrier: Arc<AppendBarrier>,
    pause_on: PausedEvent,
}

#[derive(Debug, Clone, Copy)]
enum PausedEvent {
    Plan,
    SealedPlan,
    Completion,
}

#[async_trait]
impl CeremonyEventStorePort for PauseBeforeCompletionStore {
    async fn append(
        &self,
        stream: &CeremonyId,
        expected: StreamVersion,
        facts: Vec<AuditFact>,
    ) -> Result<AppendOutcome, DomainError> {
        let should_pause = facts
            .iter()
            .any(|fact| match (&self.pause_on, &fact.event) {
                (PausedEvent::Plan, CeremonyEvent::ChildSpawnPlanned(_)) => true,
                (PausedEvent::Completion, CeremonyEvent::StepCompleted(completed)) => {
                    completed.step_id.as_str() == "delegate"
                }
                _ => false,
            });
        if should_pause && !self.barrier.paused.swap(true, Ordering::AcqRel) {
            self.barrier.arrived.notify_one();
            self.barrier.release.notified().await;
        }
        let pause_after = matches!(self.pause_on, PausedEvent::SealedPlan)
            && facts
                .iter()
                .any(|fact| matches!(fact.event, CeremonyEvent::ChildSpawnPlanned(_)));
        let outcome = self.inner.append(stream, expected, facts).await?;
        if pause_after && !self.barrier.paused.swap(true, Ordering::AcqRel) {
            self.barrier.arrived.notify_one();
            self.barrier.release.notified().await;
        }
        Ok(outcome)
    }

    async fn read(
        &self,
        stream: &CeremonyId,
        after: StreamVersion,
        limit: CeremonyEventPageLimit,
    ) -> Result<Vec<made_core::entities::AuditRecord>, DomainError> {
        self.inner.read(stream, after, limit).await
    }

    async fn read_all(
        &self,
        from: GlobalPosition,
        limit: CeremonyEventPageLimit,
    ) -> Result<Vec<PositionedRecord>, DomainError> {
        self.inner.read_all(from, limit).await
    }

    async fn head(&self, stream: &CeremonyId) -> Result<StreamVersion, DomainError> {
        self.inner.head(stream).await
    }

    async fn streams(&self) -> Result<Vec<CeremonyId>, DomainError> {
        self.inner.streams().await
    }
}

#[async_trait]
impl CeremonySnapshotStorePort for PauseBeforeCompletionStore {
    async fn save(&self, snapshot: CeremonySnapshot) -> Result<(), DomainError> {
        self.inner.save(snapshot).await
    }

    async fn latest(&self, stream: &CeremonyId) -> Result<Option<CeremonySnapshot>, DomainError> {
        self.inner.latest(stream).await
    }

    async fn forget(&self, stream: &CeremonyId) -> Result<(), DomainError> {
        self.inner.forget(stream).await
    }
}

async fn publish_pair(engine: &EmbeddedMade, parent: &CeremonyDefinition) {
    engine
        .publish_definition(definition(CHILD_YAML))
        .await
        .unwrap();
    engine.publish_definition(parent.clone()).await.unwrap();
}

fn step_input(ceremony_id: &CeremonyId, key: &str) -> RunCeremonyStepInput {
    RunCeremonyStepInput::new(
        ceremony_id.clone(),
        RoleId::new(if key == "delegate" { "PARENT" } else { "CHILD" }).unwrap(),
        AuditActorKind::Agent,
        StepId::new(key).unwrap(),
        LeaseOwnerId::new("children-proof").unwrap(),
        IdempotencyKey::new(format!("{ceremony_id}-{key}")).unwrap(),
        DurationMs::from_millis(30_000),
    )
}

fn transition_input(ceremony_id: &CeremonyId, role: &str) -> ApplyCeremonyTransitionInput {
    ApplyCeremonyTransitionInput::new(
        ceremony_id.clone(),
        RoleId::new(role).unwrap(),
        AuditActorKind::Agent,
        TransitionTrigger::new("finish").unwrap(),
    )
}

async fn facade_complete_child(engine: &EmbeddedMade, child_id: &CeremonyId) -> EventId {
    engine.run_step(step_input(child_id, "work")).await.unwrap();
    engine
        .apply_transition(transition_input(child_id, "CHILD"))
        .await
        .unwrap();
    engine
        .audit_records(child_id)
        .await
        .unwrap()
        .into_iter()
        .find(|record| matches!(record.event(), Some(CeremonyEvent::CeremonyCompleted(_))))
        .expect("child terminal should be sealed")
        .event_id()
        .clone()
}

#[tokio::test]
async fn facade_sqlite_competing_planners_adopt_the_one_sealed_plan() {
    let directory = scratch();
    let path = directory.path().join("competing-plans.sqlite3");
    let pause = Arc::new(AppendBarrier::default());
    let raw_first = Arc::new(SqliteCeremonyStore::open(&path).unwrap());
    let first_store = Arc::new(PauseBeforeCompletionStore {
        inner: raw_first.clone(),
        barrier: pause.clone(),
        pause_on: PausedEvent::Plan,
    });
    let first = Arc::new(
        EmbeddedMade::builder()
            .with_ceremony_store(first_store)
            .with_definition_publications(raw_first)
            .with_clock(Arc::new(ParityClock::at(PARITY_INSTANT)))
            .with_step_handler(ParityStepHandler::shared())
            .build(),
    );
    let raw_second = Arc::new(SqliteCeremonyStore::open(&path).unwrap());
    let second = EmbeddedMade::builder()
        .with_ceremony_store(raw_second.clone())
        .with_definition_publications(raw_second)
        .with_clock(Arc::new(ParityClock::at(
            PARITY_INSTANT + time::Duration::seconds(1),
        )))
        .with_step_handler(ParityStepHandler::shared())
        .build();
    let parent = definition(&parent_yaml("all", 1));
    publish_pair(&first, &parent).await;
    let parent_id = CeremonyId::new("sqlite-competing-plans").unwrap();
    first
        .start_published(StartCeremonyInput::new(
            parent_id.clone(),
            parent.name().clone(),
            parent.version().clone(),
            CeremonyContext::empty(),
            "operator",
            AuditActorKind::Service,
        ))
        .await
        .unwrap();
    let claim = first
        .start_step(StartCeremonyStepInput::new(
            parent_id.clone(),
            RoleId::new("PARENT").unwrap(),
            AuditActorKind::Agent,
            StepId::new("delegate").unwrap(),
            LeaseOwnerId::new("shared-owner").unwrap(),
            IdempotencyKey::new("shared-key").unwrap(),
            DurationMs::from_millis(30_000),
        ))
        .await
        .unwrap();
    let input = PrepareCeremonyChildrenInput::new(
        parent_id.clone(),
        StepId::new("delegate").unwrap(),
        claim.claim_fence().clone(),
        AuditActorKind::Agent,
    );
    let first_writer = {
        let first = first.clone();
        let input = input.clone();
        tokio::spawn(async move { first.prepare_children(input).await })
    };
    tokio::time::timeout(std::time::Duration::from_secs(5), pause.wait_until_paused())
        .await
        .expect("the first planner should pause before sealing its plan");
    let winning = second.prepare_children(input).await;
    pause.resume();
    let adopted = tokio::time::timeout(std::time::Duration::from_secs(5), first_writer)
        .await
        .expect("the losing planner should finish after adopting the winner")
        .unwrap();
    assert!(winning.is_ok(), "winning planner failed: {winning:?}");
    assert!(adopted.is_ok(), "losing planner failed: {adopted:?}");
    assert_eq!(
        winning.unwrap().child_ids(),
        adopted.unwrap().child_ids(),
        "both callers must return the child ids from the sealed plan"
    );
    let records = second.audit_records(&parent_id).await.unwrap();
    assert_eq!(
        records
            .iter()
            .filter(|record| matches!(record.event(), Some(CeremonyEvent::ChildSpawnPlanned(_))))
            .count(),
        1
    );
    assert_eq!(
        records
            .iter()
            .filter(|record| matches!(record.event(), Some(CeremonyEvent::StepCompleted(_))))
            .count(),
        1
    );
}

#[tokio::test]
async fn a_child_plan_sealed_before_pause_finishes_opening_and_adoption() {
    let directory = scratch();
    let path = directory.path().join("pause-after-plan.sqlite3");
    let pause = Arc::new(AppendBarrier::default());
    let raw = Arc::new(SqliteCeremonyStore::open(&path).unwrap());
    let delayed_store = Arc::new(PauseBeforeCompletionStore {
        inner: raw.clone(),
        barrier: pause.clone(),
        pause_on: PausedEvent::SealedPlan,
    });
    let worker = Arc::new(
        EmbeddedMade::builder()
            .with_ceremony_store(delayed_store)
            .with_definition_publications(raw)
            .with_clock(ParityClock::shared())
            .with_step_handler(ParityStepHandler::shared())
            .build(),
    );
    let controller = durable_engine(&path);
    let parent = definition(&parent_yaml("all", 1));
    publish_pair(&worker, &parent).await;
    let parent_id = CeremonyId::new("pause-after-plan").unwrap();
    worker
        .start_published(StartCeremonyInput::new(
            parent_id.clone(),
            parent.name().clone(),
            parent.version().clone(),
            CeremonyContext::empty(),
            "operator",
            AuditActorKind::Service,
        ))
        .await
        .unwrap();
    let claim = worker
        .start_step(StartCeremonyStepInput::new(
            parent_id.clone(),
            RoleId::new("PARENT").unwrap(),
            AuditActorKind::Agent,
            StepId::new("delegate").unwrap(),
            LeaseOwnerId::new("worker").unwrap(),
            IdempotencyKey::new("accepted-before-pause").unwrap(),
            DurationMs::from_millis(30_000),
        ))
        .await
        .unwrap();
    let input = PrepareCeremonyChildrenInput::new(
        parent_id.clone(),
        StepId::new("delegate").unwrap(),
        claim.claim_fence().clone(),
        AuditActorKind::Agent,
    );
    let preparing = {
        let worker = worker.clone();
        tokio::spawn(async move { worker.prepare_children(input).await })
    };
    tokio::time::timeout(std::time::Duration::from_secs(5), pause.wait_until_paused())
        .await
        .expect("the child plan should be sealed before the worker pauses");
    controller
        .pause_ceremony(PauseCeremonyInput::new(
            parent_id.clone(),
            "operator",
            AuditActorKind::Service,
            LifecycleReason::new("maintenance").unwrap(),
        ))
        .await
        .unwrap();
    pause.resume();
    let prepared = tokio::time::timeout(std::time::Duration::from_secs(5), preparing)
        .await
        .expect("accepted child work should drain")
        .unwrap()
        .expect("the sealed plan should finish adoption while paused");
    assert_eq!(prepared.child_ids().len(), 1);

    let reopened = durable_engine(&path);
    let parent = reopened.instance(&parent_id).await.unwrap();
    assert!(parent.is_paused());
    let group = parent.child_groups().values().next().unwrap();
    assert_eq!(group.adopted_claim_fence(), claim.claim_fence());
    assert!(reopened.instance(&prepared.child_ids()[0]).await.is_ok());
}

#[tokio::test]
#[allow(clippy::too_many_lines)] // one durable two-writer scenario through completion and reopen
async fn facade_sqlite_competing_writers_reopen_one_all_join_without_duplicates() {
    let directory = scratch();
    let path = directory.path().join("children.sqlite3");
    let pause = Arc::new(AppendBarrier::default());
    let raw_first = Arc::new(SqliteCeremonyStore::open(&path).unwrap());
    let first_store = Arc::new(PauseBeforeCompletionStore {
        inner: raw_first.clone(),
        barrier: pause.clone(),
        pause_on: PausedEvent::Completion,
    });
    let first = Arc::new(
        EmbeddedMade::builder()
            .with_ceremony_store(first_store)
            .with_definition_publications(raw_first)
            .with_clock(ParityClock::shared())
            .with_step_handler(ParityStepHandler::shared())
            .build(),
    );
    let second = durable_engine(&path);
    let parent = definition(&parent_yaml("all", 2));
    publish_pair(&first, &parent).await;
    let parent_id = CeremonyId::new("sqlite-parent").unwrap();
    first
        .start_published(StartCeremonyInput::new(
            parent_id.clone(),
            parent.name().clone(),
            parent.version().clone(),
            CeremonyContext::empty(),
            "operator",
            AuditActorKind::Service,
        ))
        .await
        .unwrap();

    let first_writer = {
        let first = first.clone();
        let input = step_input(&parent_id, "delegate");
        tokio::spawn(async move { first.run_step(input).await })
    };
    tokio::time::timeout(std::time::Duration::from_secs(5), pause.wait_until_paused())
        .await
        .expect("the prepare writer should pause before completing the spawn step");
    let recovered = second
        .recover_children(CeremonyEventPageLimit::DEFAULT)
        .await;
    pause.resume();
    let prepared = tokio::time::timeout(std::time::Duration::from_secs(5), first_writer)
        .await
        .expect("the paused prepare writer should finish after release")
        .unwrap();
    assert!(recovered.is_ok(), "recovery writer failed: {recovered:?}");
    assert!(prepared.is_ok(), "prepare writer failed: {prepared:?}");

    drop(first);
    drop(second);
    let reopened = durable_engine(&path);
    let instance = reopened.instance(&parent_id).await.unwrap();
    assert_eq!(instance.child_groups().len(), 1);
    let group = instance.child_groups().values().next().unwrap();
    assert_eq!(group.plan().children().len(), 2);
    let child_ids = group
        .plan()
        .children()
        .iter()
        .map(|child| child.child_id().clone())
        .collect::<Vec<_>>();
    let records = reopened.audit_records(&parent_id).await.unwrap();
    assert_eq!(
        records
            .iter()
            .filter(|record| matches!(record.event(), Some(CeremonyEvent::ChildSpawnPlanned(_))))
            .count(),
        1
    );

    let first_terminal = facade_complete_child(&reopened, &child_ids[0]).await;
    reopened
        .accept_child_completion(AcceptChildCompletionInput::new(
            child_ids[0].clone(),
            first_terminal.clone(),
        ))
        .await
        .unwrap();
    let duplicate = reopened
        .accept_child_completion(AcceptChildCompletionInput::new(
            child_ids[0].clone(),
            first_terminal,
        ))
        .await
        .unwrap();
    assert_eq!(
        duplicate
            .parent()
            .child_groups()
            .values()
            .next()
            .unwrap()
            .completions()
            .len(),
        1
    );
    assert!(reopened
        .apply_transition(transition_input(&parent_id, "PARENT"))
        .await
        .is_err());

    let second_terminal = facade_complete_child(&reopened, &child_ids[1]).await;
    let foreign = reopened
        .accept_child_completion(AcceptChildCompletionInput::new(
            child_ids[0].clone(),
            second_terminal.clone(),
        ))
        .await
        .unwrap_err();
    assert!(foreign.to_string().contains("terminal"), "{foreign}");
    reopened
        .accept_child_completion(AcceptChildCompletionInput::new(
            child_ids[1].clone(),
            second_terminal,
        ))
        .await
        .unwrap();
    let completed = reopened
        .apply_transition(transition_input(&parent_id, "PARENT"))
        .await
        .unwrap();
    assert!(completed.is_completed(&parent));

    drop(reopened);
    let reopened = durable_engine(&path);
    let final_instance = reopened.instance(&parent_id).await.unwrap();
    assert!(final_instance.is_completed(&parent));
    assert_eq!(
        final_instance
            .child_groups()
            .values()
            .next()
            .unwrap()
            .completions()
            .len(),
        2
    );
    let accepted = reopened
        .audit_records(&parent_id)
        .await
        .unwrap()
        .into_iter()
        .filter(|record| {
            matches!(
                record.event(),
                Some(CeremonyEvent::ChildCompletionAccepted(_))
            )
        })
        .count();
    assert_eq!(accepted, 2, "duplicate or refused locators appended a fact");
}

async fn grpc_fixture() -> GrpcFixture {
    GrpcFixture::start_with(
        GrpcFixtureWiring::new()
            .with_clock(ParityClock::shared())
            .with_step_handler(ParityStepHandler::shared()),
    )
    .await
}

async fn rpc_publish(client: &mut MadeServiceClient<tonic::transport::Channel>, raw: &str) {
    client
        .publish_ceremony_definition(PublishCeremonyDefinitionRequest {
            definition_yaml: raw.to_owned(),
        })
        .await
        .unwrap();
}

async fn rpc_complete_child(
    client: &mut MadeServiceClient<tonic::transport::Channel>,
    child_id: &str,
) -> String {
    client
        .run_ceremony_step(RunCeremonyStepRequest {
            ceremony_id: child_id.to_owned(),
            step_id: "work".into(),
            actor_kind: "agent".into(),
            lease_owner_id: "rpc-proof".into(),
            idempotency_key: format!("{child_id}-work"),
            lease_ttl_ms: 30_000,
        })
        .await
        .unwrap();
    client
        .apply_ceremony_transition(ApplyCeremonyTransitionRequest {
            ceremony_id: child_id.to_owned(),
            trigger: "finish".into(),
            actor_kind: "agent".into(),
        })
        .await
        .unwrap();
    client
        .read_ceremony_events(ReadCeremonyEventsRequest {
            ceremony_id: child_id.to_owned(),
            from_version: 0,
            limit: 0,
        })
        .await
        .unwrap()
        .into_inner()
        .records
        .into_iter()
        .find(|record| record.event_type == "ceremony_completed")
        .expect("child terminal should cross gRPC")
        .event_id
}

#[tokio::test]
async fn direct_rpc_any_join_stays_terminal_when_the_late_sibling_arrives() {
    let fixture = grpc_fixture().await;
    let mut client = MadeServiceClient::new(fixture.channel.clone());
    let parent_raw = parent_yaml("any", 2);
    rpc_publish(&mut client, CHILD_YAML).await;
    rpc_publish(&mut client, &parent_raw).await;
    let parent_id = "rpc-any-parent";
    client
        .start_published_ceremony(StartPublishedCeremonyRequest {
            ceremony_id: parent_id.into(),
            ceremony: "parent_any".into(),
            version: "1.0".into(),
            actor_id: "operator".into(),
            actor_kind: "service".into(),
            context: Some(prost_types::Struct {
                fields: std::collections::BTreeMap::default(),
            }),
        })
        .await
        .unwrap();
    let prepared = client
        .prepare_ceremony_children(PrepareCeremonyChildrenRequest {
            ceremony_id: parent_id.into(),
            step_id: "delegate".into(),
            lease_owner_id: "rpc-proof".into(),
            idempotency_key: "rpc-any-spawn".into(),
            lease_ttl_ms: 30_000,
            actor_kind: "agent".into(),
        })
        .await
        .unwrap()
        .into_inner();
    assert_eq!(prepared.child_ids.len(), 2);
    let first_terminal = rpc_complete_child(&mut client, &prepared.child_ids[0]).await;
    let accepted = client
        .accept_child_completion(AcceptChildCompletionRequest {
            child_id: prepared.child_ids[0].clone(),
            terminal_event_id: first_terminal.clone(),
        })
        .await
        .unwrap()
        .into_inner();
    assert_eq!(
        accepted.parent.as_ref().unwrap().child_groups[0]
            .completions
            .len(),
        1
    );
    let replayed = client
        .accept_child_completion(AcceptChildCompletionRequest {
            child_id: prepared.child_ids[0].clone(),
            terminal_event_id: first_terminal,
        })
        .await
        .unwrap()
        .into_inner();
    assert_eq!(
        replayed.parent.as_ref().unwrap().child_groups[0]
            .completions
            .len(),
        1
    );
    let completed = client
        .apply_ceremony_transition(ApplyCeremonyTransitionRequest {
            ceremony_id: parent_id.into(),
            trigger: "finish".into(),
            actor_kind: "agent".into(),
        })
        .await
        .unwrap()
        .into_inner()
        .instance
        .unwrap();
    assert!(completed.completed);

    let second_terminal = rpc_complete_child(&mut client, &prepared.child_ids[1]).await;
    let foreign = client
        .accept_child_completion(AcceptChildCompletionRequest {
            child_id: prepared.child_ids[0].clone(),
            terminal_event_id: second_terminal.clone(),
        })
        .await
        .unwrap_err();
    assert_eq!(foreign.code(), tonic::Code::NotFound);
    let late = client
        .accept_child_completion(AcceptChildCompletionRequest {
            child_id: prepared.child_ids[1].clone(),
            terminal_event_id: second_terminal,
        })
        .await
        .unwrap()
        .into_inner()
        .parent
        .unwrap();
    assert!(
        late.completed,
        "a late accepted fact reverted terminal state"
    );
    assert_eq!(late.child_groups[0].completions.len(), 2);
}

async fn mcp_call(server: &MadeMcpServer, name: &str, arguments: Value) -> Value {
    let request = json!({
        "jsonrpc":"2.0", "id":1, "method":"tools/call",
        "params":{"name":name,"arguments":arguments}
    });
    serde_json::from_str(&server.handle_json_line(&request.to_string()).await.unwrap()).unwrap()
}

async fn mcp_both(servers: &[MadeMcpServer; 2], name: &str, arguments: Value) -> Value {
    let remote = mcp_call(&servers[0], name, arguments.clone()).await;
    let embedded = mcp_call(&servers[1], name, arguments).await;
    assert_eq!(
        remote["result"]["isError"], embedded["result"]["isError"],
        "{name}"
    );
    assert_eq!(
        without_transport_trace(remote["result"]["structuredContent"].clone()),
        without_transport_trace(embedded["result"]["structuredContent"].clone()),
        "{name}"
    );
    embedded
}

fn without_transport_trace(value: Value) -> Value {
    match value {
        Value::Array(values) => {
            Value::Array(values.into_iter().map(without_transport_trace).collect())
        }
        Value::Object(mut fields) => {
            fields.remove("trace_id");
            fields.remove("record_hash");
            fields.remove("previous_record_hash");
            fields.remove("terminal_record_hash");
            Value::Object(
                fields
                    .into_iter()
                    .map(|(key, value)| (key, without_transport_trace(value)))
                    .collect(),
            )
        }
        scalar => scalar,
    }
}

fn structured(answer: &Value) -> &Value {
    &answer["result"]["structuredContent"]
}

async fn mcp_complete_child(servers: &[MadeMcpServer; 2], child_id: &str) -> String {
    mcp_both(
        servers,
        "made_run_ceremony_step",
        json!({
            "ceremony_id":child_id, "step_id":"work", "actor_kind":"agent",
            "lease_owner_id":"mcp-proof", "idempotency_key":format!("{child_id}-work")
        }),
    )
    .await;
    mcp_both(
        servers,
        "made_apply_ceremony_transition",
        json!({"ceremony_id":child_id,"trigger":"finish","actor_kind":"agent"}),
    )
    .await;
    let history = mcp_both(
        servers,
        "made_read_ceremony_events",
        json!({"ceremony_id":child_id}),
    )
    .await;
    structured(&history)["records"]
        .as_array()
        .unwrap()
        .iter()
        .find(|record| record["event_type"] == "ceremony_completed")
        .unwrap()["event_id"]
        .as_str()
        .unwrap()
        .to_owned()
}

#[tokio::test]
async fn grpc_and_embedded_mcp_agree_on_quorum_dedup_and_refusal() {
    let directory = scratch();
    let path = directory.path().join("mcp.sqlite3");
    let fixture = grpc_fixture().await;
    let servers = [
        MadeMcpServer::with_backend(GrpcMadeMcpBackend::new(
            format!("http://{}", fixture.addr),
            MadeMcpGrpcTlsConfig::disabled(),
        )),
        MadeMcpServer::with_backend(EmbeddedMadeMcpBackend::new(durable_engine(&path))),
    ];
    let parent_raw = parent_yaml("quorum:2", 3);
    for raw in [CHILD_YAML, parent_raw.as_str()] {
        let published = mcp_both(
            &servers,
            "made_publish_ceremony_definition",
            json!({"definition_yaml":raw}),
        )
        .await;
        assert_eq!(structured(&published)["outcome"], "published");
    }
    let parent_id = "mcp-quorum-parent";
    mcp_both(
        &servers,
        "made_start_published_ceremony",
        json!({
            "ceremony_id":parent_id, "ceremony":"parent_quorum_2", "version":"1.0",
            "actor_id":"operator", "actor_kind":"service", "context":{}
        }),
    )
    .await;
    let prepared = mcp_both(
        &servers,
        "made_prepare_ceremony_children",
        json!({
            "ceremony_id":parent_id, "step_id":"delegate", "actor_kind":"agent",
            "lease_owner_id":"mcp-proof", "idempotency_key":"mcp-quorum-spawn"
        }),
    )
    .await;
    let child_ids = structured(&prepared)["child_ids"]
        .as_array()
        .unwrap()
        .iter()
        .map(|value| value.as_str().unwrap().to_owned())
        .collect::<Vec<_>>();
    assert_eq!(child_ids.len(), 3);

    let first_terminal = mcp_complete_child(&servers, &child_ids[0]).await;
    let accepted = mcp_both(
        &servers,
        "made_accept_child_completion",
        json!({"child_id":child_ids[0],"terminal_event_id":first_terminal}),
    )
    .await;
    assert_eq!(
        structured(&accepted)["parent"]["child_groups"][0]["completions"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
    let blocked = mcp_both(
        &servers,
        "made_apply_ceremony_transition",
        json!({"ceremony_id":parent_id,"trigger":"finish","actor_kind":"agent"}),
    )
    .await;
    assert_eq!(blocked["result"]["isError"], true);

    let second_terminal = mcp_complete_child(&servers, &child_ids[1]).await;
    let refused = mcp_both(
        &servers,
        "made_accept_child_completion",
        json!({"child_id":child_ids[0],"terminal_event_id":second_terminal}),
    )
    .await;
    assert_eq!(refused["result"]["isError"], true);
    let accepted = mcp_both(
        &servers,
        "made_accept_child_completion",
        json!({"child_id":child_ids[1],"terminal_event_id":second_terminal}),
    )
    .await;
    assert_eq!(
        structured(&accepted)["parent"]["child_groups"][0]["completions"]
            .as_array()
            .unwrap()
            .len(),
        2
    );
    let completed = mcp_both(
        &servers,
        "made_apply_ceremony_transition",
        json!({"ceremony_id":parent_id,"trigger":"finish","actor_kind":"agent"}),
    )
    .await;
    assert_eq!(structured(&completed)["completed"], true);
}
