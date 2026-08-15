//! Bounded external context bundle passed into a deliberation.
//!
//! This keeps external context first-class and typed without baking
//! any domain taxonomy into the core. Callers choose their own
//! `item.kind` labels (for example `finding`, `decision`, `action`,
//! `note`) and attach machine-readable detail through `Attributes`.

use serde::{Deserialize, Serialize};

use crate::error::DomainError;
use crate::value_objects::Attributes;

const MAX_BUNDLE_ID_LEN: usize = 128;
const MAX_SCHEMA_VERSION_LEN: usize = 64;
const MAX_ITEM_ID_LEN: usize = 128;
const MAX_ITEM_KIND_LEN: usize = 64;
const MAX_ITEM_TITLE_LEN: usize = 256;
const MAX_REFERENCE_ID_LEN: usize = 128;
const MAX_URI_LEN: usize = 2048;
const MAX_ITEMS: usize = 256;
const MAX_REFERENCES: usize = 512;

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

/// Top-level bundle summary meant for fast orientation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContextSummary {
    text: String,
    #[serde(default)]
    attributes: Attributes,
}

impl ContextSummary {
    pub fn new(text: impl Into<String>, attributes: Attributes) -> Result<Self, DomainError> {
        let text = text.into();
        Ok(Self {
            text: validate_text(&text, "external_context.summary.text", MAX_ITEM_TITLE_LEN)?,
            attributes,
        })
    }

    #[must_use]
    pub fn text(&self) -> &str {
        &self.text
    }

    #[must_use]
    pub fn attributes(&self) -> &Attributes {
        &self.attributes
    }
}

/// One structured item inside the external bundle.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContextItem {
    item_id: String,
    kind: String,
    title: String,
    narrative: Option<String>,
    #[serde(default)]
    attributes: Attributes,
    #[serde(default)]
    reference_ids: Vec<String>,
}

impl ContextItem {
    pub fn new(
        item_id: impl Into<String>,
        kind: impl Into<String>,
        title: impl Into<String>,
        narrative: Option<String>,
        attributes: Attributes,
        reference_ids: Vec<String>,
    ) -> Result<Self, DomainError> {
        let reference_ids = reference_ids
            .into_iter()
            .map(|reference_id| {
                validate_text(
                    &reference_id,
                    "external_context.item.reference_id",
                    MAX_REFERENCE_ID_LEN,
                )
            })
            .collect::<Result<Vec<_>, _>>()?;
        let item_id = item_id.into();
        let kind = kind.into();
        let title = title.into();

        Ok(Self {
            item_id: validate_text(&item_id, "external_context.item_id", MAX_ITEM_ID_LEN)?,
            kind: validate_text(&kind, "external_context.item.kind", MAX_ITEM_KIND_LEN)?,
            title: validate_text(&title, "external_context.item.title", MAX_ITEM_TITLE_LEN)?,
            narrative: narrative.and_then(|text| {
                let trimmed = text.trim().to_owned();
                if trimmed.is_empty() {
                    None
                } else {
                    Some(trimmed)
                }
            }),
            attributes,
            reference_ids,
        })
    }

    #[must_use]
    pub fn item_id(&self) -> &str {
        &self.item_id
    }

    #[must_use]
    pub fn kind(&self) -> &str {
        &self.kind
    }

    #[must_use]
    pub fn title(&self) -> &str {
        &self.title
    }

    #[must_use]
    pub fn narrative(&self) -> Option<&str> {
        self.narrative.as_deref()
    }

    #[must_use]
    pub fn attributes(&self) -> &Attributes {
        &self.attributes
    }

    #[must_use]
    pub fn reference_ids(&self) -> &[String] {
        &self.reference_ids
    }
}

/// Structured reference material associated with a context bundle.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContextReference {
    reference_id: String,
    uri: String,
    title: Option<String>,
    media_type: Option<String>,
    #[serde(default)]
    attributes: Attributes,
}

impl ContextReference {
    pub fn new(
        reference_id: impl Into<String>,
        uri: impl Into<String>,
        title: Option<String>,
        media_type: Option<String>,
        attributes: Attributes,
    ) -> Result<Self, DomainError> {
        let reference_id = reference_id.into();
        let uri = uri.into();
        Ok(Self {
            reference_id: validate_text(
                &reference_id,
                "external_context.reference_id",
                MAX_REFERENCE_ID_LEN,
            )?,
            uri: validate_text(&uri, "external_context.reference.uri", MAX_URI_LEN)?,
            title: normalize_optional(title),
            media_type: normalize_optional(media_type),
            attributes,
        })
    }

    #[must_use]
    pub fn reference_id(&self) -> &str {
        &self.reference_id
    }

    #[must_use]
    pub fn uri(&self) -> &str {
        &self.uri
    }

    #[must_use]
    pub fn title(&self) -> Option<&str> {
        self.title.as_deref()
    }

    #[must_use]
    pub fn media_type(&self) -> Option<&str> {
        self.media_type.as_deref()
    }

    #[must_use]
    pub fn attributes(&self) -> &Attributes {
        &self.attributes
    }
}

fn validate_text(value: &str, field: &'static str, max_len: usize) -> Result<String, DomainError> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err(DomainError::EmptyField { field });
    }
    if trimmed.len() > max_len {
        return Err(DomainError::FieldTooLong {
            field,
            actual: trimmed.len(),
            max: max_len,
        });
    }
    Ok(trimmed.to_owned())
}

fn validate_collection_len(
    field: &'static str,
    actual: usize,
    max: usize,
) -> Result<(), DomainError> {
    if actual > max {
        return Err(DomainError::OutOfRange {
            field,
            value: actual as f64,
            min: 0.0,
            max: max as f64,
        });
    }
    Ok(())
}

fn normalize_optional(value: Option<String>) -> Option<String> {
    value.and_then(|value| {
        let trimmed = value.trim().to_owned();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed)
        }
    })
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
