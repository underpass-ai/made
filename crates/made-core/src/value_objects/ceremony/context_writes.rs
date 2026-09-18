use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::error::DomainError;

use super::{ContextKey, ContextPatch, StepOutput, StepOutputField};

/// Declared copies from top-level step output fields into ceremony context.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ContextWrites(BTreeMap<ContextKey, StepOutputField>);

impl ContextWrites {
    #[must_use]
    pub fn new(entries: BTreeMap<ContextKey, StepOutputField>) -> Self {
        Self(entries)
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    #[must_use]
    pub fn entries(&self) -> &BTreeMap<ContextKey, StepOutputField> {
        &self.0
    }

    pub fn resolve(&self, output: &StepOutput) -> Result<Option<ContextPatch>, DomainError> {
        if self.is_empty() {
            return Ok(None);
        }
        let mut patch = BTreeMap::new();
        for (destination, source) in &self.0 {
            let value = output
                .attributes()
                .get(source.as_str())
                .ok_or(DomainError::NotFound {
                    what: "ceremony_step.context_write.output_field",
                })?;
            patch.insert(destination.clone(), value.clone());
        }
        Ok(Some(ContextPatch::new(patch)?))
    }
}
