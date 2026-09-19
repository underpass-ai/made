use made_app::usecases::CreateCouncilInput;
use made_core::ports::AgentDescriptor;
use made_core::value_objects::{
    AgentId, AgentKind, Attributes, CouncilId, CouncilJournalConsumer, CouncilJournalPageLimit,
    DurationMs, OutputContract, OutputFormat, Specialty,
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
    let consumer = CouncilJournalConsumer::new("public-reader").unwrap();
    let first = engine
        .read_council_events(None, CouncilJournalPageLimit::new(2).unwrap())
        .await
        .unwrap();
    assert_eq!(first.len(), 2);
    let through = first.last().unwrap().position();
    let lease = engine
        .lease_council_events(&consumer, DurationMs::from_millis(30_000))
        .await
        .unwrap()
        .unwrap();
    engine
        .acknowledge_council_events(&lease, through)
        .await
        .unwrap();
    assert!(
        engine.release_council_events(&lease).await.is_err(),
        "acknowledgement retires ownership"
    );
    drop(engine);
    let reopened = EmbeddedMade::open(&path).unwrap();
    assert_eq!(
        reopened.get_council_event_cursor(&consumer).await.unwrap(),
        Some(through)
    );
    let remaining = reopened
        .read_council_events(Some(through), CouncilJournalPageLimit::default())
        .await
        .unwrap();
    assert_eq!(remaining.len(), 1);
    assert!(remaining[0].position() > through);
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
