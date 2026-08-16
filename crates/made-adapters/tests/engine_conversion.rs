//! Converting a ceremony store between engines.
//!
//! The property that matters is not that the command succeeds — it is that
//! the store on the other side answers every question the same way. So this
//! reads the converted store through the ports rather than counting rows:
//! the revision, the journal in order, and the published catalogue.

#![cfg(feature = "sqlite")]

use made_adapters::redb::RedbCeremonyStore;
use made_adapters::StorageEngine;
use made_core::ports::{
    AuditJournalPort, CeremonyDefinitionPublicationPort, CeremonyInstanceRepositoryPort,
    CeremonyUnitOfWorkPort,
};
use made_core::value_objects::{CeremonyId, ExpectedRevision};
use tempfile::TempDir;

const COMMITS: u64 = 12;

async fn seeded(path: &std::path::Path) -> CeremonyId {
    let ceremony = CeremonyId::new("converted-ceremony").unwrap();
    let store = RedbCeremonyStore::open(path).expect("the source store opens");
    let mut expected = ExpectedRevision::New;
    for ordinal in 1..=COMMITS {
        let outcome = store
            .commit(support::commit(&ceremony, expected, ordinal))
            .await
            .expect("the commit lands");
        expected = ExpectedRevision::Exactly(outcome.committed_revision().unwrap());
    }
    store
        .publish(support::definition_publication())
        .await
        .expect("the definition publishes");
    ceremony
}

#[tokio::test]
async fn a_redb_store_converts_to_sqlite_and_answers_the_same() {
    let directory = TempDir::new().expect("a temporary directory");
    let source = directory.path().join("ceremonies.redb");
    let destination = directory.path().join("ceremonies.sqlite3");

    let ceremony = seeded(&source).await;
    let before = {
        let store = RedbCeremonyStore::open(&source).expect("the source reopens");
        (
            store.revision(&ceremony).await.unwrap(),
            store.records(&ceremony).await.unwrap(),
            store.catalogue().await.unwrap(),
        )
    };

    let receipt = RedbCeremonyStore::convert(&source, &destination, StorageEngine::Sqlite)
        .expect("the conversion succeeds");

    assert_eq!(receipt.source_engine, StorageEngine::Redb);
    assert_eq!(receipt.destination_engine, StorageEngine::Sqlite);
    assert_eq!(receipt.ceremonies, 1);
    assert_eq!(receipt.journal_records, COMMITS);
    assert_eq!(receipt.publications, 1);

    // The destination is a SQLite store, and a plain open finds that out for
    // itself rather than being told.
    assert_eq!(
        made_adapters::engine_of(&destination).unwrap(),
        Some(StorageEngine::Sqlite)
    );
    let converted = RedbCeremonyStore::open(&destination).expect("a plain open picks the engine");

    let (revision, records, catalogue) = before;
    assert_eq!(converted.revision(&ceremony).await.unwrap(), revision);
    assert_eq!(
        converted.records(&ceremony).await.unwrap(),
        records,
        "the journal must survive whole and in order: it is a hash chain, and a \
         reordered or truncated copy would not verify"
    );
    assert_eq!(converted.catalogue().await.unwrap(), catalogue);
    assert!(converted.exists(&ceremony).await.unwrap());

    // The source is left alone: a conversion that damaged its own input would
    // leave an operator with nothing to go back to.
    let reopened = RedbCeremonyStore::open(&source).expect("the source still opens");
    assert_eq!(reopened.revision(&ceremony).await.unwrap(), revision);
}

#[tokio::test]
async fn conversion_refuses_the_three_ways_it_could_destroy_something() {
    let directory = TempDir::new().expect("a temporary directory");
    let source = directory.path().join("ceremonies.redb");
    let occupied = directory.path().join("occupied.sqlite3");
    seeded(&source).await;

    // Into itself.
    assert!(
        RedbCeremonyStore::convert(&source, &source, StorageEngine::Sqlite).is_err(),
        "a conversion into its own source would overwrite what it is reading"
    );

    // Into a path that already holds a store.
    RedbCeremonyStore::open_sqlite(&occupied).expect("an unrelated store exists");
    assert!(
        RedbCeremonyStore::convert(&source, &occupied, StorageEngine::Sqlite).is_err(),
        "converting into occupied memory is a worse failure than the one it fixes"
    );

    // To the engine it already runs on: not destructive, but a no-op dressed
    // as work, and an operator who runs it is confused about their store.
    let elsewhere = directory.path().join("copy.redb");
    assert!(
        RedbCeremonyStore::convert(&source, &elsewhere, StorageEngine::Redb).is_err(),
        "converting to the engine it already runs on should say so"
    );
}

mod support {
    use made_core::entities::{
        AuditFact, CeremonyCommit, CeremonyDefinition, CeremonyInstance,
        PublishedCeremonyDefinition,
    };
    use made_core::value_objects::{
        AuditActor, AuditActorKind, AuditEventType, CeremonyContext, CeremonyId, CeremonyName,
        CeremonyState, CeremonyTransition, CeremonyVersion, EventId, ExpectedRevision, StateId,
        TransitionTrigger,
    };
    use time::OffsetDateTime;

    pub fn commit(
        ceremony_id: &CeremonyId,
        expected: ExpectedRevision,
        ordinal: u64,
    ) -> CeremonyCommit {
        let definition = definition();
        let instance = CeremonyInstance::start(
            ceremony_id.clone(),
            &definition,
            CeremonyContext::empty(),
            OffsetDateTime::UNIX_EPOCH,
        );
        let fact = AuditFact {
            event_id: EventId::new(format!("convert-{ordinal}")).unwrap(),
            event_type: AuditEventType::StepCompleted,
            ceremony_id: ceremony_id.clone(),
            definition_name: definition.name().clone(),
            definition_version: definition.version().clone(),
            occurred_at: OffsetDateTime::UNIX_EPOCH,
            actor: AuditActor::new("test", AuditActorKind::Engine, None).unwrap(),
            correlation_id: None,
            causation_id: None,
            trace: None,
        };
        CeremonyCommit::new(instance, expected, [fact], []).unwrap()
    }

    pub fn definition_publication() -> PublishedCeremonyDefinition {
        PublishedCeremonyDefinition::seal(definition()).expect("the definition seals")
    }

    pub fn definition() -> CeremonyDefinition {
        CeremonyDefinition::new(
            CeremonyName::new("converted_ceremony").unwrap(),
            CeremonyVersion::v1(),
            None,
            Vec::new(),
            Vec::new(),
            vec![
                CeremonyState::initial(StateId::new("OPEN").unwrap()),
                CeremonyState::terminal(StateId::new("DONE").unwrap()),
            ],
            vec![CeremonyTransition::new(
                StateId::new("OPEN").unwrap(),
                StateId::new("DONE").unwrap(),
                TransitionTrigger::new("finish").unwrap(),
                Vec::new(),
            )
            .unwrap()],
            Vec::new(),
            Vec::new(),
            Vec::new(),
        )
        .unwrap()
    }
}
