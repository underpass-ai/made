use std::path::PathBuf;

use choreo_adapters::mermaid::CeremonyConversationDiagram;
use choreo_adapters::yaml::CeremonyDefinitionYaml;
use choreo_core::value_objects::RoleId;

#[test]
fn editorial_meeting_fixture_renders_conversation_diagram() {
    let definition = CeremonyDefinitionYaml::parse_path(fixture_path()).unwrap();

    assert!(definition
        .role(&RoleId::new("FACILITATOR").unwrap())
        .is_some());
    assert!(definition
        .role(&RoleId::new("CUSTOMER_ADVOCATE").unwrap())
        .is_some());
    assert!(definition
        .role(&RoleId::new("RISK_REVIEWER").unwrap())
        .is_some());
    assert!(definition
        .role(&RoleId::new("SYNTHESIZER").unwrap())
        .is_some());

    let diagram = CeremonyConversationDiagram::render(&definition);

    assert!(diagram.contains("title editorial_planning_meeting v1.0"));
    assert!(diagram.contains("participant R"));
    assert!(diagram.contains("open_room [facilitation_prompt]"));
    assert!(diagram.contains("customer_story [persona_prompt]"));
    assert!(diagram.contains("risk_check [challenge_prompt]"));
    assert!(diagram.contains("decision_summary [synthesis_prompt]"));
    assert!(diagram.contains("decision_written -> CLOSED"));
}

fn fixture_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/e2e/ceremonies/editorial-planning-meeting.yaml")
}
