use serde::{Deserialize, Serialize};

use crate::error::DomainError;
use crate::value_objects::Attributes;

use super::external_context_validation::{
    normalize_optional, validate_text, MAX_REFERENCE_ID_LEN, MAX_URI_LEN,
};

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
