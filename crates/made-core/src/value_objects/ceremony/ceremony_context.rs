use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::error::DomainError;
use crate::value_objects::Attributes;

use super::{ContextKey, ContextPatch, GuardName};

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct CeremonyContext(Attributes);

impl CeremonyContext {
    #[must_use]
    pub fn new(attributes: Attributes) -> Self {
        Self(attributes)
    }

    #[must_use]
    pub fn empty() -> Self {
        Self::default()
    }

    pub fn with_guard_approval(self, guard_name: &GuardName) -> Result<Self, DomainError> {
        let mut entries = self.0.into_inner();
        entries.insert(guard_name.as_str().to_owned(), Value::Bool(true));
        Ok(Self(Attributes::new(entries)?))
    }

    pub fn with_patch(self, patch: &ContextPatch) -> Result<Self, DomainError> {
        let mut entries = self.0.into_inner();
        for (key, value) in patch.entries() {
            entries.insert(key.as_str().to_owned(), value.clone());
        }
        Ok(Self(Attributes::new(entries)?))
    }

    #[must_use]
    pub fn get(&self, key: &ContextKey) -> Option<&Value> {
        self.0.get(key.as_str())
    }

    #[must_use]
    pub fn is_guard_approved(&self, guard_name: &GuardName) -> bool {
        self.0
            .get(guard_name.as_str())
            .and_then(Value::as_bool)
            .unwrap_or(false)
    }

    #[must_use]
    pub fn attributes(&self) -> &Attributes {
        &self.0
    }
}
