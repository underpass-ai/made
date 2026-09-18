//! Visit coordinates and reset behavior across MCP and direct gRPC.
use super::optionals::checked;
use super::*;
use made_proto::v1::made_service_client::MadeServiceClient;
use made_proto::v1::{GetCeremonyInstanceRequest, GetCeremonyTranscriptRequest};

const YAML: &str = include_str!("../../../../tests/e2e/ceremonies/state-visits.yaml");
const ID: &str = "parity-visits";

#[tokio::test]
async fn cyclic_state_visits_have_mcp_and_direct_grpc_parity() {
    let arms = ParityArms::start_on_the_shipped_store().await;
    let mut direct = MadeServiceClient::new(arms.fixture.channel.clone());
    let started = checked(
        &arms,
        4000,
        "made_start_ceremony",
        json!({
            "ceremony_id": ID, "definition_yaml": YAML,
            "actor_id": "operator", "actor_kind": "service"
        }),
    )
    .await;
    assert_eq!(structured(&started)["current_state_visit"], 1);
    let mut request_id = 4001;
    for (step, visit, iteration, ready, transition) in [
        ("a", 1, 1, false, None),
        ("a", 1, 2, true, Some("next")),
        ("b", 2, 1, true, Some("back")),
        ("a", 3, 1, false, None),
        ("a", 3, 2, true, Some("finish")),
    ] {
        claim_complete(
            &arms,
            &mut direct,
            request_id,
            step,
            visit,
            iteration,
            ready,
        )
        .await;
        request_id += 2;
        if let Some(trigger) = transition {
            let moved = checked(
                &arms,
                request_id,
                "made_apply_ceremony_transition",
                json!({"ceremony_id": ID, "trigger": trigger, "actor_kind": "agent"}),
            )
            .await;
            assert_eq!(structured(&moved)["current_state_visit"], visit + 1);
            assert_eq!(structured(&moved)["current_state_iteration"], 1);
            if trigger == "back" {
                let reset = structured(&moved)["steps"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .find(|r| r["step_id"] == "a")
                    .unwrap();
                assert_eq!(reset["status"], "pending");
                assert_eq!(reset["state_visit"], 3);
            }
            request_id += 1;
        }
    }
    assert_history(&arms, &mut direct, request_id).await;
}

async fn claim_complete(
    arms: &ParityArms,
    direct: &mut MadeServiceClient<tonic::transport::Channel>,
    request_id: u64,
    step: &str,
    visit: u32,
    iteration: u32,
    ready: bool,
) {
    let claimed = checked(
        arms,
        request_id,
        "made_claim_ceremony_step",
        json!({
            "ceremony_id": ID, "step_id": step, "actor_kind": "agent",
            "lease_owner_id": "host", "lease_ttl_ms": 60000,
            "idempotency_key": format!("{visit}-{iteration}-{step}")
        }),
    )
    .await;
    let record = structured(&claimed)["steps"]
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["step_id"] == step)
        .unwrap();
    assert_eq!(record["state_visit"], visit);
    assert_eq!(record["state_iteration"], iteration);
    assert_eq!(record["attempt"], 1);

    let completed = checked(
        arms,
        request_id + 1,
        "made_complete_ceremony_step",
        json!({
            "ceremony_id": ID, "step_id": step, "actor_kind": "agent", "status": "completed",
            "output": {"ready": ready, "marker": format!("{visit}-{iteration}")}
        }),
    )
    .await;
    let grpc = direct
        .get_ceremony_instance(GetCeremonyInstanceRequest {
            ceremony_id: ID.to_owned(),
        })
        .await
        .unwrap()
        .into_inner()
        .instance
        .unwrap();
    assert_eq!(grpc.current_state_visit, visit);
    assert_eq!(
        u64::from(grpc.current_state_iteration),
        structured(&completed)["current_state_iteration"]
            .as_u64()
            .unwrap()
    );
    let grpc_record = grpc.steps.iter().find(|r| r.step_id == step).unwrap();
    assert_eq!(grpc_record.state_visit, visit);
}

async fn assert_history(
    arms: &ParityArms,
    direct: &mut MadeServiceClient<tonic::transport::Channel>,
    request_id: u64,
) {
    let history = checked(
        arms,
        request_id,
        "made_read_ceremony_events",
        json!({"ceremony_id": ID}),
    )
    .await;
    let records = structured(&history)["records"].as_array().unwrap();
    assert_eq!(
        records
            .iter()
            .map(|r| r["event_id"].as_str().unwrap())
            .collect::<BTreeSet<_>>()
            .len(),
        records.len()
    );
    let boundaries = records
        .iter()
        .filter(|r| r["event_type"] == "state_iteration_started")
        .collect::<Vec<_>>();
    assert_eq!(boundaries.len(), 2);
    assert_eq!(boundaries[0]["event"]["state_visit"], 1);
    assert_eq!(boundaries[1]["event"]["state_visit"], 3);
    assert_eq!(
        records
            .iter()
            .filter(|r| r["event_type"] == "transition_applied")
            .count(),
        3
    );
    let transcript = checked(
        arms,
        request_id + 1,
        "made_get_ceremony_transcript",
        json!({"ceremony_id": ID}),
    )
    .await;
    let expected = [
        ("a", 1, 1),
        ("a", 1, 2),
        ("b", 2, 1),
        ("a", 3, 1),
        ("a", 3, 2),
    ];
    assert_eq!(
        structured(&transcript)["entries"]
            .as_array()
            .unwrap()
            .iter()
            .map(|e| (
                e["step_id"].as_str().unwrap(),
                e["state_visit"].as_u64().unwrap(),
                e["state_iteration"].as_u64().unwrap()
            ))
            .collect::<Vec<_>>(),
        expected
    );
    let grpc = direct
        .get_ceremony_transcript(GetCeremonyTranscriptRequest {
            ceremony_id: ID.to_owned(),
        })
        .await
        .unwrap()
        .into_inner();
    assert_eq!(
        grpc.entries
            .iter()
            .map(|e| (
                e.step_id.as_str(),
                u64::from(e.state_visit),
                u64::from(e.state_iteration)
            ))
            .collect::<Vec<_>>(),
        expected
    );
    let verified = checked(
        arms,
        request_id + 2,
        "made_verify_ceremony_journal",
        json!({"ceremony_id": ID}),
    )
    .await;
    assert_eq!(structured(&verified)["intact"], true);
}
