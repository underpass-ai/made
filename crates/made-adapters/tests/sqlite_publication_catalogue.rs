//! One damaged publication row must not hide the rest of the catalogue.

use made_adapters::sqlite::SqliteCeremonyStore;
use made_adapters::yaml::CeremonyDefinitionYaml;
use made_core::entities::{CeremonyCatalogueEntry, PublishedCeremonyDefinition};
use made_core::ports::CeremonyDefinitionPublicationPort;

fn sealed(name: &str) -> PublishedCeremonyDefinition {
    let yaml = format!(
        r#"
version: "1.0"
name: "{name}"
states:
  - id: OPEN
    initial: true
  - id: DONE
    terminal: true
transitions:
  - from: OPEN
    to: DONE
    trigger: finish
steps:
  - id: work
    state: OPEN
    handler: noop
roles:
  - id: FACILITATOR
    allowed_actions: [work, finish]
"#
    );
    PublishedCeremonyDefinition::seal(CeremonyDefinitionYaml::parse_str(&yaml).unwrap()).unwrap()
}

/// The key the store writes for a publication: a big-endian length,
/// the name, then the version.
fn key(name: &str, version: &str) -> Vec<u8> {
    let mut key = u16::try_from(name.len()).unwrap().to_be_bytes().to_vec();
    key.extend_from_slice(name.as_bytes());
    key.extend_from_slice(version.as_bytes());
    key
}

#[tokio::test]
async fn damaged_rows_are_listed_as_unreadable_and_the_rest_still_read() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tmp");
    std::fs::create_dir_all(&root).unwrap();
    let scratch = tempfile::tempdir_in(root).unwrap();
    let path = scratch.path().join("ceremonies.sqlite3");
    {
        let store = SqliteCeremonyStore::open(&path).unwrap();
        store.publish(sealed("intact")).await.unwrap();
        store.publish(sealed("resealed")).await.unwrap();
    }

    // A row re-encoded without being resealed, and a row whose payload
    // is not a publication at all.
    let connection = rusqlite::Connection::open(&path).unwrap();
    let stored: Vec<u8> = connection
        .query_row(
            "SELECT v FROM published_definitions WHERE k = ?1",
            rusqlite::params![key("resealed", "1.0")],
            |row| row.get(0),
        )
        .unwrap();
    let mut stored: serde_json::Value = serde_json::from_slice(&stored).unwrap();
    stored["digest"] = serde_json::json!(vec![7_u8; 32]);
    connection
        .execute(
            "UPDATE published_definitions SET v = ?2 WHERE k = ?1",
            rusqlite::params![key("resealed", "1.0"), serde_json::to_vec(&stored).unwrap()],
        )
        .unwrap();
    connection
        .execute(
            "INSERT INTO published_definitions (k, v) VALUES (?1, ?2)",
            rusqlite::params![key("undecodable", "2.0"), b"not a publication".to_vec()],
        )
        .unwrap();
    drop(connection);

    let store = SqliteCeremonyStore::open(&path).unwrap();
    assert!(
        store.catalogue().await.is_err(),
        "the strict catalogue still refuses a damaged row"
    );

    let entries = store.catalogue_entries().await.unwrap();
    let mut described = entries
        .iter()
        .map(|entry| match entry {
            CeremonyCatalogueEntry::Readable(published) => {
                (published.name().to_string(), "readable".to_owned())
            }
            CeremonyCatalogueEntry::Unreadable(unreadable) => (
                unreadable.name().to_string(),
                format!(
                    "unreadable, recorded digest {}",
                    if unreadable.recorded_digest().is_some() {
                        "kept"
                    } else {
                        "lost"
                    }
                ),
            ),
        })
        .collect::<Vec<_>>();
    described.sort();

    assert_eq!(
        described,
        [
            ("intact".to_owned(), "readable".to_owned()),
            (
                "resealed".to_owned(),
                "unreadable, recorded digest kept".to_owned()
            ),
            (
                "undecodable".to_owned(),
                "unreadable, recorded digest lost".to_owned()
            ),
        ]
    );
}
