use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::error::DomainError;

use super::ContextKey;

/// An atomic set of top-level ceremony-context replacements.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ContextPatch(BTreeMap<ContextKey, Value>);

impl ContextPatch {
    pub fn new(entries: BTreeMap<ContextKey, Value>) -> Result<Self, DomainError> {
        if entries.is_empty() {
            return Err(DomainError::EmptyCollection {
                field: "context_patch.entries",
            });
        }
        Ok(Self(entries))
    }

    #[must_use]
    pub fn entries(&self) -> &BTreeMap<ContextKey, Value> {
        &self.0
    }
}
