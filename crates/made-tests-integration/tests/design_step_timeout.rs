//! Per-stage timeouts in `made_design_ceremony`, and what they must not
//! change.
//!
//! Before per-stage timeouts, `step_timeout_seconds` was the only knob
//! and it became the technical timeout of every designed step: a review
//! that needs thirty minutes lent its budget to the bookkeeping steps
//! around it. A stage or group step may now carry its own
//! `timeout_seconds`, which wins over the ceremony-wide value.
//!
//! The new field is additive. An intent that does not use it must
//! design byte-for-byte the document it designed before, and the
//! definition parsed from that document must keep its digest: the
//! pinned values below were recorded on `main` before the field
//! existed.

use made_adapters::yaml::CeremonyDefinitionYaml;
use made_core::value_objects::StepId;
use made_embedded::EmbeddedMade;
use made_mcp::backend::{MadeMcpGrpcTlsConfig, MadeMcpToolBackend};
use made_mcp::protocol::ToolErrorCode;
use made_mcp::{EmbeddedMadeMcpBackend, GrpcMadeMcpBackend};
use made_tests_integration::grpc_fixture::GrpcFixture;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};

/// The design intent the plugin smoke test sends, copied verbatim from
/// `tests/plugin/made-smoke.jsonl` (the `made_design_ceremony` call).
/// Copied rather than included: the CI router does not route that file
/// to the Rust jobs, and a pin must not depend on a file whose edits
/// would not re-run it.
fn plugin_smoke_intent() -> Value {
    json!({
        "name": "plugin_designed_review",
        "objective": "Produce one proposal and require a human to approve the reviewed outcome.",
        "required_inputs": ["brief"],
        "outputs": ["reviewed_proposal"],
        "participants": [
            {"role_id": "WORKER"},
            {"role_id": "APPROVER", "capabilities": ["request_intervention"]}
        ],
        "stages": [
            {"id": "propose", "owner_role_id": "WORKER",
             "instructions": "Produce one proposal from the brief."},
            {"id": "review", "owner_role_id": "APPROVER",
             "instructions": "Review the proposal against the brief."}
        ],
        "final_approval": {"role_id": "APPROVER"}
    })
}

/// A grouped concurrent state with an explicit ceremony-wide timeout,
/// so the pin covers the group path and a non-default global.
fn grouped_intent() -> Value {
    json!({
        "name": "parallel_review",
        "objective": "Collect two independent reviews.",
        "outputs": ["decision"],
        "participants": [{"role_id": "A"}, {"role_id": "B"}],
        "max_parallel": 2,
        "step_timeout_seconds": 321,
        "stages": [{
            "id": "review",
            "group": {
                "execution": "concurrent",
                "steps": [
                    {"id": "review_a", "owner_role_id": "A", "instructions": "Review A."},
                    {"id": "review_b", "owner_role_id": "B", "instructions": "Review B."}
                ],
                "join": {"condition": "steps_completed", "count": 1}
            }
        }]
    })
}

/// The shape F4 was reported against: a bookkeeping step, two
/// concurrent reviewers with a thirty-minute budget each, and a
/// closing step, with a short ceremony-wide default.
fn two_reviewers_intent() -> Value {
    json!({
        "name": "pr_review_two_reviewers",
        "objective": "Review one pull request with two independent reviewers.",
        "outputs": ["review_outcome"],
        "participants": [
            {"role_id": "HOST"}, {"role_id": "REVIEWER_A"}, {"role_id": "REVIEWER_B"}
        ],
        "max_parallel": 2,
        "step_timeout_seconds": 120,
        "stages": [
            {"id": "open_review", "owner_role_id": "HOST",
             "instructions": "Open the review."},
            {"id": "reviews", "group": {
                "execution": "concurrent",
                "steps": [
                    {"id": "review_a", "owner_role_id": "REVIEWER_A",
                     "instructions": "Review independently.", "timeout_seconds": 1800},
                    {"id": "review_b", "owner_role_id": "REVIEWER_B",
                     "instructions": "Review independently.", "timeout_seconds": 1800}
                ]
            }},
            {"id": "close_review", "owner_role_id": "HOST",
             "instructions": "Close the review.", "timeout_seconds": 60}
        ]
    })
}

fn backends(fixture: &GrpcFixture) -> [(&'static str, Box<dyn MadeMcpToolBackend>); 2] {
    [
        (
            "gRPC",
            Box::new(GrpcMadeMcpBackend::new(
                format!("http://{}", fixture.addr),
                MadeMcpGrpcTlsConfig::disabled(),
            )),
        ),
        (
            "in-process",
            Box::new(EmbeddedMadeMcpBackend::new(EmbeddedMade::default())),
        ),
    ]
}

async fn designed_yaml(backend: &dyn MadeMcpToolBackend, intent: &Value) -> String {
    let answer = backend
        .call_tool("made_design_ceremony", intent)
        .await
        .expect("the intent designs a ceremony");
    answer["structuredContent"]["definition_yaml"]
        .as_str()
        .expect("a design answers with YAML")
        .to_owned()
}

fn sha256_hex(text: &str) -> String {
    use std::fmt::Write as _;
    Sha256::digest(text.as_bytes())
        .iter()
        .fold(String::new(), |mut hex, byte| {
            write!(hex, "{byte:02x}").expect("writing to a String cannot fail");
            hex
        })
}

#[tokio::test]
async fn an_intent_without_stage_timeouts_designs_the_same_document_and_digest_as_before() {
    let fixture = GrpcFixture::start().await;
    let pinned = [
        (
            plugin_smoke_intent(),
            "d880de29752af4fd12ab204bf4f95fc901e68aa4f415a973e74bd684df669669",
            "81226ed44156422541f550ac571e001e313dcc1afff9706a0f17e2e5b01ed2fd",
        ),
        (
            grouped_intent(),
            "4632638f6620113014e8f4949b23f1d0191b28322d31d85051df8dd500c3e725",
            "0d2a271e0b6ede32999333f7ed686e852e8f92212d5658a1310e84fa9e5b1147",
        ),
    ];
    for (backend_name, backend) in backends(&fixture) {
        for (intent, yaml_sha, digest) in &pinned {
            let yaml = designed_yaml(backend.as_ref(), intent).await;
            assert!(
                !yaml.contains("timeout_seconds"),
                "{backend_name}: no step may carry its own timeout: {yaml}"
            );
            let definition = CeremonyDefinitionYaml::parse_str(&yaml).unwrap();
            assert_eq!(
                &sha256_hex(&yaml),
                yaml_sha,
                "{backend_name}: {} rendered different bytes: {yaml}",
                intent["name"]
            );
            assert_eq!(
                &definition.digest().unwrap().to_hex(),
                digest,
                "{backend_name}: {} changed its definition digest",
                intent["name"]
            );
        }
    }
}

#[tokio::test]
async fn each_step_keeps_its_own_timeout_on_both_backends() {
    let fixture = GrpcFixture::start().await;
    let mut rendered = Vec::new();
    for (backend_name, backend) in backends(&fixture) {
        let yaml = designed_yaml(backend.as_ref(), &two_reviewers_intent()).await;
        assert!(yaml.contains("step_default: 120"), "{backend_name}: {yaml}");
        let definition = CeremonyDefinitionYaml::parse_str(&yaml).unwrap();
        let seconds = |id: &str| {
            definition
                .step(&StepId::new(id).unwrap())
                .and_then(made_core::value_objects::CeremonyStep::timeout)
                .map(|timeout| timeout.duration().get() / 1_000)
        };
        assert_eq!(seconds("open_review"), Some(120), "{backend_name}");
        assert_eq!(seconds("review_a"), Some(1_800), "{backend_name}");
        assert_eq!(seconds("review_b"), Some(1_800), "{backend_name}");
        assert_eq!(seconds("close_review"), Some(60), "{backend_name}");
        rendered.push(yaml);
    }
    assert_eq!(rendered[0], rendered[1], "one intent, two documents");
}

#[tokio::test]
async fn a_zero_or_misplaced_stage_timeout_is_refused_the_same_way_on_both_backends() {
    let fixture = GrpcFixture::start().await;
    let mut zero_leaf = two_reviewers_intent();
    zero_leaf["stages"][0]["timeout_seconds"] = json!(0);
    let mut zero_group_step = two_reviewers_intent();
    zero_group_step["stages"][1]["group"]["steps"][0]["timeout_seconds"] = json!(0);
    let mut on_group_container = two_reviewers_intent();
    on_group_container["stages"][1]["timeout_seconds"] = json!(600);

    for intent in [zero_leaf, zero_group_step, on_group_container] {
        let mut refusals = Vec::new();
        for (backend_name, backend) in backends(&fixture) {
            let error = backend
                .call_tool("made_design_ceremony", &intent)
                .await
                .expect_err("the intent must be refused");
            assert_eq!(
                error.code(),
                ToolErrorCode::InvalidRequest,
                "{backend_name}: {}",
                error.message()
            );
            refusals.push(error.message().to_owned());
        }
        assert_eq!(refusals[0], refusals[1], "{intent}");
    }
}

/// A group that repeats with a declared exhaustion outcome keeps its
/// steps' own timeouts: the two fields sit at different levels (the
/// timeout on the step, `on_exhausted` on the group repeat) and neither
/// displaces the other.
#[tokio::test]
async fn a_step_timeout_coexists_with_a_group_repeat_and_its_exhaustion_outcome() {
    let fixture = GrpcFixture::start().await;
    let intent = json!({
        "name": "pr_review",
        "objective": "Decide whether the change is approved.",
        "outputs": ["decision"],
        "participants": [{"role_id": "author"}, {"role_id": "reviewer"}],
        "max_transitions": 12,
        "step_timeout_seconds": 120,
        "stages": [
            {"id": "review_cycle", "group": {
                "steps": [
                    {"id": "verdict", "owner_role_id": "reviewer",
                     "instructions": "Review.", "timeout_seconds": 1800},
                    {"id": "outcome", "owner_role_id": "author", "instructions": "Decide."}
                ],
                "repeat": {
                    "max_iterations": 4,
                    "until": {"step": "outcome", "output_field": "outcome", "equals": "approved"},
                    "on_exhausted": {"terminal": "exhausted"}
                }
            }},
            {"id": "close_review", "owner_role_id": "author", "instructions": "Close.",
             "timeout_seconds": 60,
             "exit_guards": [{"kind": "output_field", "step": "outcome",
                              "output_field": "outcome", "equals": "approved"}]}
        ]
    });
    let mut rendered = Vec::new();
    for (backend_name, backend) in backends(&fixture) {
        let yaml = designed_yaml(backend.as_ref(), &intent).await;
        let definition = CeremonyDefinitionYaml::parse_str(&yaml).unwrap();
        let seconds = |id: &str| {
            definition
                .step(&StepId::new(id).unwrap())
                .and_then(made_core::value_objects::CeremonyStep::timeout)
                .map(|timeout| timeout.duration().get() / 1_000)
        };
        assert_eq!(seconds("verdict"), Some(1_800), "{backend_name}");
        assert_eq!(seconds("outcome"), Some(120), "{backend_name}");
        assert_eq!(seconds("close_review"), Some(60), "{backend_name}");
        assert!(yaml.contains("exhausted"), "{backend_name}: {yaml}");
        rendered.push(yaml);
    }
    assert_eq!(rendered[0], rendered[1], "one intent, two documents");
}
