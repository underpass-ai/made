//! Rows sealed by Underpass Choreographer read back from SQLite (#253).
//!
//! The rows are `published_definitions` values sealed under the
//! Choreographer scheme (synthetic, see the made-core fixtures), inserted
//! byte for byte as a store carried across the rename holds them. Reading them
//! must neither fail nor rewrite them, and must hand back the digest
//! instances bound to.

#![cfg(feature = "sqlite")]

use made_adapters::sqlite::SqliteCeremonyStore;
use made_adapters::yaml::{CeremonyDefinitionYaml, PublishedCeremonyDefinitionYaml};
use made_core::entities::{
    CeremonyCatalogueEntry, PublicationOutcome, PublishedCeremonyDefinition,
};
use made_core::ports::CeremonyDefinitionPublicationPort;
use made_core::value_objects::{
    CeremonyDefinitionDigest, CeremonyDefinitionDigestScheme, CeremonyName, CeremonyVersion,
};

const INCIDENT_REVIEW: &[u8] = include_bytes!(
    "../../made-core/tests/fixtures/published_definitions/choreographer_incident_review_1_0.json"
);
const DAILY_STANDUP: &[u8] = include_bytes!(
    "../../made-core/tests/fixtures/published_definitions/choreographer_daily_standup_1_0.json"
);

/// The key the store writes for a publication: a big-endian length,
/// the name, then the version.
fn key(name: &str, version: &str) -> Vec<u8> {
    let mut key = u16::try_from(name.len()).unwrap().to_be_bytes().to_vec();
    key.extend_from_slice(name.as_bytes());
    key.extend_from_slice(version.as_bytes());
    key
}

fn recorded(row: &[u8]) -> CeremonyDefinitionDigest {
    let value: serde_json::Value = serde_json::from_slice(row).unwrap();
    serde_json::from_value(value["digest"].clone()).unwrap()
}

/// A store holding the two Choreographer rows verbatim.
fn store_with_rows(scratch: &tempfile::TempDir) -> std::path::PathBuf {
    let path = scratch.path().join("ceremonies.sqlite3");
    drop(SqliteCeremonyStore::open(&path).unwrap());
    let connection = rusqlite::Connection::open(&path).unwrap();
    for (name, version, row) in [
        ("incident_review", "1.0", INCIDENT_REVIEW),
        ("daily_standup", "1.0", DAILY_STANDUP),
    ] {
        connection
            .execute(
                "INSERT INTO published_definitions (k, v) VALUES (?1, ?2)",
                rusqlite::params![key(name, version), row.to_vec()],
            )
            .unwrap();
    }
    path
}

fn stored_row(path: &std::path::Path, name: &str, version: &str) -> Vec<u8> {
    rusqlite::Connection::open(path)
        .unwrap()
        .query_row(
            "SELECT v FROM published_definitions WHERE k = ?1",
            rusqlite::params![key(name, version)],
            |row| row.get(0),
        )
        .unwrap()
}

fn scratch() -> tempfile::TempDir {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tmp");
    std::fs::create_dir_all(&root).unwrap();
    tempfile::tempdir_in(root).unwrap()
}

#[tokio::test]
async fn choreographer_rows_read_back_with_the_digest_they_were_sealed_with() {
    let scratch = scratch();
    let path = store_with_rows(&scratch);
    let store = SqliteCeremonyStore::open(&path).unwrap();

    let published = store
        .published(
            &CeremonyName::new("incident_review").unwrap(),
            &CeremonyVersion::new("1.0").unwrap(),
        )
        .await
        .unwrap()
        .unwrap();
    assert_eq!(published.digest(), recorded(INCIDENT_REVIEW));
    assert_eq!(
        published.scheme(),
        CeremonyDefinitionDigestScheme::ChoreographerV1
    );

    assert_eq!(store.catalogue().await.unwrap().len(), 2);
    let entries = store.catalogue_entries().await.unwrap();
    assert_eq!(entries.len(), 2);
    assert!(entries
        .iter()
        .all(|entry| matches!(entry, CeremonyCatalogueEntry::Readable(_))));

    // Reading never rewrites the row.
    drop(store);
    assert_eq!(stored_row(&path, "incident_review", "1.0"), INCIDENT_REVIEW);
}

#[tokio::test]
async fn a_choreographer_row_renders_as_yaml_that_names_the_same_content() {
    let scratch = scratch();
    let path = store_with_rows(&scratch);
    let store = SqliteCeremonyStore::open(&path).unwrap();
    let published = store
        .published(
            &CeremonyName::new("daily_standup").unwrap(),
            &CeremonyVersion::new("1.0").unwrap(),
        )
        .await
        .unwrap()
        .unwrap();

    let yaml = PublishedCeremonyDefinitionYaml::render(&published).unwrap();

    let reread = CeremonyDefinitionYaml::parse_str(&yaml).unwrap();
    assert_eq!(
        reread
            .digest_under(CeremonyDefinitionDigestScheme::ChoreographerV1)
            .unwrap(),
        published.digest()
    );
}

#[tokio::test]
async fn republishing_unchanged_content_over_a_choreographer_row_is_not_a_conflict() {
    let scratch = scratch();
    let path = store_with_rows(&scratch);
    let store = SqliteCeremonyStore::open(&path).unwrap();
    let legacy = store
        .published(
            &CeremonyName::new("incident_review").unwrap(),
            &CeremonyVersion::new("1.0").unwrap(),
        )
        .await
        .unwrap()
        .unwrap();

    let offered = PublishedCeremonyDefinition::seal(legacy.definition().clone()).unwrap();
    let outcome = store.publish(offered).await.unwrap();

    let PublicationOutcome::AlreadyPublished(occupant) = outcome else {
        panic!("expected AlreadyPublished, got {outcome:?}");
    };
    assert_eq!(occupant.digest(), recorded(INCIDENT_REVIEW));
    drop(store);
    assert_eq!(stored_row(&path, "incident_review", "1.0"), INCIDENT_REVIEW);
}

#[tokio::test]
async fn different_content_under_a_choreographer_version_is_still_a_conflict() {
    let scratch = scratch();
    let path = store_with_rows(&scratch);
    let store = SqliteCeremonyStore::open(&path).unwrap();
    let design = store
        .published(
            &CeremonyName::new("daily_standup").unwrap(),
            &CeremonyVersion::new("1.0").unwrap(),
        )
        .await
        .unwrap()
        .unwrap();
    let mut value = serde_json::to_value(design.definition()).unwrap();
    value["description"] = serde_json::json!("edited after publication");
    let offered =
        PublishedCeremonyDefinition::seal(serde_json::from_value(value).unwrap()).unwrap();

    let outcome = store.publish(offered).await.unwrap();

    assert!(
        matches!(
            outcome,
            PublicationOutcome::VersionOccupied { published, .. }
                if published == recorded(DAILY_STANDUP)
        ),
        "{outcome:?}"
    );
}
