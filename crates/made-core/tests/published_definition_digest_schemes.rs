//! Publications sealed before the rename must still verify (#253).
//!
//! The fixtures are synthetic `published_definitions` rows in the exact
//! shape the store writes, `{definition, digest}`, built from the e2e
//! ceremonies under `tests/e2e/ceremonies`. The two `choreographer_*`
//! rows are sealed under the pre-rename domain separator
//! `underpass.choreo.ceremony-definition.v1`, as Underpass Choreographer
//! sealed them; the `made_*` row is sealed under the current separator
//! and is the control. Their digests were computed with
//! `CeremonyDefinition::digest_under`, never copied.
//!
//! Claims that must fall if broken: each row verifies under exactly the
//! scheme it was sealed with and keeps its recorded digest; the content
//! is unchanged, only the separator differs; and a digest matching no
//! known scheme is still refused.

use made_core::entities::{CeremonyDefinition, PublishedCeremonyDefinition};
use made_core::value_objects::{CeremonyDefinitionDigest, CeremonyDefinitionDigestScheme};

const CHOREOGRAPHER_ROWS: [(&str, &str); 2] = [
    (
        "incident_review@1.0",
        include_str!("fixtures/published_definitions/choreographer_incident_review_1_0.json"),
    ),
    (
        "daily_standup@1.0",
        include_str!("fixtures/published_definitions/choreographer_daily_standup_1_0.json"),
    ),
];

const MADE_ROW: &str = include_str!("fixtures/published_definitions/made_sprint_planning_1_0.json");

/// A stored row: `{definition, digest}` with the digest as 32 bytes.
fn row(raw: &str) -> (CeremonyDefinition, CeremonyDefinitionDigest) {
    let value: serde_json::Value = serde_json::from_str(raw).unwrap();
    let definition: CeremonyDefinition =
        serde_json::from_value(value["definition"].clone()).unwrap();
    let digest: CeremonyDefinitionDigest = serde_json::from_value(value["digest"].clone()).unwrap();
    (definition, digest)
}

#[test]
fn choreographer_rows_verify_under_their_own_scheme_and_keep_their_digest() {
    for (label, raw) in CHOREOGRAPHER_ROWS {
        let (definition, recorded) = row(raw);
        assert_eq!(
            format!("{}@{}", definition.name(), definition.version()),
            label
        );

        let verified = PublishedCeremonyDefinition::verify(definition.clone(), recorded)
            .unwrap_or_else(|error| panic!("{label}: {error}"));

        assert_eq!(
            verified.scheme(),
            CeremonyDefinitionDigestScheme::ChoreographerV1,
            "{label}"
        );
        assert_eq!(verified.digest(), recorded, "{label}");
        assert_eq!(verified.definition(), &definition, "{label}");
    }
}

/// Only the separator differs: the canonical form today is the one the
/// Choreographer hashed, so the mismatch was never a content change.
#[test]
fn the_choreographer_digest_is_the_same_canonical_form_under_another_separator() {
    for (label, raw) in CHOREOGRAPHER_ROWS {
        let (definition, recorded) = row(raw);

        assert_eq!(
            definition
                .digest_under(CeremonyDefinitionDigestScheme::ChoreographerV1)
                .unwrap(),
            recorded,
            "{label}"
        );
        assert_ne!(definition.digest().unwrap(), recorded, "{label}");
    }
}

#[test]
fn a_made_row_verifies_under_the_current_scheme() {
    let (definition, recorded) = row(MADE_ROW);

    let verified = PublishedCeremonyDefinition::verify(definition.clone(), recorded).unwrap();

    assert_eq!(verified.scheme(), CeremonyDefinitionDigestScheme::CURRENT);
    assert_eq!(
        verified,
        PublishedCeremonyDefinition::seal(definition).unwrap()
    );
}

#[test]
fn a_digest_matching_no_known_scheme_is_refused() {
    for (label, raw) in CHOREOGRAPHER_ROWS
        .iter()
        .copied()
        .chain([("made", MADE_ROW)])
    {
        let (definition, _) = row(raw);

        let error = PublishedCeremonyDefinition::verify(
            definition,
            CeremonyDefinitionDigest::from_bytes([7; 32]),
        )
        .unwrap_err();

        assert!(error.to_string().contains("digest"), "{label}: {error}");
    }
}

/// Changed content under a Choreographer digest is not accepted just
/// because the digest is a legacy one.
#[test]
fn edited_content_under_a_choreographer_digest_is_refused() {
    let (_, recorded) = row(CHOREOGRAPHER_ROWS[1].1);
    let mut value: serde_json::Value = serde_json::from_str(CHOREOGRAPHER_ROWS[1].1).unwrap();
    value["definition"]["description"] = serde_json::json!("edited after publication");
    let edited: CeremonyDefinition = serde_json::from_value(value["definition"].clone()).unwrap();

    assert!(PublishedCeremonyDefinition::verify(edited, recorded).is_err());
}

/// Republishing unchanged content over a Choreographer row is the same
/// publication, not a conflict; different content is not.
#[test]
fn same_content_is_recognised_across_schemes() {
    let (definition, recorded) = row(CHOREOGRAPHER_ROWS[0].1);
    let legacy = PublishedCeremonyDefinition::verify(definition.clone(), recorded).unwrap();
    let resealed = PublishedCeremonyDefinition::seal(definition).unwrap();

    assert_ne!(legacy.digest(), resealed.digest());
    assert!(legacy.holds_same_content_as(&resealed).unwrap());
    assert!(resealed.holds_same_content_as(&legacy).unwrap());

    let (other, _) = row(CHOREOGRAPHER_ROWS[1].1);
    let other = PublishedCeremonyDefinition::seal(other).unwrap();
    assert!(!legacy.holds_same_content_as(&other).unwrap());
}
