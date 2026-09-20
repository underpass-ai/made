//! The durable agentic-system stores against their contracts, and
//! after a reopen.

#![cfg(feature = "sqlite")]

use made_adapters::sqlite::{
    SqliteAgenticSystemExecutions, SqliteAgenticSystemPublications, SqliteAgenticSystemRepository,
    SqliteCeremonyStore,
};
use made_core::conformance::{
    AgenticSystemExecutionStoreConformance, AgenticSystemPublicationConformance,
    AgenticSystemRepositoryConformance,
};
use made_core::entities::{AgenticSystem, PublishedAgenticSystem};
use made_core::ports::{
    AgenticSystemPublicationPort, AgenticSystemRepositoryPort, AgenticSystemSaveOutcome,
};
use made_core::value_objects::{
    AgenticSystemId, AgenticSystemRevision, AttentionPolicy, CeremonyActivation,
    CeremonyComposition, CeremonyDefinitionDigest, CeremonyName, CeremonyVersion, DefinitionPin,
    LogicalParticipant, ParticipantBindingPolicy, ParticipantId, ParticipantKind, Responsibility,
    SupervisionPolicy, SystemCeremonyId, SystemPurpose, SystemRole, SystemRoleId, SystemRoleKind,
};
use std::path::Path;
use tempfile::TempDir;
use time::OffsetDateTime;

fn database(directory: &Path) -> std::path::PathBuf {
    directory.join("ceremonies.sqlite3")
}

#[tokio::test]
async fn sqlite_satisfies_the_repository_contract() {
    let directory = TempDir::new().expect("a temporary directory");
    let repository =
        SqliteAgenticSystemRepository::open(database(directory.path())).expect("it opens");

    let passed = AgenticSystemRepositoryConformance::run(&repository)
        .await
        .unwrap_or_else(|failure| panic!("{failure}"));

    assert_eq!(passed.len(), 5, "properties run: {passed:?}");
}

#[tokio::test]
async fn sqlite_satisfies_the_publication_contract() {
    let directory = TempDir::new().expect("a temporary directory");
    let publications =
        SqliteAgenticSystemPublications::open(database(directory.path())).expect("it opens");

    let passed = AgenticSystemPublicationConformance::run(&publications)
        .await
        .unwrap_or_else(|failure| panic!("{failure}"));

    assert_eq!(passed.len(), 5, "properties run: {passed:?}");
}

#[tokio::test]
async fn sqlite_satisfies_the_execution_store_contract() {
    let directory = TempDir::new().expect("a temporary directory");
    let executions =
        SqliteAgenticSystemExecutions::open(database(directory.path())).expect("it opens");

    let passed = AgenticSystemExecutionStoreConformance::run(&executions)
        .await
        .unwrap_or_else(|failure| panic!("{failure}"));

    assert_eq!(passed.len(), 4, "properties run: {passed:?}");
}

/// A restart is the whole point. A design somebody edited twice, and
/// the revision a run is pinned to, both have to be there when the
/// process that wrote them is gone — and the earlier revision has to
/// still say what it said, not what the head says now.
#[tokio::test]
async fn a_reopened_store_still_holds_every_revision_and_seal() {
    let directory = TempDir::new().expect("a temporary directory");
    let id = AgenticSystemId::new("reopened").unwrap();

    {
        let store = SqliteCeremonyStore::open(database(directory.path())).expect("it opens");
        let repository = store.agentic_system_repository();
        let created = repository
            .save(design(&id, "as first written"), None)
            .await
            .expect("the first save lands");
        assert_eq!(
            created,
            AgenticSystemSaveOutcome::saved(AgenticSystemRevision::INITIAL)
        );
        repository
            .save(
                design(&id, "as rewritten").edited(OffsetDateTime::UNIX_EPOCH),
                Some(AgenticSystemRevision::INITIAL),
            )
            .await
            .expect("the second save lands");
        store
            .agentic_system_publications()
            .publish(
                PublishedAgenticSystem::seal(
                    design(&id, "as first written")
                        .published(OffsetDateTime::UNIX_EPOCH)
                        .expect("a draft publishes"),
                    OffsetDateTime::UNIX_EPOCH,
                )
                .expect("it seals"),
            )
            .await
            .expect("the seal lands");
    }

    let reopened = SqliteCeremonyStore::open(database(directory.path())).expect("it reopens");
    let head = reopened
        .agentic_system_repository()
        .get(&id, None)
        .await
        .expect("the repository reads")
        .expect("the head survived the reopen");
    assert_eq!(head.revision().get(), 2);
    assert_eq!(head.purpose().as_str(), "as rewritten");

    let first = reopened
        .agentic_system_repository()
        .get(&id, Some(AgenticSystemRevision::INITIAL))
        .await
        .expect("the repository reads")
        .expect("the first revision survived the reopen");
    assert_eq!(first.purpose().as_str(), "as first written");

    let sealed = reopened
        .agentic_system_publications()
        .published(&id, AgenticSystemRevision::INITIAL)
        .await
        .expect("the publications read")
        .expect("the seal survived the reopen");
    assert_eq!(sealed.digest(), first.digest().expect("it digests"));
}

fn design(id: &AgenticSystemId, purpose: &str) -> AgenticSystem {
    let integrator = SystemRoleId::new("integrator").unwrap();
    AgenticSystem::draft(
        id.clone(),
        SystemPurpose::new(purpose).unwrap(),
        integrator.clone(),
        [SystemRole::new(
            integrator.clone(),
            Responsibility::new("drives the system").unwrap(),
            SystemRoleKind::Integrator,
        )],
        [LogicalParticipant::new(
            ParticipantId::new("operator").unwrap(),
            integrator,
            ParticipantKind::Person,
            ParticipantBindingPolicy::default(),
        )],
        [],
        [],
        [CeremonyComposition::new(
            SystemCeremonyId::new("delivery").unwrap(),
            DefinitionPin::new(
                CeremonyName::new("delivery").unwrap(),
                CeremonyVersion::new("1.0").unwrap(),
                CeremonyDefinitionDigest::from_bytes([0x0a; 32]),
            ),
            SystemPurpose::new("does the work").unwrap(),
            [],
            CeremonyActivation::Manual,
            [],
            [],
        )],
        SupervisionPolicy::default(),
        AttentionPolicy::default(),
        OffsetDateTime::UNIX_EPOCH,
    )
    .expect("a valid draft")
}
