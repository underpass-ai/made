use made_app::usecases::CreateCouncilInput;
use made_core::ports::AgentDescriptor;
use made_core::value_objects::{
    AgentId, AgentKind, Attributes, CouncilId, OutputContract, OutputFormat, Specialty,
};
use made_embedded::EmbeddedMade;
use std::collections::BTreeMap;

#[tokio::test]
async fn durable_public_facade_reopens_councils_agents_and_contracts() {
    let scratch = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tmp");
    std::fs::create_dir_all(&scratch).unwrap();
    let dir = tempfile::tempdir_in(scratch).unwrap();
    let path = dir.path().join("public.sqlite3");
    let id = AgentId::new("persistent-agent").unwrap();
    let specialty = Specialty::new("review").unwrap();
    let engine = EmbeddedMade::open(&path).unwrap();
    engine
        .register_agent(AgentDescriptor {
            id: id.clone(),
            specialty: specialty.clone(),
            kind: AgentKind::new("noop").unwrap(),
            attributes: Attributes::empty(),
        })
        .await
        .unwrap();
    engine
        .create_council(CreateCouncilInput {
            council_id: CouncilId::new("persistent-council").unwrap(),
            specialty: specialty.clone(),
            agents: vec![id.clone()],
        })
        .await
        .unwrap();
    engine
        .register_contract(
            OutputContract::new(
                "persistent-report",
                OutputFormat::JsonObject,
                BTreeMap::new(),
            )
            .unwrap(),
        )
        .await
        .unwrap();
    drop(engine);
    let reopened = EmbeddedMade::open(&path).unwrap();
    assert_eq!(reopened.list_councils().await.unwrap().len(), 1);
    assert_eq!(reopened.list_contracts().await.unwrap().len(), 1);
    reopened.delete_council(&specialty).await.unwrap();
    // Creating again resolves the persisted agent through a newly constructed factory.
    reopened
        .create_council(CreateCouncilInput {
            council_id: CouncilId::new("second-council").unwrap(),
            specialty,
            agents: vec![id],
        })
        .await
        .unwrap();
}
