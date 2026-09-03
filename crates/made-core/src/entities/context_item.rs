use serde::{Deserialize, Serialize};

use crate::error::DomainError;
use crate::value_objects::Attributes;

use super::external_context_validation::{
    validate_text, MAX_ITEM_ID_LEN, MAX_ITEM_KIND_LEN, MAX_ITEM_TITLE_LEN, MAX_REFERENCE_ID_LEN,
};

/// One structured item inside an external context bundle.
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
                (!trimmed.is_empty()).then_some(trimmed)
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
