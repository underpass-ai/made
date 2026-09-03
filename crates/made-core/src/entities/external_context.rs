//! Bounded external context bundle passed into a deliberation.
//!
//! This keeps external context first-class and typed without baking
//! any domain taxonomy into the core. Callers choose their own
//! `item.kind` labels (for example `finding`, `decision`, `action`,
//! `note`) and attach machine-readable detail through `Attributes`.

use serde::{Deserialize, Serialize};

use super::external_context_validation::{
    validate_collection_len, validate_text, MAX_BUNDLE_ID_LEN, MAX_ITEMS, MAX_REFERENCES,
    MAX_SCHEMA_VERSION_LEN,
};
use crate::entities::{ContextItem, ContextReference, ContextSummary};
use crate::error::DomainError;
use crate::value_objects::Attributes;

/// Immutable bounded context handed to a council invocation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExternalContextBundle {
    bundle_id: String,
    schema_version: String,
    summary: Option<ContextSummary>,
    #[serde(default)]
    items: Vec<ContextItem>,
    #[serde(default)]
    references: Vec<ContextReference>,
    #[serde(default)]
    metadata: Attributes,
}

impl ExternalContextBundle {
    pub fn new(
        bundle_id: impl Into<String>,
        schema_version: impl Into<String>,
        summary: Option<ContextSummary>,
        items: Vec<ContextItem>,
        references: Vec<ContextReference>,
        metadata: Attributes,
    ) -> Result<Self, DomainError> {
        let bundle_id = bundle_id.into();
        let bundle_id = validate_text(&bundle_id, "external_context.bundle_id", MAX_BUNDLE_ID_LEN)?;
        let schema_version = schema_version.into();
        let schema_version = validate_text(
            &schema_version,
            "external_context.schema_version",
            MAX_SCHEMA_VERSION_LEN,
        )?;
        validate_collection_len("external_context.items", items.len(), MAX_ITEMS)?;
        validate_collection_len(
            "external_context.references",
            references.len(),
            MAX_REFERENCES,
        )?;

        Ok(Self {
            bundle_id,
            schema_version,
            summary,
            items,
            references,
            metadata,
        })
    }

    #[must_use]
    pub fn bundle_id(&self) -> &str {
        &self.bundle_id
    }

    #[must_use]
    pub fn schema_version(&self) -> &str {
        &self.schema_version
    }

    #[must_use]
    pub fn summary(&self) -> Option<&ContextSummary> {
        self.summary.as_ref()
    }

    #[must_use]
    pub fn items(&self) -> &[ContextItem] {
        &self.items
    }

    #[must_use]
    pub fn references(&self) -> &[ContextReference] {
        &self.references
    }

    #[must_use]
    pub fn metadata(&self) -> &Attributes {
        &self.metadata
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::collections::BTreeMap;

    fn attrs(key: &str, value: serde_json::Value) -> Attributes {
        Attributes::new(BTreeMap::from([(key.to_owned(), value)])).unwrap()
    }

    fn sample_bundle() -> ExternalContextBundle {
        ExternalContextBundle::new(
            "ctx-1",
            "v1",
            Some(
                ContextSummary::new(
                    "Complex state assembled from external systems",
                    attrs("source", json!("kernel")),
                )
                .unwrap(),
            ),
            vec![
                ContextItem::new(
                    "finding-1",
                    "finding",
                    "Primary observation",
                    Some("A recent deployment correlates with the symptom".to_owned()),
                    attrs("score", json!(0.92)),
                    vec!["ref-1".to_owned()],
                )
                .unwrap(),
                ContextItem::new(
                    "decision-1",
                    "decision",
                    "Previous decision",
                    None,
                    attrs("decision", json!("rollback rejected")),
                    vec!["ref-2".to_owned()],
                )
                .unwrap(),
            ],
            vec![
                ContextReference::new(
                    "ref-1",
                    "s3://evidence/1.json",
                    Some("evidence snapshot".to_owned()),
                    Some("application/json".to_owned()),
                    Attributes::empty(),
                )
                .unwrap(),
                ContextReference::new(
                    "ref-2",
                    "graph://decision/2",
                    None,
                    None,
                    attrs("kind", json!("decision")),
                )
                .unwrap(),
            ],
            attrs("bundle_kind", json!("external")),
        )
        .unwrap()
    }

    #[test]
    fn bundle_preserves_typed_sections() {
        let bundle = sample_bundle();
        assert_eq!(bundle.bundle_id(), "ctx-1");
        assert_eq!(bundle.schema_version(), "v1");
        assert_eq!(
            bundle.summary().unwrap().text(),
            "Complex state assembled from external systems"
        );
        assert_eq!(bundle.items().len(), 2);
        assert_eq!(bundle.references().len(), 2);
        assert_eq!(bundle.items()[0].kind(), "finding");
        assert_eq!(
            bundle.items()[1].attributes().get("decision"),
            Some(&json!("rollback rejected"))
        );
    }

    #[test]
    fn empty_bundle_id_is_rejected() {
        let err = ExternalContextBundle::new("", "v1", None, vec![], vec![], Attributes::empty())
            .unwrap_err();
        assert!(matches!(
            err,
            DomainError::EmptyField {
                field: "external_context.bundle_id"
            }
        ));
    }

    #[test]
    fn context_item_requires_kind_and_title() {
        let err =
            ContextItem::new("item-1", "", "", None, Attributes::empty(), vec![]).unwrap_err();
        assert!(matches!(
            err,
            DomainError::EmptyField {
                field: "external_context.item.kind"
            }
        ));
    }

    #[test]
    fn reference_requires_uri() {
        let err = ContextReference::new("ref-1", " ", None, None, Attributes::empty()).unwrap_err();
        assert!(matches!(
            err,
            DomainError::EmptyField {
                field: "external_context.reference.uri"
            }
        ));
    }

    #[test]
    fn serde_roundtrip_preserves_structure() {
        let bundle = sample_bundle();
        let json = serde_json::to_value(&bundle).unwrap();
        let back: ExternalContextBundle = serde_json::from_value(json).unwrap();
        assert_eq!(back, bundle);
    }
}
