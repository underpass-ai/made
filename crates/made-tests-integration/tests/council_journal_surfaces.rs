//! The two MCP routes operate the same durable council cursor over separate handles.
use made_adapters::sqlite::{SqliteCouncilJournal, SqliteCouncilRegistry, SqliteCouncilStore};
use made_app::services::AuthorizationOperationScope;
use made_core::entities::{Council, CouncilJournalEvent};
use made_core::events::{EventEnvelope, PhaseChangedEvent};
use made_core::ports::{CouncilJournalPort, CouncilRegistryPort};
use made_core::value_objects::{
    AgentId, AuthenticatedPrincipal, AuthenticationMethod, AuthorizationEvidence,
    AuthorizedOperation, CouncilId, EventId, PrincipalId, PrincipalKind, Specialty, TaskId,
};
use made_embedded::EmbeddedMade;
use made_mcp::backend::{MadeMcpGrpcTlsConfig, MadeMcpToolBackend};
use made_mcp::{EmbeddedMadeMcpBackend, GrpcMadeMcpBackend};
use made_tests_integration::grpc_fixture::{GrpcFixture, GrpcFixtureWiring};
use serde_json::{json, Value};
use std::sync::Arc;

async fn call(backend: &dyn MadeMcpToolBackend, name: &str, args: Value) -> Value {
    backend
        .call_tool(name, &args)
        .await
        .unwrap_or_else(|error| panic!("{name}: {error}"))["structuredContent"]
        .clone()
}
fn journal(path: &std::path::Path) -> Arc<SqliteCouncilJournal> {
    Arc::new(SqliteCouncilJournal::new(
        SqliteCouncilStore::open(path).unwrap(),
    ))
}
fn authorization(request: &str) -> AuthorizedOperation {
    let principal = AuthenticatedPrincipal::new(
        PrincipalId::new("council-surface-host").unwrap(),
        PrincipalKind::TrustedHost,
        AuthenticationMethod::MutualTls,
    )
    .unwrap();
    let evidence: AuthorizationEvidence = serde_json::from_value(json!({
        "decision_id": "a".repeat(64),
        "request_id": request,
        "principal_id": "council-surface-host",
        "action": "process_trigger_event",
        "scope": {"kind":"global"},
        "target_digest": "b".repeat(64),
        "policy_version": 1,
        "admitted_at": "2026-09-19T12:00:00Z",
        "valid_until": "2026-09-19T12:01:00Z"
    }))
    .unwrap();
    AuthorizedOperation::new(principal, evidence).unwrap()
}
async fn clients(
    path: &std::path::Path,
) -> (GrpcFixture, GrpcMadeMcpBackend, EmbeddedMadeMcpBackend) {
    let fixture =
        GrpcFixture::start_with(GrpcFixtureWiring::new().with_council_journal(journal(path))).await;
    let remote = GrpcMadeMcpBackend::new(
        format!("http://{}", fixture.addr),
        MadeMcpGrpcTlsConfig::disabled(),
    );
    let embedded = EmbeddedMadeMcpBackend::new(
        EmbeddedMade::builder()
            .with_council_journal(journal(path))
            .build(),
    );
    (fixture, remote, embedded)
}
async fn seed(path: &std::path::Path) {
    let store = SqliteCouncilStore::open(path).unwrap();
    let now = time::OffsetDateTime::now_utc();
    AuthorizationOperationScope::run(
        authorization("register-council-surface"),
        SqliteCouncilRegistry::new(store.clone()).register(
            Council::new(
                CouncilId::new("research").unwrap(),
                Specialty::new("research").unwrap(),
                [AgentId::new("writer").unwrap()],
                now,
            )
            .unwrap(),
        ),
    )
    .await
    .unwrap();
    AuthorizationOperationScope::run(
        authorization("publish-council-surface"),
        SqliteCouncilJournal::new(store).publish(CouncilJournalEvent::PhaseChanged(
            PhaseChangedEvent::new(
                EventEnvelope::new(EventId::new("phase-1").unwrap(), now, "fixture", None).unwrap(),
                TaskId::new("task-1").unwrap(),
                "proposing",
                "reviewing",
            )
            .unwrap(),
        )),
    )
    .await
    .unwrap();
}

#[tokio::test]
async fn public_consumers_share_fencing_and_resume_after_restart_without_changing_ceremony_cursor()
{
    let scratch = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tmp");
    std::fs::create_dir_all(&scratch).unwrap();
    let directory = tempfile::tempdir_in(scratch).unwrap();
    let path = directory.path().join("councils.sqlite3");
    seed(&path).await;
    let (fixture, remote, embedded) = clients(&path).await;
    let first = call(&embedded, "made_read_council_events", json!({"limit":1})).await;
    assert_eq!(
        first,
        call(&remote, "made_read_council_events", json!({"limit":1})).await
    );
    assert_eq!(first["records"][0]["event"]["kind"], "council_registered");
    assert_eq!(
        first["records"][0]["authorization"]["principal_id"],
        "council-surface-host"
    );
    assert_eq!(first["next_after"], 1);
    let consumer = json!({"consumer":"public-reader"});
    assert_eq!(
        call(&remote, "made_get_council_event_cursor", consumer.clone()).await,
        json!({"acknowledged_through":null})
    );
    let lease_args = json!({"consumer":"public-reader","duration_ms":30_000});
    let lease =
        call(&embedded, "made_lease_council_events", lease_args.clone()).await["lease"].clone();
    assert!(lease.is_object());
    assert_eq!(
        call(&remote, "made_lease_council_events", lease_args.clone()).await,
        json!({"lease":null})
    );
    let mut wrong = lease.clone();
    wrong["id"] = json!("another-owner");
    assert!(remote
        .call_tool(
            "made_acknowledge_council_events",
            &json!({"lease":wrong,"through":1})
        )
        .await
        .is_err());
    assert_eq!(
        call(&embedded, "made_get_council_event_cursor", consumer.clone()).await,
        json!({"acknowledged_through":null})
    );
    call(
        &remote,
        "made_acknowledge_council_events",
        json!({"lease":lease,"through":1}),
    )
    .await;
    assert_eq!(
        call(&embedded, "made_get_council_event_cursor", consumer.clone()).await,
        json!({"acknowledged_through":1})
    );
    let second_lease =
        call(&remote, "made_lease_council_events", lease_args).await["lease"].clone();
    call(
        &embedded,
        "made_release_council_events",
        json!({"lease":second_lease}),
    )
    .await;
    assert_eq!(
        call(&remote, "made_get_council_event_cursor", consumer.clone()).await,
        json!({"acknowledged_through":1})
    );
    let ceremony = call(
        &remote,
        "made_pull_ceremony_events",
        json!({"consumer":"public-reader","limit":1}),
    )
    .await;
    assert_eq!(ceremony["acknowledged_through"], Value::Null);
    assert!(ceremony["records"].as_array().unwrap().is_empty());
    drop((fixture, remote, embedded));
    let (_fixture, remote, embedded) = clients(&path).await;
    assert_eq!(
        call(&remote, "made_get_council_event_cursor", consumer).await,
        json!({"acknowledged_through":1})
    );
    let second = call(
        &embedded,
        "made_read_council_events",
        json!({"after":1,"limit":1}),
    )
    .await;
    assert_eq!(
        second,
        call(
            &remote,
            "made_read_council_events",
            json!({"after":1,"limit":1})
        )
        .await
    );
    assert_eq!(second["records"][0]["event"]["kind"], "phase_changed");
    assert_eq!(
        second["records"][0]["authorization"]["principal_id"],
        "council-surface-host"
    );
    assert_eq!(second["next_after"], 2);
    assert_eq!(
        call(&remote, "made_read_council_events", json!({"after":2})).await,
        json!({"records":[],"next_after":2})
    );
    let invalid = remote
        .call_tool("made_read_council_events", &json!({"limit":1001}))
        .await;
    assert!(invalid.is_err());
}
