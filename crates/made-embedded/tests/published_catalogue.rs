//! The published catalogue, read back through the facade.

use made_adapters::yaml::{CeremonyDefinitionYaml, PublishedCeremonyDefinitionYaml};
use made_app::usecases::PublishedCeremonyDefinitionQuery;
use made_core::error::DomainError;
use made_core::value_objects::{
    CeremonyDefinitionCursor, CeremonyDefinitionPageLimit, CeremonyName, CeremonyVersion,
};
use made_embedded::EmbeddedMade;

fn document(name: &str, version: &str) -> String {
    format!(
        r#"
version: "{version}"
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
    )
}

async fn publish(engine: &EmbeddedMade, name: &str, version: &str) {
    engine
        .publish_definition(CeremonyDefinitionYaml::parse_str(&document(name, version)).unwrap())
        .await
        .unwrap();
}

#[tokio::test]
async fn a_host_pages_through_what_it_published_and_reads_one_back() {
    let engine = EmbeddedMade::default();
    publish(&engine, "facade_beta", "1.0").await;
    publish(&engine, "facade_alpha", "1.0").await;
    // Mounted is not published: it must not appear in the catalogue.
    engine
        .mount_yaml(&document("facade_mounted", "1.0"))
        .await
        .unwrap();

    let first = engine
        .list_definitions(&PublishedCeremonyDefinitionQuery::new(
            None,
            CeremonyDefinitionPageLimit::new(1).unwrap(),
            None,
        ))
        .await
        .unwrap();
    assert_eq!(first.definitions()[0].name().as_str(), "facade_alpha");
    let cursor = first.next_cursor().cloned().unwrap();

    let rest = engine
        .list_definitions(&PublishedCeremonyDefinitionQuery::new(
            None,
            CeremonyDefinitionPageLimit::default(),
            Some(cursor),
        ))
        .await
        .unwrap();
    let names = rest
        .definitions()
        .iter()
        .map(|published| published.name().as_str())
        .collect::<Vec<_>>();
    assert_eq!(names, ["facade_beta"]);
    assert!(rest.next_cursor().is_none());

    let name = CeremonyName::new("facade_beta").unwrap();
    let version = CeremonyVersion::new("1.0").unwrap();
    let read = engine.get_definition(&name, &version).await.unwrap();
    let yaml = PublishedCeremonyDefinitionYaml::render(&read).unwrap();
    assert_eq!(
        CeremonyDefinitionYaml::parse_str(&yaml)
            .unwrap()
            .digest()
            .unwrap(),
        read.digest()
    );
}

#[tokio::test]
async fn reading_a_version_nobody_published_is_not_found() {
    let engine = EmbeddedMade::default();
    engine
        .mount_yaml(&document("facade_mounted", "1.0"))
        .await
        .unwrap();

    let error = engine
        .get_definition(
            &CeremonyName::new("facade_mounted").unwrap(),
            &CeremonyVersion::new("1.0").unwrap(),
        )
        .await
        .unwrap_err();

    assert!(matches!(error, DomainError::NotFound { .. }), "{error:?}");
    let cursor = CeremonyDefinitionCursor::parse("facade_mounted@1.0").unwrap();
    assert!(engine
        .list_definitions(&PublishedCeremonyDefinitionQuery::new(
            None,
            CeremonyDefinitionPageLimit::default(),
            Some(cursor),
        ))
        .await
        .unwrap()
        .definitions()
        .is_empty());
}
