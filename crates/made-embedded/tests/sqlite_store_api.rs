use std::collections::BTreeMap;

use made_api::{CeremonyEngineApi, StartCeremonyRequest};
use made_core::value_objects::CeremonyId;
use made_embedded::EmbeddedMade;

const DEFINITION: &str = r#"
version: "1.0"
name: "durable_public_contract"
states:
  - id: OPEN
    initial: true
  - id: CLOSED
    terminal: true
transitions:
  - from: OPEN
    to: CLOSED
    trigger: close
    guards: []
steps: []
guards: {}
roles: []
"#;

#[tokio::test]
async fn published_definition_and_instance_survive_reopening_via_the_public_surface() {
    let directory = tempfile::tempdir().expect("temporary state directory");
    let path = directory.path().join("made.sqlite3");

    let engine = EmbeddedMade::open(&path).expect("durable engine opens");
    let analysis = engine
        .analyze_definition(DEFINITION)
        .await
        .expect("definition analyzes");
    assert_eq!(analysis.definition_name, "durable_public_contract");
    assert_eq!(analysis.definition_version, "1.0");
    assert!(analysis.publishable);
    let analyzed_digest = analysis
        .definition_digest
        .clone()
        .expect("a publishable analysis names the publication identity");
    let published = CeremonyEngineApi::publish_definition(&engine, DEFINITION)
        .await
        .expect("definition publishes");
    assert_eq!(published.digest, analyzed_digest);
    engine
        .start_ceremony(StartCeremonyRequest {
            ceremony_id: "ceremony-1".to_owned(),
            definition_name: analysis.definition_name,
            definition_version: analysis.definition_version,
            context: BTreeMap::new(),
            actor_id: "host-1".to_owned(),
            actor_kind: "service".to_owned(),
        })
        .await
        .expect("published ceremony starts");
    drop(engine);

    let reopened = EmbeddedMade::open(&path).expect("durable engine reopens");
    let ceremony = reopened
        .ceremony("ceremony-1")
        .await
        .expect("instance survives restart");
    assert_eq!(ceremony.definition_name, "durable_public_contract");
    assert_eq!(ceremony.definition_version, "1.0");
    assert_eq!(
        ceremony.definition_digest.as_deref(),
        Some(analyzed_digest.as_str())
    );
    let journal = reopened
        .audit_records(&CeremonyId::new("ceremony-1").unwrap())
        .await
        .expect("journal survives restart through the embedded facade");
    assert_eq!(journal.len(), 1);
    assert_eq!(
        journal[0].event_type().as_str(),
        "ceremony_instance_started"
    );
}

#[tokio::test]
async fn typed_builder_wires_ceremony_state_and_memory_from_one_adapter() {
    use std::sync::Arc;

    use made_adapters::sqlite::SqliteCeremonyStore;
    use made_app::usecases::StartCeremonyInput;
    use made_core::ports::MemoryWriterPort;
    use made_core::value_objects::{
        Attributes, AuditActorKind, CeremonyContext, MemoryEntry, MemoryEntryId, MemoryEntryKind,
        MemoryProvenance, MemoryScope, MemoryWrite,
    };
    use serde_json::json;
    use time::OffsetDateTime;

    let directory = tempfile::tempdir().expect("temporary state directory");
    let path = directory.path().join("made.sqlite3");
    let store = Arc::new(SqliteCeremonyStore::open(&path).unwrap());
    let scope = MemoryScope::new("team:typed-builder").unwrap();
    let remembered = MemoryEntry::new(
        MemoryEntryId::new("typed-builder:decision").unwrap(),
        MemoryEntryKind::Decision,
        "keep ceremony state and memory together",
        MemoryProvenance::new(
            CeremonyId::new("earlier-session").unwrap(),
            None,
            OffsetDateTime::UNIX_EPOCH,
        ),
        Attributes::empty(),
    )
    .unwrap();
    store
        .remember(
            &scope,
            MemoryWrite::unexplained(vec![remembered]).unwrap(),
            "typed-builder:write",
        )
        .await
        .unwrap();

    let engine = EmbeddedMade::builder()
        .with_ceremony_store_and_memory(store.clone())
        .with_definition_publications(store)
        .build();
    let mounted = engine.mount_yaml(DEFINITION).await.unwrap();
    let definition = &mounted.definitions()[0];
    let context = CeremonyContext::new(
        Attributes::new(BTreeMap::from([(
            "memory_scope".to_owned(),
            json!(scope.as_str()),
        )]))
        .unwrap(),
    );

    let opened = engine
        .start(StartCeremonyInput::new(
            CeremonyId::new("typed-builder-session").unwrap(),
            definition.name().clone(),
            definition.version().clone(),
            context,
            "host",
            AuditActorKind::Service,
        ))
        .await
        .unwrap();

    let recollection = opened
        .recollection()
        .expect("the typed adapter must supply the memory it stores");
    assert_eq!(recollection.scope(), &scope);
    assert_eq!(
        recollection.entries()[0].summary(),
        "keep ceremony state and memory together"
    );
}

#[tokio::test]
async fn generic_builder_remains_forgetful_until_a_host_supplies_memory() {
    use made_app::usecases::StartCeremonyInput;
    use made_core::value_objects::{Attributes, AuditActorKind, CeremonyContext};
    use serde_json::json;

    let engine = EmbeddedMade::builder().build();
    let mounted = engine.mount_yaml(DEFINITION).await.unwrap();
    let definition = &mounted.definitions()[0];
    let context = CeremonyContext::new(
        Attributes::new(BTreeMap::from([(
            "memory_scope".to_owned(),
            json!("team:forgetful"),
        )]))
        .unwrap(),
    );

    let opened = engine
        .start(StartCeremonyInput::new(
            CeremonyId::new("forgetful-session").unwrap(),
            definition.name().clone(),
            definition.version().clone(),
            context,
            "host",
            AuditActorKind::Service,
        ))
        .await
        .unwrap();

    assert!(opened.recollection().is_none());
}

const HUMAN_GUARD: &str = r#"
version: "1.0"
name: "durable_human_guard"
states:
  - id: WAITING
    initial: true
  - id: APPROVED
    terminal: true
transitions:
  - from: WAITING
    to: APPROVED
    trigger: approve
    guards:
      - human_approved
guards:
  human_approved:
    type: human
    check: manual_approval
roles:
  - id: APPROVER
    allowed_actions:
      - approve
"#;

/// The snapshot is the fold and nothing more: dropping it changes what
/// the next load costs, not what it returns, and the ceremony goes on.
/// Reopening the file finds the stream, the session and an intact
/// chain.
#[tokio::test]
async fn a_snapshot_is_a_cache_of_the_fold_and_the_stream_survives_reopening() {
    use made_adapters::sqlite::SqliteCeremonyStore;
    use made_app::usecases::{
        ApplyCeremonyTransitionInput, ApproveCeremonyGuardInput, StartCeremonyInput,
    };
    use made_core::entities::{AuditChain, CeremonyInstance};
    use made_core::ports::{CeremonyEventStorePort, CeremonySnapshotStorePort};
    use made_core::value_objects::{
        AuditActorKind, CeremonyContext, GuardName, RoleId, StreamVersion, TransitionTrigger,
    };

    let directory = tempfile::tempdir().expect("temporary state directory");
    let path = directory.path().join("made.sqlite3");
    let ceremony_id = CeremonyId::new("guarded-1").unwrap();
    let approver = RoleId::new("APPROVER").unwrap();

    let engine = EmbeddedMade::open(&path).expect("durable engine opens");
    let mounted = engine.mount_yaml(HUMAN_GUARD).await.unwrap();
    let definition = mounted.definitions()[0].clone();
    engine
        .start(StartCeremonyInput::new(
            ceremony_id.clone(),
            definition.name().clone(),
            definition.version().clone(),
            CeremonyContext::empty(),
            "operator-1",
            AuditActorKind::Service,
        ))
        .await
        .unwrap();
    let after_approval = engine
        .approve_guard(ApproveCeremonyGuardInput::new(
            ceremony_id.clone(),
            GuardName::new("human_approved").unwrap(),
            approver.clone(),
            AuditActorKind::Human,
        ))
        .await
        .unwrap();

    // A second handle on the same file, to look at the store beneath
    // the facade: the snapshot must equal the fold of the stream.
    let store = SqliteCeremonyStore::open(&path).expect("a second handle opens");
    let records = store
        .read(
            &ceremony_id,
            StreamVersion::EMPTY,
            made_core::value_objects::CeremonyEventPageLimit::DEFAULT,
        )
        .await
        .unwrap();
    let folded =
        CeremonyInstance::rehydrate(records.iter().map(|record| record.event().unwrap())).unwrap();
    let snapshot = store
        .latest(&ceremony_id)
        .await
        .unwrap()
        .expect("every append leaves a snapshot");
    assert_eq!(snapshot.version, StreamVersion::new(2));
    assert_eq!(snapshot.instance, folded);
    assert_eq!(snapshot.instance, after_approval);

    // Forgetting the cache changes nothing the host can see, and the
    // ceremony continues from the fold.
    store.forget(&ceremony_id).await.unwrap();
    assert_eq!(store.latest(&ceremony_id).await.unwrap(), None);
    assert_eq!(engine.instance(&ceremony_id).await.unwrap(), after_approval);
    let completed = engine
        .apply_transition(ApplyCeremonyTransitionInput::new(
            ceremony_id.clone(),
            approver,
            AuditActorKind::Agent,
            TransitionTrigger::new("approve").unwrap(),
        ))
        .await
        .unwrap();
    assert!(completed.is_completed(&definition));
    drop(store);
    drop(engine);

    let reopened = EmbeddedMade::open(&path).expect("durable engine reopens");
    assert_eq!(reopened.instance(&ceremony_id).await.unwrap(), completed);
    assert_eq!(reopened.instances().await.unwrap(), vec![completed]);
    let records = reopened.audit_records(&ceremony_id).await.unwrap();
    assert_eq!(
        records
            .iter()
            .map(|record| record.event_type().as_str())
            .collect::<Vec<_>>(),
        [
            "ceremony_instance_started",
            "human_approval_recorded",
            "transition_applied",
            "ceremony_completed",
        ]
    );
    assert!(
        AuditChain::verify(&records).is_intact(),
        "the reopened stream must verify as one chain"
    );
}

/// A session a v0.3.x store kept in `ceremony_instances` has no stream,
/// and the event-sourced engine does not see it: not found on its id,
/// absent from the list, until `made-mcp migrate-store` imports it.
///
/// The store is the committed one `made-tests-integration` holds — a
/// file v0.3.1 really wrote. Nothing in this repository can produce
/// such a file any more, which is the point: the rows exist, no code
/// writes them, and the engine reads streams.
#[tokio::test]
async fn a_session_from_an_earlier_store_without_a_stream_is_not_visible() {
    use made_adapters::sqlite::SqliteCeremonyStore;
    use made_core::DomainError;

    let fixture = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../made-tests-integration/fixtures/stores/v0.3.1/ceremonies.sqlite3");
    let directory = tempfile::tempdir().expect("temporary state directory");
    let path = directory.path().join("made.sqlite3");
    std::fs::copy(&fixture, &path).expect("the v0.3.1 fixture copies");
    let ceremony_id = CeremonyId::new("pre-stream-midflight").unwrap();

    let legacy = SqliteCeremonyStore::open(&path).expect("the store opens");
    assert_eq!(legacy.legacy_instances_without_a_stream().unwrap(), 2);
    drop(legacy);

    let engine = EmbeddedMade::open(&path).expect("durable engine opens beside legacy rows");
    assert!(matches!(
        engine.instance(&ceremony_id).await,
        Err(DomainError::NotFound {
            what: "ceremony_instance"
        })
    ));
    assert!(engine.instances().await.unwrap().is_empty());
    assert!(matches!(
        engine.audit_records(&ceremony_id).await,
        Err(DomainError::NotFound {
            what: "ceremony_instance"
        })
    ));
}
