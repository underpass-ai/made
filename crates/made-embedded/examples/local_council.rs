use made_app::usecases::CreateCouncilInput;
use made_core::ports::AgentDescriptor;
use made_core::value_objects::{AgentId, AgentKind, Attributes, CouncilId, Specialty};
use made_embedded::EmbeddedMade;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let engine = EmbeddedMade::builder().build();
    let agent_id = engine
        .register_agent(AgentDescriptor {
            id: AgentId::new("reviewer-1")?,
            specialty: Specialty::new("review")?,
            kind: AgentKind::new("noop")?,
            attributes: Attributes::empty(),
        })
        .await?;
    let council = engine
        .create_council(CreateCouncilInput {
            council_id: CouncilId::new("review-council")?,
            specialty: Specialty::new("review")?,
            agents: vec![agent_id],
        })
        .await?;
    assert_eq!(council.size(), 1);
    Ok(())
}
