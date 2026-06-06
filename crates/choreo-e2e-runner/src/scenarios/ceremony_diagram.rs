use anyhow::{bail, Context, Result};
use choreo_adapters::mermaid::CeremonyConversationDiagram;
use choreo_adapters::yaml::CeremonyDefinitionYaml;
use choreo_core::value_objects::RoleId;
use tracing::info;

const EDITORIAL_MEETING_CEREMONY: &str =
    include_str!("../../../../tests/e2e/ceremonies/editorial-planning-meeting.yaml");

pub(crate) fn verify_editorial_meeting_ceremony_diagram() -> Result<()> {
    let definition = CeremonyDefinitionYaml::parse_str(EDITORIAL_MEETING_CEREMONY)
        .context("editorial planning ceremony fixture must parse")?;
    for role in [
        "FACILITATOR",
        "CUSTOMER_ADVOCATE",
        "RISK_REVIEWER",
        "SYNTHESIZER",
    ] {
        if definition.role(&RoleId::new(role)?).is_none() {
            bail!("editorial planning ceremony is missing role {role}");
        }
    }

    let diagram = CeremonyConversationDiagram::render(&definition);
    for expected in [
        "sequenceDiagram",
        "open_room [facilitation_prompt]",
        "customer_story [persona_prompt]",
        "risk_check [challenge_prompt]",
        "decision_summary [synthesis_prompt]",
        "decision_written -> CLOSED",
    ] {
        if !diagram.contains(expected) {
            bail!("ceremony diagram does not contain expected fragment `{expected}`");
        }
    }

    info!(diagram = %diagram, "editorial planning ceremony diagram rendered");
    Ok(())
}
