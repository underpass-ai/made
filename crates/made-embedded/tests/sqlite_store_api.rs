use std::collections::BTreeMap;

use made_api::{ProtocolEngineApi, StartProtocolRequest};
use made_core::value_objects::ProtocolId;
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
    let published = ProtocolEngineApi::publish_definition(&engine, DEFINITION)
        .await
        .expect("definition publishes");
    assert_eq!(published.digest, analyzed_digest);
    engine
        .start_protocol(StartProtocolRequest {
            protocol_id: "protocol-1".to_owned(),
            definition_name: analysis.definition_name,
            definition_version: analysis.definition_version,
            context: BTreeMap::new(),
            actor_id: "host-1".to_owned(),
            actor_kind: "service".to_owned(),
        })
        .await
        .expect("published protocol starts");
    drop(engine);

    let reopened = EmbeddedMade::open(&path).expect("durable engine reopens");
    let protocol = reopened
        .protocol("protocol-1")
        .await
        .expect("instance survives restart");
    assert_eq!(protocol.definition_name, "durable_public_contract");
    assert_eq!(protocol.definition_version, "1.0");
    assert_eq!(
        protocol.definition_digest.as_deref(),
        Some(analyzed_digest.as_str())
    );
    let journal = reopened
        .audit_records(&ProtocolId::new("protocol-1").unwrap())
        .await
        .expect("journal survives restart through the embedded facade");
    assert_eq!(journal.len(), 1);
    assert_eq!(
        journal[0].event_type().as_str(),
        "protocol_instance_started"
    );
}

#[tokio::test]
async fn lifecycle_controls_from_two_hosts_survive_reopening() {
    use made_app::usecases::{CancelProtocolInput, PauseProtocolInput, ResumeProtocolInput};
    use made_core::value_objects::{
        AuditActorKind, LifecycleReason, ProtocolEndReason, ProtocolLifecyclePhase,
    };

    let directory = tempfile::tempdir().expect("temporary state directory");
    let path = directory.path().join("made.sqlite3");
    let id = ProtocolId::new("two-host-lifecycle").unwrap();

    let first = EmbeddedMade::open(&path).expect("first durable host opens");
    ProtocolEngineApi::publish_definition(&first, DEFINITION)
        .await
        .expect("definition publishes");
    first
        .start_protocol(StartProtocolRequest {
            protocol_id: id.as_str().to_owned(),
            definition_name: "durable_public_contract".to_owned(),
            definition_version: "1.0".to_owned(),
            context: BTreeMap::new(),
            actor_id: "host-1".to_owned(),
            actor_kind: "service".to_owned(),
        })
        .await
        .expect("protocol starts");
    let second = EmbeddedMade::open(&path).expect("second durable host opens");

    first
        .pause_protocol(PauseProtocolInput::new(
            id.clone(),
            "host-1",
            AuditActorKind::Service,
            LifecycleReason::new("operator_hold").unwrap(),
        ))
        .await
        .expect("first host pauses");
    let resume = second.resume_protocol(ResumeProtocolInput::new(
        id.clone(),
        "host-2",
        AuditActorKind::Service,
    ));
    let cancel = first.cancel_protocol(CancelProtocolInput::new(
        id.clone(),
        "host-1",
        AuditActorKind::Service,
        LifecycleReason::new("operator_cancelled").unwrap(),
    ));
    let (_resumed, cancelled) = tokio::join!(resume, cancel);
    let cancelled = cancelled.expect("cancellation retries after a competing append");
    assert_eq!(cancelled.lifecycle().phase(), ProtocolLifecyclePhase::Ended);
    assert_eq!(
        cancelled.lifecycle().end_reason(),
        Some(ProtocolEndReason::Cancelled)
    );

    drop(first);
    drop(second);
    let reopened = EmbeddedMade::open(&path).expect("third host reopens the store");
    let recovered = reopened.instance(&id).await.expect("protocol reopens");
    assert_eq!(recovered, cancelled);
    let event_types = reopened
        .audit_records(&id)
        .await
        .unwrap()
        .iter()
        .map(|record| record.event_type().as_str())
        .collect::<Vec<_>>();
    assert_eq!(event_types.first(), Some(&"protocol_instance_started"));
    assert!(event_types.contains(&"protocol_paused"));
    assert_eq!(event_types.last(), Some(&"protocol_cancelled"));
}

#[tokio::test]
async fn typed_builder_wires_protocol_state_and_memory_from_one_adapter() {
    use std::sync::Arc;

    use made_adapters::sqlite::SqliteProtocolStore;
    use made_app::usecases::StartProtocolInput;
    use made_core::ports::MemoryWriterPort;
    use made_core::value_objects::{
        Attributes, AuditActorKind, MemoryEntry, MemoryEntryId, MemoryEntryKind, MemoryProvenance,
        MemoryScope, MemoryWrite, ProtocolContext,
    };
    use serde_json::json;
    use time::OffsetDateTime;

    let directory = tempfile::tempdir().expect("temporary state directory");
    let path = directory.path().join("made.sqlite3");
    let store = Arc::new(SqliteProtocolStore::open(&path).unwrap());
    let scope = MemoryScope::new("team:typed-builder").unwrap();
    let remembered = MemoryEntry::new(
        MemoryEntryId::new("typed-builder:decision").unwrap(),
        MemoryEntryKind::Decision,
        "keep protocol state and memory together",
        MemoryProvenance::new(
            ProtocolId::new("earlier-session").unwrap(),
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
        .with_protocol_store_and_memory(store.clone())
        .with_definition_publications(store)
        .build();
    let mounted = engine.mount_yaml(DEFINITION).await.unwrap();
    let definition = &mounted.definitions()[0];
    let context = ProtocolContext::new(
        Attributes::new(BTreeMap::from([(
            "memory_scope".to_owned(),
            json!(scope.as_str()),
        )]))
        .unwrap(),
    );

    let opened = engine
        .start(StartProtocolInput::new(
            ProtocolId::new("typed-builder-session").unwrap(),
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
        "keep protocol state and memory together"
    );
}

#[tokio::test]
async fn generic_builder_remains_forgetful_until_a_host_supplies_memory() {
    use made_app::usecases::StartProtocolInput;
    use made_core::value_objects::{Attributes, AuditActorKind, ProtocolContext};
    use serde_json::json;

    let engine = EmbeddedMade::builder().build();
    let mounted = engine.mount_yaml(DEFINITION).await.unwrap();
    let definition = &mounted.definitions()[0];
    let context = ProtocolContext::new(
        Attributes::new(BTreeMap::from([(
            "memory_scope".to_owned(),
            json!("team:forgetful"),
        )]))
        .unwrap(),
    );

    let opened = engine
        .start(StartProtocolInput::new(
            ProtocolId::new("forgetful-session").unwrap(),
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
/// the next load costs, not what it returns, and the protocol goes on.
/// Reopening the file finds the stream, the session and an intact
/// chain.
#[tokio::test]
async fn a_snapshot_is_a_cache_of_the_fold_and_the_stream_survives_reopening() {
    use made_adapters::sqlite::SqliteProtocolStore;
    use made_app::usecases::{
        ApplyProtocolTransitionInput, ApproveProtocolGuardInput, StartProtocolInput,
    };
    use made_core::entities::{AuditChain, ProtocolInstance};
    use made_core::ports::{ProtocolEventStorePort, ProtocolSnapshotStorePort};
    use made_core::value_objects::{
        AuditActorKind, GuardName, ProtocolContext, RoleId, StreamVersion, TransitionTrigger,
    };

    let directory = tempfile::tempdir().expect("temporary state directory");
    let path = directory.path().join("made.sqlite3");
    let protocol_id = ProtocolId::new("guarded-1").unwrap();
    let approver = RoleId::new("APPROVER").unwrap();

    let engine = EmbeddedMade::open(&path).expect("durable engine opens");
    let mounted = engine.mount_yaml(HUMAN_GUARD).await.unwrap();
    let definition = mounted.definitions()[0].clone();
    engine
        .start(StartProtocolInput::new(
            protocol_id.clone(),
            definition.name().clone(),
            definition.version().clone(),
            ProtocolContext::empty(),
            "operator-1",
            AuditActorKind::Service,
        ))
        .await
        .unwrap();
    let after_approval = engine
        .approve_guard(ApproveProtocolGuardInput::new(
            protocol_id.clone(),
            GuardName::new("human_approved").unwrap(),
            approver.clone(),
            AuditActorKind::Human,
        ))
        .await
        .unwrap();

    // A second handle on the same file, to look at the store beneath
    // the facade: the snapshot must equal the fold of the stream.
    let store = SqliteProtocolStore::open(&path).expect("a second handle opens");
    let records = store
        .read(
            &protocol_id,
            StreamVersion::EMPTY,
            made_core::value_objects::ProtocolEventPageLimit::DEFAULT,
        )
        .await
        .unwrap();
    let folded =
        ProtocolInstance::rehydrate(records.iter().map(|record| record.event().unwrap())).unwrap();
    let snapshot = store
        .latest(&protocol_id)
        .await
        .unwrap()
        .expect("every append leaves a snapshot");
    assert_eq!(snapshot.version, StreamVersion::new(2));
    assert_eq!(snapshot.instance, folded);
    assert_eq!(snapshot.instance, after_approval);

    // Forgetting the cache changes nothing the host can see, and the
    // protocol continues from the fold.
    store.forget(&protocol_id).await.unwrap();
    assert_eq!(store.latest(&protocol_id).await.unwrap(), None);
    assert_eq!(engine.instance(&protocol_id).await.unwrap(), after_approval);
    let completed = engine
        .apply_transition(ApplyProtocolTransitionInput::new(
            protocol_id.clone(),
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
    assert_eq!(reopened.instance(&protocol_id).await.unwrap(), completed);
    assert_eq!(reopened.instances().await.unwrap(), vec![completed]);
    let records = reopened.audit_records(&protocol_id).await.unwrap();
    assert_eq!(
        records
            .iter()
            .map(|record| record.event_type().as_str())
            .collect::<Vec<_>>(),
        [
            "protocol_instance_started",
            "human_approval_recorded",
            "transition_applied",
            "protocol_completed",
        ]
    );
    assert!(
        AuditChain::verify(&records).is_intact(),
        "the reopened stream must verify as one chain"
    );
}
