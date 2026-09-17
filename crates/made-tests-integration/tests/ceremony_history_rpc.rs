//! What a session left behind, read over gRPC.
//!
//! The point of handing out *sealed records* rather than a rendering
//! of them is that a client can check them: read the answer back into
//! `AuditRecord`, run `AuditChain::verify`, and believe the stream
//! because the chain holds — not because the server said so. That is
//! what this test does, over the wire, through the tool a client
//! actually calls.

use made_core::entities::{AuditChain, AuditRecord};
use made_mcp::backend::{MadeMcpGrpcTlsConfig, MadeMcpToolBackend};
use made_mcp::protocol::ToolErrorCode;
use made_mcp::GrpcMadeMcpBackend;
use made_tests_integration::grpc_fixture::{GrpcFixture, GrpcFixtureWiring};
use made_tests_integration::parity_step_handler::ParityStepHandler;
use serde_json::{json, Value};

const HISTORY_CEREMONY: &str = r#"
version: "1.0"
name: "history_session"
states:
  - id: OPEN
    initial: true
  - id: DONE
    terminal: true
transitions:
  - from: OPEN
    to: DONE
    trigger: finish
    guards:
      - work_done
guards:
  work_done:
    type: automated
    check: "step_status:work:COMPLETED"
steps:
  - id: work
    state: OPEN
    handler: host_callback
roles:
  - id: FACILITATOR
    allowed_actions:
      - work
      - finish
"#;

const SESSION_ID: &str = "history-over-grpc";

fn structured(result: &Value) -> Value {
    result["structuredContent"].clone()
}

/// A server whose steps actually finish. The bundled handler
/// deliberates through an executor this fixture does not have, so a
/// step run against the default composition never reaches COMPLETED
/// and the guard that waits on it never opens.
async fn fixture() -> GrpcFixture {
    GrpcFixture::start_with(GrpcFixtureWiring::new().with_step_handler(ParityStepHandler::shared()))
        .await
}

/// A finished session, driven through the tools a client uses.
async fn session(remote: &GrpcMadeMcpBackend) {
    for (tool, arguments) in [
        (
            "made_start_ceremony",
            json!({
                "ceremony_id": SESSION_ID,
                "definition_yaml": HISTORY_CEREMONY,
                "actor_id": "operator-1",
                "actor_kind": "service",
                "context": { "incident_ref": "INC-9", "severity": 2 },
            }),
        ),
        (
            "made_run_ceremony_step",
            json!({
                "ceremony_id": SESSION_ID,
                "step_id": "work",
                "actor_kind": "agent",
                "lease_owner_id": "integration-host",
                "idempotency_key": "history-work-1",
            }),
        ),
        (
            "made_apply_ceremony_transition",
            json!({ "ceremony_id": SESSION_ID, "trigger": "finish", "actor_kind": "agent" }),
        ),
    ] {
        remote
            .call_tool(tool, &arguments)
            .await
            .unwrap_or_else(|error| panic!("`{tool}` should succeed: {error:?}"));
    }
}

fn records_from(answer: &Value) -> Vec<AuditRecord> {
    serde_json::from_value(answer["records"].clone())
        .expect("what the wire hands back must read back as the records it sealed")
}

#[tokio::test]
async fn a_client_can_verify_the_chain_it_was_handed_over_the_wire() {
    let fixture = fixture().await;
    let remote = GrpcMadeMcpBackend::new(
        format!("http://{}", fixture.addr),
        MadeMcpGrpcTlsConfig::disabled(),
    );
    session(&remote).await;

    let answer = structured(
        &remote
            .call_tool(
                "made_read_ceremony_events",
                &json!({ "ceremony_id": SESSION_ID }),
            )
            .await
            .expect("the stream should be readable"),
    );

    let records = records_from(&answer);
    assert!(
        records.len() >= 4,
        "a started, stepped and finished session seals more than a handful of records: {answer:#}"
    );
    assert_eq!(answer["record_count"], json!(records.len()));
    assert_eq!(answer["head_version"], json!(records.len()));
    assert_eq!(answer["next_version"], answer["head_version"]);
    assert_eq!(answer["has_more"], json!(false));

    // The whole point: the client checks the chain itself.
    let verdict = AuditChain::verify(&records);
    assert!(verdict.is_intact(), "{verdict:?}");

    // And every record still matches its own digest — which is what
    // makes the payload on the wire the payload that was sealed, not
    // a re-rendering of it.
    for record in &records {
        assert!(
            record
                .digest_is_intact()
                .expect("a digest can be recomputed"),
            "record {} arrived without its content: {record:?}",
            record.sequence().value()
        );
        assert!(
            record.event().is_some(),
            "a sealed record carries its event"
        );
    }
}

#[tokio::test]
async fn a_page_continues_where_the_last_one_ended() {
    let fixture = fixture().await;
    let remote = GrpcMadeMcpBackend::new(
        format!("http://{}", fixture.addr),
        MadeMcpGrpcTlsConfig::disabled(),
    );
    session(&remote).await;

    let read = |from: u64, limit: u64| {
        let remote = &remote;
        async move {
            structured(
                &remote
                    .call_tool(
                        "made_read_ceremony_events",
                        &json!({
                            "ceremony_id": SESSION_ID,
                            "from_version": from,
                            "limit": limit,
                        }),
                    )
                    .await
                    .expect("a page should be readable"),
            )
        }
    };

    let first = read(0, 2).await;
    assert_eq!(first["record_count"], json!(2));
    assert_eq!(first["next_version"], json!(2));
    assert_eq!(first["has_more"], json!(true));

    let mut seen = records_from(&first);
    let mut cursor = first["next_version"].as_u64().expect("a version");
    let head = first["head_version"].as_u64().expect("a head");
    while cursor < head {
        let page = read(cursor, 2).await;
        seen.extend(records_from(&page));
        cursor = page["next_version"].as_u64().expect("a version");
    }

    // Paged the whole way through: nothing seen twice, nothing missed,
    // and the chain still holds across the joins.
    assert_eq!(
        seen.iter()
            .map(|record| record.sequence().value())
            .collect::<Vec<_>>(),
        (1..=head).collect::<Vec<_>>()
    );
    assert!(AuditChain::verify(&seen).is_intact());

    // Past the end is caught up, not wrong.
    let beyond = read(head + 99, 10).await;
    assert_eq!(beyond["record_count"], json!(0));
    assert_eq!(beyond["next_version"], json!(head));
    assert_eq!(beyond["has_more"], json!(false));
}

/// The engine's verdict and the caller's own verdict on the same
/// records, side by side.
///
/// That is the whole claim of a verifiable journal: a client does not
/// have to take `intact: true` on trust, and this test does not either
/// — it reads the records back, runs the verifier itself, and compares.
#[tokio::test]
async fn the_engine_and_the_client_reach_the_same_verdict_on_the_same_journal() {
    let fixture = fixture().await;
    let remote = GrpcMadeMcpBackend::new(
        format!("http://{}", fixture.addr),
        MadeMcpGrpcTlsConfig::disabled(),
    );
    session(&remote).await;

    let verdict = structured(
        &remote
            .call_tool(
                "made_verify_ceremony_journal",
                &json!({ "ceremony_id": SESSION_ID }),
            )
            .await
            .expect("the journal should be verifiable"),
    );
    let page = structured(
        &remote
            .call_tool(
                "made_read_ceremony_events",
                &json!({ "ceremony_id": SESSION_ID }),
            )
            .await
            .expect("the stream should be readable"),
    );
    let records = records_from(&page);

    assert_eq!(verdict["ceremony_id"], SESSION_ID);
    assert_eq!(verdict["intact"], json!(true));
    assert!(verdict["first_broken_sequence"].is_null());
    assert!(verdict["reason"].is_null());
    assert_eq!(verdict["record_count"], json!(records.len()));
    assert_eq!(verdict["head_version"], page["head_version"]);
    assert!(
        AuditChain::verify(&records).is_intact(),
        "the client's own verdict must agree with the engine's"
    );
}

#[tokio::test]
async fn a_journal_that_was_never_written_is_not_found_over_the_wire() {
    let fixture = fixture().await;
    let remote = GrpcMadeMcpBackend::new(
        format!("http://{}", fixture.addr),
        MadeMcpGrpcTlsConfig::disabled(),
    );

    let error = remote
        .call_tool(
            "made_verify_ceremony_journal",
            &json!({ "ceremony_id": "never-started" }),
        )
        .await
        .expect_err("a session that does not exist cannot be verified");

    assert_eq!(error.code(), ToolErrorCode::NotFound);
}

#[tokio::test]
async fn a_ceremony_with_no_stream_is_not_found_over_the_wire() {
    let fixture = fixture().await;
    let remote = GrpcMadeMcpBackend::new(
        format!("http://{}", fixture.addr),
        MadeMcpGrpcTlsConfig::disabled(),
    );

    let error = remote
        .call_tool(
            "made_read_ceremony_events",
            &json!({ "ceremony_id": "never-started" }),
        )
        .await
        .expect_err("a session that was never opened has no stream");

    assert_eq!(error.code(), ToolErrorCode::NotFound);
}

#[tokio::test]
async fn the_transcript_carries_what_the_steps_contributed() {
    let fixture = fixture().await;
    let remote = GrpcMadeMcpBackend::new(
        format!("http://{}", fixture.addr),
        MadeMcpGrpcTlsConfig::disabled(),
    );
    session(&remote).await;

    let answer = structured(
        &remote
            .call_tool(
                "made_get_ceremony_transcript",
                &json!({ "ceremony_id": SESSION_ID }),
            )
            .await
            .expect("the transcript should be readable"),
    );

    let entries = answer["entries"]
        .as_array()
        .expect("a transcript is a list");
    assert_eq!(answer["entry_count"], json!(entries.len()));
    assert_eq!(entries.len(), 1, "{answer:#}");
    assert_eq!(entries[0]["step_id"], json!("work"));
    assert!(entries[0]["output"].is_object(), "{answer:#}");

    // A session that was never started is refused rather than
    // answered with nothing, the same way reading its stream is: an
    // empty transcript would say the id exists and has said nothing.
    let error = remote
        .call_tool(
            "made_get_ceremony_transcript",
            &json!({ "ceremony_id": "never-started" }),
        )
        .await
        .expect_err("a session that was never opened has no transcript");

    assert_eq!(error.code(), ToolErrorCode::NotFound);
}
