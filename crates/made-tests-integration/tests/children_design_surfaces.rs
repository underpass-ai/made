//! Child-spawn authoring is byte-identical on every public design surface.

use std::collections::{BTreeMap, HashMap};

use made_adapters::yaml::{CeremonyDefinitionYaml, DesignedCeremonyYaml};
use made_app::usecases::{
    CeremonyDesignDocument, CeremonyDesignExitGuard, CeremonyDesignGroup, CeremonyDesignGroupStep,
    CeremonyDesignJoin, CeremonyDesignParticipant, CeremonyDesignStage, CeremonyDesignStageEntry,
};
use made_core::value_objects::{
    CeremonyChildSpawn, CeremonyChildSpec, CeremonyDescription, CeremonyName, CeremonyVersion,
    ChildJoin, ChildQuorum, ChildrenCompletedCondition, ContextKey, InputName, MaxChildDepth,
    MaxChildren, OutputName, RoleId, Rounds, StateExecution, StepId, StepInstructions,
};
use made_embedded::EmbeddedMade;
use made_mcp::backend::{MadeMcpGrpcTlsConfig, MadeMcpToolBackend};
use made_mcp::{EmbeddedMadeMcpBackend, GrpcMadeMcpBackend};
use made_proto::v1::ceremony_design_exit_guard::Guard as ProtoExitGuard;
use made_proto::v1::children_completed_condition::Join as ProtoChildJoin;
use made_proto::v1::made_service_client::MadeServiceClient;
use made_proto::v1::{
    CeremonyChildSpawn as ProtoSpawn, CeremonyChildSpec as ProtoChild,
    CeremonyDesignExitGuard as ProtoGuard, CeremonyDesignGroup as ProtoGroup,
    CeremonyDesignGroupJoin as ProtoGroupJoin, CeremonyDesignGroupStep as ProtoGroupStep,
    CeremonyDesignParticipant as ProtoParticipant, CeremonyDesignStage as ProtoStage,
    ChildrenCompletedCondition as ProtoChildrenCompleted, DesignCeremonyRequest,
};
use made_tests_integration::grpc_fixture::GrpcFixture;
use serde_json::{json, Value};

fn child(index: usize) -> Value {
    json!({
        "ceremony":"review_child", "version":"1.0",
        "inputs":{"artifact":format!("artifact_{index}")}
    })
}

fn spawn(count: usize, max_children: u32, max_depth: u32) -> Value {
    json!({
        "children":(0..count).map(child).collect::<Vec<_>>(),
        "max_children":max_children, "max_depth":max_depth
    })
}

fn intent() -> Value {
    json!({
        "name":"children_design_proof",
        "objective":"Delegate published reviews and wait for explicit child joins.",
        "required_inputs":["artifact_0","artifact_1","artifact_2"],
        "outputs":["decision"],
        "participants":[{"role_id":"PARENT"},{"role_id":"OTHER"}],
        "stages":[
            {
                "id":"delegate_all", "owner_role_id":"PARENT", "instructions":"Delegate all.",
                "spawn":spawn(1, 2, 4),
                "exit_guards":[{"kind":"children_completed","step":"delegate_all","join":"all"}]
            },
            {
                "id":"delegate_any", "owner_role_id":"PARENT", "instructions":"Delegate any.",
                "spawn":spawn(2, 3, 3),
                "exit_guards":[{"kind":"children_completed","step":"delegate_any","join":"any"}]
            },
            {
                "id":"parallel_children",
                "group":{
                    "execution":"concurrent",
                    "steps":[
                        {
                            "id":"group_spawn", "owner_role_id":"PARENT",
                            "instructions":"Delegate from a group step.", "spawn":spawn(1, 2, 2)
                        },
                        {"id":"group_work","owner_role_id":"OTHER","instructions":"Work beside it."}
                    ],
                    "join":{"condition":"all_steps_completed"}
                }
            },
            {
                "id":"delegate_quorum", "owner_role_id":"PARENT",
                "instructions":"Delegate quorum.", "spawn":spawn(3, 4, 2),
                "exit_guards":[{
                    "kind":"children_completed","step":"delegate_quorum",
                    "join":"quorum","count":2
                }]
            }
        ]
    })
}

fn typed_spawn(count: usize, max_children: u16, max_depth: u16) -> CeremonyChildSpawn {
    CeremonyChildSpawn::new(
        (0..count)
            .map(|index| {
                CeremonyChildSpec::new(
                    CeremonyName::new("review_child").unwrap(),
                    CeremonyVersion::new("1.0").unwrap(),
                    BTreeMap::from([(
                        InputName::new("artifact").unwrap(),
                        ContextKey::new(format!("artifact_{index}")).unwrap(),
                    )]),
                )
            })
            .collect(),
        MaxChildren::new(max_children).unwrap(),
        MaxChildDepth::new(max_depth).unwrap(),
    )
    .unwrap()
}

fn leaf(
    id: &str,
    instructions: &str,
    spawn: CeremonyChildSpawn,
    join: ChildJoin,
) -> CeremonyDesignStage {
    CeremonyDesignStage::new(
        StepId::new(id).unwrap(),
        RoleId::new("PARENT").unwrap(),
        StepInstructions::new(instructions).unwrap(),
        None,
        None,
        None,
        Rounds::ZERO,
        None,
    )
    .with_spawn(spawn)
    .with_exit_guards(vec![CeremonyDesignExitGuard::ChildrenCompleted(
        ChildrenCompletedCondition::new(StepId::new(id).unwrap(), join),
    )])
}

fn document() -> CeremonyDesignDocument {
    let group_spawn = CeremonyDesignGroupStep::new(
        CeremonyDesignStage::new(
            StepId::new("group_spawn").unwrap(),
            RoleId::new("PARENT").unwrap(),
            StepInstructions::new("Delegate from a group step.").unwrap(),
            None,
            None,
            None,
            Rounds::ZERO,
            None,
        )
        .with_spawn(typed_spawn(1, 2, 2)),
    );
    let group_work = CeremonyDesignGroupStep::new(CeremonyDesignStage::new(
        StepId::new("group_work").unwrap(),
        RoleId::new("OTHER").unwrap(),
        StepInstructions::new("Work beside it.").unwrap(),
        None,
        None,
        None,
        Rounds::ZERO,
        None,
    ));
    CeremonyDesignDocument::new(
        CeremonyName::new("children_design_proof").unwrap(),
        None,
        CeremonyDescription::new("Delegate published reviews and wait for explicit child joins.")
            .unwrap(),
        ["artifact_0", "artifact_1", "artifact_2"]
            .into_iter()
            .map(|name| InputName::new(name).unwrap())
            .collect(),
        Vec::new(),
        vec![OutputName::new("decision").unwrap()],
        ["PARENT", "OTHER"]
            .into_iter()
            .map(|role| CeremonyDesignParticipant::new(RoleId::new(role).unwrap(), []))
            .collect(),
        Vec::new(),
        None,
        None,
        None,
        None,
    )
    .with_stage_entries(vec![
        CeremonyDesignStageEntry::Leaf(leaf(
            "delegate_all",
            "Delegate all.",
            typed_spawn(1, 2, 4),
            ChildJoin::All,
        )),
        CeremonyDesignStageEntry::Leaf(leaf(
            "delegate_any",
            "Delegate any.",
            typed_spawn(2, 3, 3),
            ChildJoin::Any,
        )),
        CeremonyDesignStageEntry::Group(CeremonyDesignGroup::new(
            StepId::new("parallel_children").unwrap(),
            StateExecution::Concurrent,
            vec![group_spawn, group_work],
            CeremonyDesignJoin::AllStepsCompleted,
        )),
        CeremonyDesignStageEntry::Leaf(leaf(
            "delegate_quorum",
            "Delegate quorum.",
            typed_spawn(3, 4, 2),
            ChildJoin::Quorum {
                count: ChildQuorum::new(2).unwrap(),
            },
        )),
    ])
}

fn proto_spawn(count: usize, max_children: u32, max_depth: u32) -> ProtoSpawn {
    ProtoSpawn {
        children: (0..count)
            .map(|index| ProtoChild {
                ceremony: "review_child".into(),
                version: "1.0".into(),
                inputs: HashMap::from([("artifact".into(), format!("artifact_{index}"))]),
            })
            .collect(),
        max_children,
        max_depth,
    }
}

fn proto_guard(step: &str, join: ProtoChildJoin) -> ProtoGuard {
    ProtoGuard {
        guard: Some(ProtoExitGuard::ChildrenCompleted(ProtoChildrenCompleted {
            step_id: step.into(),
            join: Some(join),
        })),
    }
}

fn proto_leaf(id: &str, instructions: &str, spawn: ProtoSpawn, join: ProtoChildJoin) -> ProtoStage {
    ProtoStage {
        id: id.into(),
        owner_role_id: "PARENT".into(),
        instructions: instructions.into(),
        exit_guards: vec![proto_guard(id, join)],
        spawn: Some(spawn),
        ..Default::default()
    }
}

fn proto_request() -> DesignCeremonyRequest {
    DesignCeremonyRequest {
        name: "children_design_proof".into(),
        objective: "Delegate published reviews and wait for explicit child joins.".into(),
        required_inputs: vec![
            "artifact_0".into(),
            "artifact_1".into(),
            "artifact_2".into(),
        ],
        outputs: vec!["decision".into()],
        participants: ["PARENT", "OTHER"]
            .into_iter()
            .map(|role| ProtoParticipant {
                role_id: role.into(),
                capabilities: Vec::new(),
            })
            .collect(),
        stages: vec![
            proto_leaf(
                "delegate_all",
                "Delegate all.",
                proto_spawn(1, 2, 4),
                ProtoChildJoin::All(true),
            ),
            proto_leaf(
                "delegate_any",
                "Delegate any.",
                proto_spawn(2, 3, 3),
                ProtoChildJoin::Any(true),
            ),
            ProtoStage {
                id: "parallel_children".into(),
                group: Some(ProtoGroup {
                    execution: "concurrent".into(),
                    steps: vec![
                        ProtoGroupStep {
                            id: "group_spawn".into(),
                            owner_role_id: "PARENT".into(),
                            instructions: "Delegate from a group step.".into(),
                            spawn: Some(proto_spawn(1, 2, 2)),
                            ..Default::default()
                        },
                        ProtoGroupStep {
                            id: "group_work".into(),
                            owner_role_id: "OTHER".into(),
                            instructions: "Work beside it.".into(),
                            ..Default::default()
                        },
                    ],
                    join: Some(ProtoGroupJoin {
                        condition: "all_steps_completed".into(),
                        count: None,
                    }),
                    repeat: None,
                }),
                ..Default::default()
            },
            proto_leaf(
                "delegate_quorum",
                "Delegate quorum.",
                proto_spawn(3, 4, 2),
                ProtoChildJoin::Quorum(2),
            ),
        ],
        ..Default::default()
    }
}

fn structured(result: &Value) -> Value {
    result["structuredContent"].clone()
}

#[tokio::test]
async fn spawn_and_children_joins_render_identical_yaml_and_digest_on_four_surfaces() {
    let fixture = GrpcFixture::start().await;
    let remote = GrpcMadeMcpBackend::new(
        format!("http://{}", fixture.addr),
        MadeMcpGrpcTlsConfig::disabled(),
    );
    let embedded = EmbeddedMadeMcpBackend::new(EmbeddedMade::default());
    let arguments = intent();
    let remote_mcp = structured(
        &remote
            .call_tool("made_design_ceremony", &arguments)
            .await
            .unwrap(),
    );
    let embedded_mcp = structured(
        &embedded
            .call_tool("made_design_ceremony", &arguments)
            .await
            .unwrap(),
    );
    let mut rpc = MadeServiceClient::new(fixture.channel.clone());
    let direct_rpc = rpc
        .design_ceremony(proto_request())
        .await
        .unwrap()
        .into_inner();
    let facade = EmbeddedMade::default().design(&document()).unwrap();
    let facade_yaml = DesignedCeremonyYaml::render(&facade).unwrap();

    assert_eq!(remote_mcp, embedded_mcp);
    assert_eq!(remote_mcp["definition_yaml"], facade_yaml);
    assert_eq!(direct_rpc.definition_yaml, facade_yaml);
    let parsed = CeremonyDefinitionYaml::parse_str(&facade_yaml).unwrap();
    let expected_digest = parsed.digest().unwrap();
    for rendered in [
        remote_mcp["definition_yaml"].as_str().unwrap(),
        embedded_mcp["definition_yaml"].as_str().unwrap(),
        direct_rpc.definition_yaml.as_str(),
        facade_yaml.as_str(),
    ] {
        assert_eq!(
            CeremonyDefinitionYaml::parse_str(rendered)
                .unwrap()
                .digest()
                .unwrap(),
            expected_digest
        );
    }
    assert!(facade_yaml.contains("children_completed:delegate_all:all"));
    assert!(facade_yaml.contains("children_completed:delegate_any:any"));
    assert!(facade_yaml.contains("children_completed:delegate_quorum:quorum:2"));
    assert!(facade_yaml.contains("artifact: artifact_2"));
    assert!(facade_yaml.contains("max_children: 4"));
    assert!(facade_yaml.contains("max_depth: 4"));
    assert!(parsed
        .step(&StepId::new("group_spawn").unwrap())
        .unwrap()
        .spawn()
        .is_some());
}
