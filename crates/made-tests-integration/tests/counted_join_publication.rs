//! The authoring example must publish, survive SQLite reopen and execute its
//! counted join on every surface. No digest/shape exceptions between engines.

use std::path::Path;
use std::sync::Arc;

use made_adapters::sqlite::SqliteCeremonyStore;
use made_adapters::yaml::CeremonyDefinitionYaml;
use made_app::usecases::{ApplyCeremonyTransitionInput, RunCeremonyStepInput, StartCeremonyInput};
use made_core::entities::CeremonyDefinition;
use made_core::value_objects::{
    Attributes, AuditActorKind, CeremonyContext, CeremonyId, DurationMs, IdempotencyKey,
    LeaseOwnerId, RoleId, StepId, TransitionTrigger,
};
use made_embedded::EmbeddedMade;
use made_mcp::backend::MadeMcpGrpcTlsConfig;
use made_mcp::{EmbeddedMadeMcpBackend, GrpcMadeMcpBackend, MadeMcpServer};
use made_proto::v1::made_service_client::MadeServiceClient;
use made_proto::v1::{
    ApplyCeremonyTransitionRequest, PublishCeremonyDefinitionRequest, RunCeremonyStepRequest,
    StartPublishedCeremonyRequest,
};
use made_tests_integration::grpc_fixture::{GrpcFixture, GrpcFixtureWiring};
use made_tests_integration::parity_clock::ParityClock;
use made_tests_integration::parity_step_handler::ParityStepHandler;
use serde_json::{json, Value};

const CEREMONY: &str = include_str!("counted_join_publication/two_checks.yaml");
const DIGEST: &str = "884eacb95ca9895c0aa728d31e8329f240ce4f9720b3d458ea1ed2c207dc039e";
const SESSION: &str = "counted-join-session";
const STEPS: [(&str, &str); 2] = [
    ("inspect_api", "API_REVIEWER"),
    ("inspect_storage", "DATA_REVIEWER"),
];

fn definition() -> CeremonyDefinition {
    CeremonyDefinitionYaml::parse_str(CEREMONY).unwrap()
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

async fn fixture() -> GrpcFixture {
    GrpcFixture::start_with(
        GrpcFixtureWiring::new()
            .with_clock(ParityClock::shared())
            .with_step_handler(ParityStepHandler::shared()),
    )
    .await
}

fn scratch() -> tempfile::TempDir {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tmp");
    std::fs::create_dir_all(&path).unwrap();
    tempfile::tempdir_in(path).unwrap()
}

async fn call(server: &MadeMcpServer, name: &str, arguments: Value) -> Value {
    let request = json!({"jsonrpc":"2.0", "id":1, "method":"tools/call",
        "params":{"name":name,"arguments":arguments,
            "_meta":{"traceparent":"00-0123456789abcdef0123456789abcdef-0123456789abcdef-01"}}});
    let response = server.handle_json_line(&request.to_string()).await.unwrap();
    let answer: Value = serde_json::from_str(&response).unwrap();
    assert!(answer.get("error").is_none(), "{answer}");
    assert_ne!(answer["result"]["isError"], true, "{answer}");
    answer["result"].clone()
}

async fn both(servers: &[MadeMcpServer; 2], name: &str, args: Value) -> Value {
    let remote = call(&servers[0], name, args.clone()).await;
    let local = call(&servers[1], name, args).await;
    assert_eq!(remote, local, "{name}: every response field must agree");
    local["structuredContent"].clone()
}

#[tokio::test]
async fn counted_join_publishes_and_executes_with_exact_mcp_parity() {
    let directory = scratch();
    let path = directory.path().join("mcp.sqlite3");
    let fixture = fixture().await;
    let servers = [
        MadeMcpServer::with_backend(GrpcMadeMcpBackend::new(
            format!("http://{}", fixture.addr),
            MadeMcpGrpcTlsConfig::disabled(),
        )),
        MadeMcpServer::with_backend(EmbeddedMadeMcpBackend::new(durable_engine(&path))),
    ];
    let validated = both(
        &servers,
        "made_validate_ceremony_draft",
        json!({"definition_yaml":CEREMONY}),
    )
    .await;
    assert_eq!(validated["publishable"], true);
    let published = both(
        &servers,
        "made_publish_ceremony_definition",
        json!({"definition_yaml":CEREMONY}),
    )
    .await;
    assert_eq!(published["outcome"], "published");
    assert_eq!(
        published["digest"],
        definition().digest().unwrap().to_string()
    );
    let again = both(
        &servers,
        "made_publish_ceremony_definition",
        json!({"definition_yaml":CEREMONY}),
    )
    .await;
    assert_eq!(again["outcome"], "already_published");
    assert_eq!(again["digest"], published["digest"]);
    let started = both(&servers, "made_start_published_ceremony", json!({
        "ceremony_id":SESSION, "ceremony":"two_checks", "version":"1.0",
        "actor_id":"operator", "actor_kind":"service", "context":{"change_summary":"Inspect a change"}
    })).await;
    assert_eq!(started["transitions"][0]["enabled"], false);
    for (index, (step, _)) in STEPS.iter().enumerate() {
        let progressed = both(
            &servers,
            "made_run_ceremony_step",
            json!({
                "ceremony_id":SESSION, "step_id":step, "actor_kind":"agent",
                "lease_owner_id":"counted-host", "idempotency_key":step, "lease_ttl_ms":30_000
            }),
        )
        .await;
        assert_eq!(progressed["transitions"][0]["enabled"], index == 1);
    }
    let completed = both(
        &servers,
        "made_apply_ceremony_transition",
        json!({
            "ceremony_id":SESSION, "trigger":"finish", "actor_kind":"agent"
        }),
    )
    .await;
    assert_eq!(completed["completed"], true);
    drop(servers);
    assert_reopened(&path).await;
}

#[tokio::test]
async fn direct_rpc_publishes_binds_digest_and_waits_for_both_steps() {
    let fixture = fixture().await;
    let mut client = MadeServiceClient::new(fixture.channel.clone());
    let published = client
        .publish_ceremony_definition(PublishCeremonyDefinitionRequest {
            definition_yaml: CEREMONY.into(),
        })
        .await
        .unwrap()
        .into_inner();
    assert_eq!(published.outcome, "published");
    assert_eq!(published.digest, DIGEST);
    assert_eq!(published.digest, definition().digest().unwrap().to_string());
    let context = prost_types::Struct {
        fields: std::collections::BTreeMap::from([(
            "change_summary".into(),
            prost_types::Value {
                kind: Some(prost_types::value::Kind::StringValue(
                    "Inspect a change".into(),
                )),
            },
        )]),
    };
    let started = client
        .start_published_ceremony(StartPublishedCeremonyRequest {
            ceremony_id: SESSION.into(),
            ceremony: "two_checks".into(),
            version: "1.0".into(),
            actor_id: "operator".into(),
            actor_kind: "service".into(),
            context: Some(context),
        })
        .await
        .unwrap()
        .into_inner()
        .instance
        .unwrap();
    assert!(!started.transitions[0].enabled);
    assert_eq!(started.bound_definition_digest, published.digest);
    for (index, (step, _)) in STEPS.iter().enumerate() {
        let progressed = client
            .run_ceremony_step(RunCeremonyStepRequest {
                ceremony_id: SESSION.into(),
                step_id: (*step).into(),
                actor_kind: "agent".into(),
                lease_owner_id: "counted-host".into(),
                idempotency_key: (*step).into(),
                lease_ttl_ms: 30_000,
            })
            .await
            .unwrap()
            .into_inner()
            .instance
            .unwrap();
        assert_eq!(progressed.transitions[0].enabled, index == 1);
    }
    let completed = client
        .apply_ceremony_transition(ApplyCeremonyTransitionRequest {
            ceremony_id: SESSION.into(),
            trigger: "finish".into(),
            actor_kind: "agent".into(),
        })
        .await
        .unwrap()
        .into_inner()
        .instance
        .unwrap();
    assert!(completed.completed);
    assert_eq!(completed.current_state, "CLOSED");
}

fn transition() -> ApplyCeremonyTransitionInput {
    ApplyCeremonyTransitionInput::new(
        CeremonyId::new(SESSION).unwrap(),
        RoleId::new("API_REVIEWER").unwrap(),
        AuditActorKind::Agent,
        TransitionTrigger::new("finish").unwrap(),
    )
}

async fn facade_step(engine: &EmbeddedMade, step: &str, role: &str) {
    engine
        .run_step(RunCeremonyStepInput::new(
            CeremonyId::new(SESSION).unwrap(),
            RoleId::new(role).unwrap(),
            AuditActorKind::Agent,
            StepId::new(step).unwrap(),
            LeaseOwnerId::new("counted-host").unwrap(),
            IdempotencyKey::new(step).unwrap(),
            DurationMs::from_millis(30_000),
        ))
        .await
        .unwrap();
}

#[tokio::test]
async fn facade_reopens_publication_and_partial_join_before_finishing() {
    let directory = scratch();
    let path = directory.path().join("facade.sqlite3");
    let definition = definition();
    let engine = durable_engine(&path);
    let published = engine.publish_definition(definition.clone()).await.unwrap();
    assert!(published.is_new());
    assert_eq!(
        published.published().unwrap().digest(),
        definition.digest().unwrap()
    );
    // The definition cache is lost here. Starting must use the durable catalogue.
    drop(engine);
    let engine = durable_engine(&path);
    let repeated = engine.publish_definition(definition.clone()).await.unwrap();
    assert!(!repeated.is_new());
    assert!(!repeated.is_conflict());
    assert_eq!(
        repeated.published().unwrap().digest(),
        definition.digest().unwrap()
    );
    let changed = CeremonyDefinitionYaml::parse_str(
        &CEREMONY.replace("steps_completed:2", "steps_completed:1"),
    )
    .unwrap();
    assert!(engine
        .publish_definition(changed)
        .await
        .unwrap()
        .is_conflict());
    let started = engine
        .start_published(StartCeremonyInput::new(
            CeremonyId::new(SESSION).unwrap(),
            definition.name().clone(),
            definition.version().clone(),
            CeremonyContext::new(
                Attributes::new(
                    serde_json::from_value(json!({"change_summary":"Inspect a change"})).unwrap(),
                )
                .unwrap(),
            ),
            "operator",
            AuditActorKind::Service,
        ))
        .await
        .unwrap();
    assert_eq!(
        started.bound_definition(),
        Some(definition.digest().unwrap())
    );
    assert!(engine.apply_transition(transition()).await.is_err());
    facade_step(&engine, STEPS[0].0, STEPS[0].1).await;
    assert!(engine.apply_transition(transition()).await.is_err());
    drop(engine);
    let engine = durable_engine(&path);
    assert!(engine.apply_transition(transition()).await.is_err());
    facade_step(&engine, STEPS[1].0, STEPS[1].1).await;
    let completed = engine.apply_transition(transition()).await.unwrap();
    assert!(completed.is_completed(&definition));
    drop(engine);
    assert_reopened(&path).await;
}

async fn assert_reopened(path: &Path) {
    let engine = EmbeddedMade::open(path).unwrap();
    let expected = definition();
    let published = engine
        .published_definition(expected.name(), expected.version())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(published.definition(), &expected);
    assert_eq!(published.digest(), expected.digest().unwrap());
    assert_eq!(published.definition().digest().unwrap(), published.digest());
    let instance = engine
        .instance(&CeremonyId::new(SESSION).unwrap())
        .await
        .unwrap();
    assert_eq!(instance.bound_definition(), Some(published.digest()));
    assert!(instance.is_completed(published.definition()));
}
