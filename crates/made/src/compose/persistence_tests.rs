use super::*;
use made_adapters::agents::DispatchingAgentFactory;
use made_adapters::config::{GrpcTlsConfig, MemorySelection};
use made_core::value_objects::{
    AgentId, CouncilJournalPageLimit, MaxParallel, OutputContract, OutputContractId, OutputFormat,
    Specialty,
};

fn config(path: Option<String>) -> ServiceConfig {
    ServiceConfig {
        grpc_port: 50055,
        http_port: 8080,
        nats_enabled: false,
        nats_url: "nats://unused".into(),
        trigger_subject: "made.trigger.>".into(),
        publish_prefix: "made".into(),
        postgres_url: None,
        ceremony_store_path: path,
        artifact_store_path: None,
        memory: MemorySelection::Automatic,
        grpc_tls: GrpcTlsConfig::Disabled,
        max_parallel: MaxParallel::SERVER_MAX,
    }
}
#[tokio::test]
async fn configured_local_service_reopens_councils_contracts_and_independent_journal() {
    let scratch = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tmp");
    std::fs::create_dir_all(&scratch).unwrap();
    let directory = tempfile::tempdir_in(scratch).unwrap();
    let cfg = config(Some(
        directory
            .path()
            .join("service.sqlite3")
            .to_string_lossy()
            .into_owned(),
    ));
    let factory = Arc::new(DispatchingAgentFactory::new());
    let first = wire_persistence(&cfg, factory.clone()).await.unwrap();
    let specialty = Specialty::new("research").unwrap();
    crate::seeding::apply_seeding(
        &made_adapters::clock::SystemClock::new(),
        first.agent_registry.as_ref(),
        first.council_registry.as_ref(),
        &["research"],
    )
    .await
    .unwrap();
    let contract = OutputContract::new(
        "research-contract",
        OutputFormat::JsonObject,
        std::collections::BTreeMap::default(),
    )
    .unwrap();
    first
        .contract_registry
        .register(contract.clone())
        .await
        .unwrap();
    let facts = first
        .council_journal
        .read(None, CouncilJournalPageLimit::default())
        .await
        .unwrap();
    assert_eq!(facts.len(), 3);
    drop(first);
    let second = wire_persistence(&cfg, factory).await.unwrap();
    assert_eq!(
        second
            .council_registry
            .get(&specialty)
            .await
            .unwrap()
            .id()
            .as_str(),
        "seed-research"
    );
    assert_eq!(
        second
            .contract_registry
            .get(&OutputContractId::new("research-contract").unwrap())
            .await
            .unwrap(),
        contract
    );
    assert_eq!(
        second
            .council_journal
            .read(None, CouncilJournalPageLimit::default())
            .await
            .unwrap(),
        facts
    );
    assert_eq!(
        second
            .agent_resolver
            .resolve(&AgentId::new("seed-research-0").unwrap())
            .await
            .unwrap()
            .specialty()
            .as_str(),
        "research"
    );
    crate::seeding::apply_seeding(
        &made_adapters::clock::SystemClock::new(),
        second.agent_registry.as_ref(),
        second.council_registry.as_ref(),
        &["research"],
    )
    .await
    .unwrap();
    assert_eq!(
        second
            .council_journal
            .read(None, CouncilJournalPageLimit::default())
            .await
            .unwrap(),
        facts
    );
    assert!(second.pool.is_none());
}
