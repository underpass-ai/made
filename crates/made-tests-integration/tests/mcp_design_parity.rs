//! One intent, two backends, one ceremony — the same JSON, not the
//! same shape.
//!
//! Every other parity test compares shapes, because two independently
//! run sessions cannot share timestamps or identifiers. Designing has
//! none of that: it reads no store, starts nothing and mints no ids,
//! so the same intent must produce the same document down to the
//! bytes. Anything weaker would let the two arms render different
//! YAML from the same request and still pass.
//!
//! That is the property the F3b move exists for: one designer, called
//! by both, rather than one per adapter.

use made_adapters::yaml::DesignedCeremonyYaml;
use made_app::usecases::{
    CeremonyDesignDocument, CeremonyDesignGroup, CeremonyDesignGroupStep, CeremonyDesignJoin,
    CeremonyDesignParticipant, CeremonyDesignStage, CeremonyDesignStageEntry,
    CeremonyPatternPreset,
};
use made_core::value_objects::{
    CeremonyDescription, CeremonyName, JoinStepCount, MaxParallel, OutputName, RoleId, Rounds,
    StateExecution, StepId, StepInstructions,
};
use made_embedded::EmbeddedMade;
use made_mcp::backend::{MadeMcpGrpcTlsConfig, MadeMcpToolBackend};
use made_mcp::protocol::ToolErrorCode;
use made_mcp::{EmbeddedMadeMcpBackend, GrpcMadeMcpBackend};
use made_tests_integration::grpc_fixture::GrpcFixture;
use serde_json::{json, Value};

/// Rich on purpose: two stages, a repeat policy with a non-string
/// stop value, a human gate the author did not name, a capability
/// that is not a stage, and one number left out so the engine's own
/// default is in the answer.
fn intent() -> Value {
    json!({
        "name": "editorial_review",
        "objective": "Choose the lead story and have the editor accept it.",
        "required_inputs": ["brief"],
        "optional_inputs": ["archive"],
        "outputs": ["lead_story"],
        "participants": [
            { "role_id": "WRITER", "capabilities": ["respond_to_intervention"] },
            { "role_id": "EDITOR", "capabilities": ["request_intervention"] }
        ],
        "stages": [
            {
                "id": "draft_options",
                "owner_role_id": "WRITER",
                "instructions": "Draft three candidate leads.",
                "repeat": {
                    "max_iterations": 3,
                    "output_field": "ready",
                    "equals": true
                }
            },
            {
                "id": "weigh_options",
                "owner_role_id": "EDITOR",
                "instructions": "Weigh them against the brief.",
                "num_agents": 2,
                "review_rounds": 1,
                "see_prior": true,
                "repeat": {
                    "max_iterations": 2,
                    "output_field": "accepted",
                    "equals": true
                },
                "exit_guards": [
                    {
                        "kind": "output_field",
                        "step": "draft_options",
                        "output_field": "decision=key",
                        "equals": {"label": "left=right"}
                    },
                    {
                        "kind": "step_repeat_exhausted",
                        "step": "weigh_options"
                    }
                ]
            }
        ],
        "final_approval": { "role_id": "EDITOR" },
        "backoff_seconds": 0
    })
}

fn structured(result: &Value) -> Value {
    result["structuredContent"].clone()
}

fn pattern_intent() -> Value {
    json!({
        "name": "fixed_roundtable",
        "objective": "Collect distinct perspectives on one bounded question.",
        "outputs": ["discussion"],
        "participants": [
            { "role_id": "FACILITATOR" },
            { "role_id": "REVIEWER" },
            { "role_id": "RECORDER" }
        ],
        "stages": [],
        "pattern": "roundtable_fixed_order"
    })
}

fn pattern_document() -> CeremonyDesignDocument {
    CeremonyDesignDocument::new(
        CeremonyName::new("fixed_roundtable").unwrap(),
        None,
        CeremonyDescription::new("Collect distinct perspectives on one bounded question.").unwrap(),
        Vec::new(),
        Vec::new(),
        vec![OutputName::new("discussion").unwrap()],
        ["FACILITATOR", "REVIEWER", "RECORDER"]
            .into_iter()
            .map(|role| CeremonyDesignParticipant::new(RoleId::new(role).unwrap(), []))
            .collect(),
        Vec::new(),
        None,
        None,
        None,
        None,
    )
    .with_pattern(CeremonyPatternPreset::RoundtableFixedOrder)
}

fn concurrent_intent() -> Value {
    json!({
        "name": "parallel_review",
        "objective": "Collect two independent reviews.",
        "outputs": ["decision"],
        "participants": [{"role_id": "A"}, {"role_id": "B"}],
        "max_parallel": 2,
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

fn concurrent_document() -> CeremonyDesignDocument {
    let participants = ["A", "B"]
        .into_iter()
        .map(|role| CeremonyDesignParticipant::new(RoleId::new(role).unwrap(), []))
        .collect::<Vec<_>>();
    let steps = [
        ("review_a", "A", "Review A."),
        ("review_b", "B", "Review B."),
    ]
    .into_iter()
    .map(|(id, owner, instructions)| {
        CeremonyDesignGroupStep::new(CeremonyDesignStage::new(
            StepId::new(id).unwrap(),
            RoleId::new(owner).unwrap(),
            StepInstructions::new(instructions).unwrap(),
            None,
            None,
            None,
            Rounds::ZERO,
            None,
        ))
    })
    .collect();
    CeremonyDesignDocument::new(
        CeremonyName::new("parallel_review").unwrap(),
        None,
        CeremonyDescription::new("Collect two independent reviews.").unwrap(),
        Vec::new(),
        Vec::new(),
        vec![OutputName::new("decision").unwrap()],
        participants,
        Vec::new(),
        None,
        None,
        None,
        None,
    )
    .with_stage_entries(vec![CeremonyDesignStageEntry::Group(
        CeremonyDesignGroup::new(
            StepId::new("review").unwrap(),
            StateExecution::Concurrent,
            steps,
            CeremonyDesignJoin::StepsCompleted(JoinStepCount::new(1).unwrap()),
        ),
    )])
    .with_max_parallel(MaxParallel::new(2).unwrap())
}

#[tokio::test]
async fn the_same_intent_designs_the_same_ceremony_on_both_backends() {
    let fixture = GrpcFixture::start().await;
    let remote = GrpcMadeMcpBackend::new(
        format!("http://{}", fixture.addr),
        MadeMcpGrpcTlsConfig::disabled(),
    );
    let embedded = EmbeddedMadeMcpBackend::new(EmbeddedMade::default());

    let arguments = intent();
    let over_the_wire = structured(
        &remote
            .call_tool("made_design_ceremony", &arguments)
            .await
            .expect("the gRPC backend should design the ceremony"),
    );
    let in_process = structured(
        &embedded
            .call_tool("made_design_ceremony", &arguments)
            .await
            .expect("the in-process backend should design the ceremony"),
    );

    // Sanity first: a pair of empty answers would agree about nothing.
    assert!(
        over_the_wire["definition_yaml"]
            .as_str()
            .is_some_and(|yaml| yaml.contains("name: editorial_review")),
        "the design must answer with the document: {over_the_wire:#?}"
    );
    assert_eq!(over_the_wire["publishable"], json!(true));
    assert_eq!(
        over_the_wire["design"],
        json!({
            "topology": "linear",
            "stages": 2,
            "participants": 2,
            "final_approval_required": true,
        })
    );
    assert_eq!(over_the_wire["published"], json!(false));
    assert_eq!(over_the_wire["started"], json!(false));

    assert_eq!(
        over_the_wire, in_process,
        "one intent designed two different ceremonies"
    );
}

/// The parts of the intent an omission or a zero decides. A design
/// that read `backoff_seconds: 0` as "unset" would write one second
/// into the draft, and the two arms would have to agree about that
/// too — which is why the value is in the document, not in prose.
#[tokio::test]
async fn what_the_caller_left_out_reaches_the_same_answer_on_both_backends() {
    let fixture = GrpcFixture::start().await;
    let remote = GrpcMadeMcpBackend::new(
        format!("http://{}", fixture.addr),
        MadeMcpGrpcTlsConfig::disabled(),
    );
    let embedded = EmbeddedMadeMcpBackend::new(EmbeddedMade::default());

    let arguments = intent();
    let yaml = |answer: &Value| {
        answer["definition_yaml"]
            .as_str()
            .expect("a design answers with YAML")
            .to_owned()
    };
    let over_the_wire = yaml(&structured(
        &remote
            .call_tool("made_design_ceremony", &arguments)
            .await
            .expect("the gRPC backend should design the ceremony"),
    ));
    let in_process = yaml(&structured(
        &embedded
            .call_tool("made_design_ceremony", &arguments)
            .await
            .expect("the in-process backend should design the ceremony"),
    ));

    for (backend, rendered) in [("gRPC", &over_the_wire), ("in-process", &in_process)] {
        // Version, handler, timeout and attempts were left out; the
        // backoff was written as zero and must not become one.
        assert!(
            rendered.contains("version: '1.0'"),
            "the {backend} backend must apply the default version: {rendered}"
        );
        assert!(
            rendered.contains("handler: host_callback"),
            "the {backend} backend must apply the default handler: {rendered}"
        );
        assert!(
            rendered.contains("step_default: 300"),
            "the {backend} backend must apply the default timeout: {rendered}"
        );
        assert!(
            rendered.contains("backoff_seconds: 0"),
            "the {backend} backend must keep the backoff the caller chose: {rendered}"
        );
        assert!(
            rendered.contains("max_iterations: 3"),
            "the {backend} backend must carry the repeat cap: {rendered}"
        );
        assert!(
            rendered.contains("output_field:draft_options:decision=key={\"label\":\"left=right\"}"),
            "the {backend} backend must preserve the exact JSON guard: {rendered}"
        );
        assert!(
            rendered.contains("step_repeat_exhausted:weigh_options"),
            "the {backend} backend must preserve the exhausted-repeat guard: {rendered}"
        );
    }

    assert_eq!(
        over_the_wire, in_process,
        "the same omissions rendered two different documents"
    );
}

/// An intent that cannot become a ceremony is refused the same way on
/// both arms: the same code, and a message that names the element at
/// fault rather than the call.
#[tokio::test]
async fn an_unworkable_intent_is_refused_the_same_way_on_both_backends() {
    let fixture = GrpcFixture::start().await;
    let remote = GrpcMadeMcpBackend::new(
        format!("http://{}", fixture.addr),
        MadeMcpGrpcTlsConfig::disabled(),
    );
    let embedded = EmbeddedMadeMcpBackend::new(EmbeddedMade::default());

    let mut arguments = intent();
    arguments["stages"][0]["owner_role_id"] = json!("NOBODY");

    for (backend, error) in [
        (
            "gRPC",
            remote
                .call_tool("made_design_ceremony", &arguments)
                .await
                .expect_err("a stage owned by nobody is not a ceremony"),
        ),
        (
            "in-process",
            embedded
                .call_tool("made_design_ceremony", &arguments)
                .await
                .expect_err("a stage owned by nobody is not a ceremony"),
        ),
    ] {
        assert_eq!(
            error.code(),
            ToolErrorCode::InvalidRequest,
            "the {backend} backend must call this the caller's to fix"
        );
        assert!(
            error.message().contains("unknown owner role")
                && error.message().contains("NOBODY")
                && error.message().contains("draft_options"),
            "the {backend} backend must name the element at fault: {}",
            error.message()
        );
    }
}

#[tokio::test]
async fn the_pattern_design_is_identical_on_proto_both_mcp_arms_and_the_facade() {
    let fixture = GrpcFixture::start().await;
    let remote = GrpcMadeMcpBackend::new(
        format!("http://{}", fixture.addr),
        MadeMcpGrpcTlsConfig::disabled(),
    );
    let embedded = EmbeddedMadeMcpBackend::new(EmbeddedMade::default());

    let arguments = pattern_intent();
    let over_the_wire = structured(
        &remote
            .call_tool("made_design_ceremony", &arguments)
            .await
            .expect("the gRPC-backed MCP tool accepts the preset"),
    );
    let in_process = structured(
        &embedded
            .call_tool("made_design_ceremony", &arguments)
            .await
            .expect("the embedded MCP tool accepts the preset"),
    );
    let facade = DesignedCeremonyYaml::render(
        &EmbeddedMade::default()
            .design(&pattern_document())
            .expect("the facade accepts the typed preset"),
    )
    .expect("the YAML adapter renders the facade result");

    assert_eq!(over_the_wire, in_process);
    assert_eq!(over_the_wire["definition_yaml"], facade);
    assert_eq!(over_the_wire["publishable"], true);
    assert_eq!(over_the_wire["design"]["stages"], 3);
    let yaml = over_the_wire["definition_yaml"].as_str().unwrap();
    assert!(yaml.contains("id: roundtable_turn_1"));
    assert!(yaml.contains("id: roundtable_turn_3"));
    assert_eq!(yaml.matches("see_prior: true").count(), 2);
}

#[tokio::test]
async fn invalid_pattern_values_are_refused_identically_by_both_mcp_arms() {
    let fixture = GrpcFixture::start().await;
    let remote = GrpcMadeMcpBackend::new(
        format!("http://{}", fixture.addr),
        MadeMcpGrpcTlsConfig::disabled(),
    );
    let embedded = EmbeddedMadeMcpBackend::new(EmbeddedMade::default());

    for invalid in [Value::Null, json!(7), json!("")] {
        let mut arguments = pattern_intent();
        arguments["pattern"] = invalid.clone();
        let remote_error = remote
            .call_tool("made_design_ceremony", &arguments)
            .await
            .expect_err("the gRPC-backed MCP tool must refuse an invalid pattern");
        let embedded_error = embedded
            .call_tool("made_design_ceremony", &arguments)
            .await
            .expect_err("the embedded MCP tool must refuse an invalid pattern");

        assert_eq!(remote_error.code(), ToolErrorCode::InvalidRequest);
        assert_eq!(remote_error.code(), embedded_error.code());
        assert_eq!(
            remote_error.message(),
            embedded_error.message(),
            "{invalid}"
        );
    }
}

#[tokio::test]
async fn concurrent_design_is_identical_on_proto_both_mcp_arms_and_the_facade() {
    let fixture = GrpcFixture::start().await;
    let remote = GrpcMadeMcpBackend::new(
        format!("http://{}", fixture.addr),
        MadeMcpGrpcTlsConfig::disabled(),
    );
    let embedded = EmbeddedMadeMcpBackend::new(EmbeddedMade::default());
    let arguments = concurrent_intent();

    let over_the_wire = structured(
        &remote
            .call_tool("made_design_ceremony", &arguments)
            .await
            .expect("the gRPC-backed MCP tool accepts grouped concurrency"),
    );
    let in_process = structured(
        &embedded
            .call_tool("made_design_ceremony", &arguments)
            .await
            .expect("the embedded MCP tool accepts grouped concurrency"),
    );
    let facade = DesignedCeremonyYaml::render(
        &EmbeddedMade::default()
            .design(&concurrent_document())
            .expect("the facade accepts typed grouped concurrency"),
    )
    .expect("the YAML adapter renders the facade result");

    assert_eq!(over_the_wire, in_process);
    assert_eq!(over_the_wire["definition_yaml"], facade);
    assert_eq!(over_the_wire["publishable"], true);
    let yaml = over_the_wire["definition_yaml"].as_str().unwrap();
    assert!(yaml.contains("max_parallel: 2"), "{yaml}");
    assert!(yaml.contains("execution: concurrent"), "{yaml}");
    assert!(yaml.contains("steps_completed:1"), "{yaml}");
}
