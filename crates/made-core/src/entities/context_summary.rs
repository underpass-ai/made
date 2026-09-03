use serde::{Deserialize, Serialize};

use crate::error::DomainError;
use crate::value_objects::Attributes;

use super::external_context_validation::{validate_text, MAX_ITEM_TITLE_LEN};

/// Top-level external-context summary for fast orientation.
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
