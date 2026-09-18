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

use std::collections::BTreeMap;

use made_adapters::mermaid::CeremonyConversationDiagram;
use made_adapters::yaml::{CeremonyDefinitionYaml, DesignedCeremonyYaml};
use made_app::usecases::{
    CeremonyDesignDocument, CeremonyDesignGroup, CeremonyDesignGroupStep, CeremonyDesignJoin,
    CeremonyDesignParticipant, CeremonyDesignStage, CeremonyDesignStageEntry,
    CeremonyPatternPreset,
};
use made_core::value_objects::{
    CeremonyDescription, CeremonyName, CeremonyStepAggregation, ContextKey, ContextWrites,
    DynamicRoleBinding, JoinStepCount, MaxParallel, OutputName, PriorContext, RoleId, Rounds,
    StateExecution, StepId, StepInstructions, StepOutputField,
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

fn composed_pattern_intent() -> Value {
    json!({
        "name": "incident_review",
        "objective": "Review an incident with independent analysis and a bounded write-up.",
        "outputs": ["report"],
        "participants": [
            {"role_id": "LEAD"}, {"role_id": "OPS"},
            {"role_id": "SECURITY"}, {"role_id": "HUMAN"}
        ],
        "max_parallel": 2,
        "stages": [
            {"id": "intake", "pattern": {
                "kind": "sequential", "roles": ["LEAD"],
                "instructions": "Normalize the incident timeline."
            }},
            {"id": "analysis", "pattern": {
                "kind": "broadcast_collect", "roles": ["OPS", "SECURITY"],
                "manager_role_id": "LEAD", "instructions": "Analyze the incident independently."
            }},
            {"id": "writeup", "pattern": {
                "kind": "maker_checker", "roles": ["LEAD", "SECURITY"],
                "fallback_role_id": "HUMAN", "max_iterations": 2,
                "instructions": "Write a corrective-action report."
            }}
        ]
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

fn aggregation_intent() -> Value {
    json!({
        "name": "parallel_vote",
        "objective": "Collect independent recommendations and select the majority.",
        "outputs": ["decision"],
        "participants": [{"role_id": "A"}, {"role_id": "B"}, {"role_id": "JUDGE"}],
        "stages": [
            {
                "id": "review",
                "group": {
                    "execution": "concurrent",
                    "steps": [
                        {"id": "review_a", "owner_role_id": "A", "instructions": "Recommend."},
                        {"id": "review_b", "owner_role_id": "B", "instructions": "Recommend independently."}
                    ],
                    "join": {"condition": "all_steps_completed"}
                }
            },
            {
                "id": "vote",
                "owner_role_id": "JUDGE",
                "instructions": "Select the strict majority.",
                "see_prior": true,
                "aggregate": {"strategy": "vote", "output_field": "recommendation"}
            }
        ]
    })
}

fn aggregation_document() -> CeremonyDesignDocument {
    aggregation_document_with_join(CeremonyDesignJoin::AllStepsCompleted)
}

fn aggregation_document_with_join(join: CeremonyDesignJoin) -> CeremonyDesignDocument {
    let participants = ["A", "B", "JUDGE"]
        .into_iter()
        .map(|role| CeremonyDesignParticipant::new(RoleId::new(role).unwrap(), []))
        .collect::<Vec<_>>();
    let siblings = [
        ("review_a", "A", "Recommend."),
        ("review_b", "B", "Recommend independently."),
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
    let vote = CeremonyDesignStage::new(
        StepId::new("vote").unwrap(),
        RoleId::new("JUDGE").unwrap(),
        StepInstructions::new("Select the strict majority.").unwrap(),
        None,
        Some(PriorContext::from_visible(true)),
        None,
        Rounds::ZERO,
        None,
    )
    .with_aggregation(CeremonyStepAggregation::vote(
        StepOutputField::new("recommendation").unwrap(),
    ));
    CeremonyDesignDocument::new(
        CeremonyName::new("parallel_vote").unwrap(),
        None,
        CeremonyDescription::new("Collect independent recommendations and select the majority.")
            .unwrap(),
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
    .with_stage_entries(vec![
        CeremonyDesignStageEntry::Group(CeremonyDesignGroup::new(
            StepId::new("review").unwrap(),
            StateExecution::Concurrent,
            siblings,
            join,
        )),
        CeremonyDesignStageEntry::Leaf(vote),
    ])
}

#[test]
fn aggregation_analysis_rejects_an_early_join() {
    for join in [
        CeremonyDesignJoin::AnyStepCompleted,
        CeremonyDesignJoin::StepsCompleted(JoinStepCount::new(1).unwrap()),
    ] {
        let designed = EmbeddedMade::default()
            .design(&aggregation_document_with_join(join))
            .unwrap();
        assert!(designed.definition().analyze().errors().any(|finding| {
            finding
                .defect()
                .to_string()
                .contains("all-siblings predecessor join")
        }));
    }
}

fn dynamic_intent() -> Value {
    json!({
        "name": "dynamic_handoff",
        "objective": "Write a handoff and route its review from context.",
        "outputs": ["decision"],
        "participants": [
            {"role_id": "WRITER"}, {"role_id": "REVIEWER"}, {"role_id": "EDITOR"}
        ],
        "stages": [
            {
                "id": "draft", "owner_role_id": "WRITER",
                "instructions": "Draft the handoff.",
                "context_writes": {"next_role": "reviewer"}
            },
            {
                "id": "review", "owner_role_id": "REVIEWER",
                "instructions": "Review the handoff.",
                "role_from": "context.next_role",
                "allowed_roles": ["REVIEWER", "EDITOR"]
            }
        ]
    })
}

fn dynamic_document() -> CeremonyDesignDocument {
    let participants = ["WRITER", "REVIEWER", "EDITOR"]
        .into_iter()
        .map(|role| CeremonyDesignParticipant::new(RoleId::new(role).unwrap(), []))
        .collect();
    let draft = CeremonyDesignStage::new(
        StepId::new("draft").unwrap(),
        RoleId::new("WRITER").unwrap(),
        StepInstructions::new("Draft the handoff.").unwrap(),
        None,
        None,
        None,
        Rounds::ZERO,
        None,
    )
    .with_context_writes(ContextWrites::new(BTreeMap::from([(
        ContextKey::new("next_role").unwrap(),
        StepOutputField::new("reviewer").unwrap(),
    )])));
    let review = CeremonyDesignStage::new(
        StepId::new("review").unwrap(),
        RoleId::new("REVIEWER").unwrap(),
        StepInstructions::new("Review the handoff.").unwrap(),
        None,
        None,
        None,
        Rounds::ZERO,
        None,
    )
    .with_dynamic_role_binding(
        DynamicRoleBinding::new(
            ContextKey::new("next_role").unwrap(),
            [
                RoleId::new("REVIEWER").unwrap(),
                RoleId::new("EDITOR").unwrap(),
            ],
        )
        .unwrap(),
    );
    CeremonyDesignDocument::new(
        CeremonyName::new("dynamic_handoff").unwrap(),
        None,
        CeremonyDescription::new("Write a handoff and route its review from context.").unwrap(),
        Vec::new(),
        Vec::new(),
        vec![OutputName::new("decision").unwrap()],
        participants,
        vec![draft, review],
        None,
        None,
        None,
        None,
    )
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

    for invalid in [Value::Null, json!(7), json!(""), json!("next_role")] {
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

#[tokio::test]
async fn composed_incident_review_is_identical_on_both_mcp_editions() {
    let fixture = GrpcFixture::start().await;
    let remote = GrpcMadeMcpBackend::new(
        format!("http://{}", fixture.addr),
        MadeMcpGrpcTlsConfig::disabled(),
    );
    let embedded = EmbeddedMadeMcpBackend::new(EmbeddedMade::default());
    let arguments = composed_pattern_intent();
    let over_the_wire = structured(
        &remote
            .call_tool("made_design_ceremony", &arguments)
            .await
            .expect("gRPC designs composed patterns"),
    );
    let in_process = structured(
        &embedded
            .call_tool("made_design_ceremony", &arguments)
            .await
            .expect("embedded designs composed patterns"),
    );
    assert_eq!(over_the_wire, in_process);
    assert_eq!(over_the_wire["publishable"], true);
    let yaml = over_the_wire["definition_yaml"].as_str().unwrap();
    assert!(yaml.contains("x-pattern: broadcast_collect"), "{yaml}");
    assert!(yaml.contains("execution: concurrent"), "{yaml}");
    assert!(yaml.contains("WRITEUP_FALLBACK"), "{yaml}");
    assert_eq!(
        yaml,
        include_str!("../../../tests/e2e/ceremonies/incident_review.yaml")
    );
    let definition = CeremonyDefinitionYaml::parse_str(yaml).expect("composed YAML parses");
    let diagram = CeremonyConversationDiagram::render(&definition);
    assert!(diagram.contains("Pattern broadcast_collect"), "{diagram}");
    assert!(diagram.contains("par analysis_review_1"), "{diagram}");
    assert!(diagram.contains("and analysis_review_2"), "{diagram}");
}

#[tokio::test]
async fn aggregation_design_is_identical_on_proto_both_mcp_arms_and_the_facade() {
    let fixture = GrpcFixture::start().await;
    let remote = GrpcMadeMcpBackend::new(
        format!("http://{}", fixture.addr),
        MadeMcpGrpcTlsConfig::disabled(),
    );
    let embedded = EmbeddedMadeMcpBackend::new(EmbeddedMade::default());
    let arguments = aggregation_intent();
    let over_the_wire = structured(
        &remote
            .call_tool("made_design_ceremony", &arguments)
            .await
            .expect("the gRPC-backed MCP tool accepts aggregation"),
    );
    let in_process = structured(
        &embedded
            .call_tool("made_design_ceremony", &arguments)
            .await
            .expect("the embedded MCP tool accepts aggregation"),
    );
    let facade = DesignedCeremonyYaml::render(
        &EmbeddedMade::default()
            .design(&aggregation_document())
            .expect("the facade accepts typed aggregation"),
    )
    .expect("the YAML adapter renders aggregation");

    assert_eq!(over_the_wire, in_process);
    assert_eq!(over_the_wire["definition_yaml"], facade);
    assert_eq!(over_the_wire["publishable"], true);
    let yaml = over_the_wire["definition_yaml"].as_str().unwrap();
    assert!(yaml.contains("strategy: vote"), "{yaml}");
    assert!(yaml.contains("output_field: recommendation"), "{yaml}");
    let parsed = CeremonyDefinitionYaml::parse_str(yaml)
        .expect("the rendered aggregation definition remains valid explicit YAML");
    assert_eq!(
        parsed
            .step(&StepId::new("vote").unwrap())
            .unwrap()
            .aggregation(),
        Some(&CeremonyStepAggregation::vote(
            StepOutputField::new("recommendation").unwrap()
        ))
    );
}

#[tokio::test]
async fn dynamic_design_is_identical_on_proto_both_mcp_arms_and_the_facade() {
    let fixture = GrpcFixture::start().await;
    let remote = GrpcMadeMcpBackend::new(
        format!("http://{}", fixture.addr),
        MadeMcpGrpcTlsConfig::disabled(),
    );
    let embedded = EmbeddedMadeMcpBackend::new(EmbeddedMade::default());
    let arguments = dynamic_intent();
    let over_the_wire = structured(
        &remote
            .call_tool("made_design_ceremony", &arguments)
            .await
            .expect("the gRPC-backed MCP tool accepts dynamic authoring"),
    );
    let in_process = structured(
        &embedded
            .call_tool("made_design_ceremony", &arguments)
            .await
            .expect("the embedded MCP tool accepts dynamic authoring"),
    );
    let facade = DesignedCeremonyYaml::render(
        &EmbeddedMade::default()
            .design(&dynamic_document())
            .expect("the facade accepts typed dynamic authoring"),
    )
    .expect("the YAML adapter renders the facade result");

    assert_eq!(over_the_wire, in_process);
    assert_eq!(over_the_wire["definition_yaml"], facade);
    assert_eq!(over_the_wire["publishable"], true);
    let yaml = over_the_wire["definition_yaml"].as_str().unwrap();
    assert!(yaml.contains("role_from: context.next_role"), "{yaml}");
    assert!(yaml.contains("- REVIEWER"), "{yaml}");
    assert!(yaml.contains("next_role: reviewer"), "{yaml}");
    let parsed = CeremonyDefinitionYaml::parse_str(yaml)
        .expect("the rendered dynamic definition remains valid explicit YAML");
    let review = parsed.step(&StepId::new("review").unwrap()).unwrap();
    assert_eq!(
        review
            .dynamic_role_binding()
            .unwrap()
            .context_key()
            .as_str(),
        "next_role"
    );
    assert_eq!(
        parsed
            .step(&StepId::new("draft").unwrap())
            .unwrap()
            .context_writes()
            .entries()
            .get(&ContextKey::new("next_role").unwrap())
            .map(StepOutputField::as_str),
        Some("reviewer")
    );
}

#[tokio::test]
async fn malformed_dynamic_fields_are_refused_identically_by_both_mcp_arms() {
    let fixture = GrpcFixture::start().await;
    let remote = GrpcMadeMcpBackend::new(
        format!("http://{}", fixture.addr),
        MadeMcpGrpcTlsConfig::disabled(),
    );
    let embedded = EmbeddedMadeMcpBackend::new(EmbeddedMade::default());

    let mut cases = Vec::new();
    for invalid in [
        Value::Null,
        json!(7),
        json!(""),
        json!("next_role"),
        json!("context.   "),
        json!("context.bad\u{0007}"),
    ] {
        let mut arguments = dynamic_intent();
        arguments["stages"][1]["role_from"] = invalid.clone();
        cases.push((format!("role_from={invalid}"), arguments));
    }
    for invalid in [
        Value::Null,
        json!("REVIEWER"),
        json!([]),
        json!(["REVIEWER", 7]),
        json!(["REVIEWER", "REVIEWER"]),
        json!([""]),
        json!(["   "]),
        json!(["REVIEWER\u{0007}"]),
    ] {
        let mut arguments = dynamic_intent();
        arguments["stages"][1]["allowed_roles"] = invalid.clone();
        cases.push((format!("allowed_roles={invalid}"), arguments));
    }
    let mut missing_allowed = dynamic_intent();
    missing_allowed["stages"][1]
        .as_object_mut()
        .unwrap()
        .remove("allowed_roles");
    cases.push(("missing allowed_roles".to_owned(), missing_allowed));
    for (label, invalid) in [
        ("non-object context_writes", Value::Null),
        ("non-string context_writes value", json!({"next_role": 7})),
        ("blank context_writes key", json!({"": "reviewer"})),
        ("whitespace context_writes key", json!({"   ": "reviewer"})),
        ("blank context_writes value", json!({"next_role": ""})),
        (
            "control context_writes value",
            json!({"next_role": "reviewer\u{0007}"}),
        ),
    ] {
        let mut malformed_writes = dynamic_intent();
        malformed_writes["stages"][0]["context_writes"] = invalid;
        cases.push((label.to_owned(), malformed_writes));
    }
    for (field, value) in [
        ("owner_role_id", json!("A")),
        ("instructions", json!("Do the work.")),
        ("handler", json!("host_callback")),
        ("see_prior", json!(true)),
        ("num_agents", json!(2)),
        ("review_rounds", json!(1)),
        (
            "repeat",
            json!({"max_iterations": 2, "output_field": "done", "equals": true}),
        ),
        ("exit_guards", json!([])),
        ("role_from", json!("context.next_role")),
        ("allowed_roles", json!(["A"])),
        ("context_writes", json!({"next_role": "reviewer"})),
    ] {
        let mut group_container = concurrent_intent();
        group_container["stages"][0][field] = value;
        cases.push((
            format!("group container leaf field {field}"),
            group_container,
        ));
    }

    for (label, arguments) in cases {
        let remote_error = remote
            .call_tool("made_design_ceremony", &arguments)
            .await
            .expect_err("the gRPC-backed MCP tool must refuse malformed dynamic fields");
        let embedded_error = embedded
            .call_tool("made_design_ceremony", &arguments)
            .await
            .expect_err("the embedded MCP tool must refuse malformed dynamic fields");
        assert_eq!(remote_error.code(), ToolErrorCode::InvalidRequest);
        assert_eq!(remote_error.code(), embedded_error.code());
        assert_eq!(remote_error.message(), embedded_error.message(), "{label}");
    }
}
