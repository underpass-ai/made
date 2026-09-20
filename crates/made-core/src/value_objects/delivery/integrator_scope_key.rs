use std::fmt;

use serde::{Deserialize, Serialize};

/// The stable spelling of what a binding is scoped to.
///
/// One binding per scope is the rule, and this key is what makes that
/// rule storable: the store holds one row per key, so a second live
/// binding for the same scope cannot exist by construction.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct IntegratorScopeKey(String);

impl IntegratorScopeKey {
    pub(super) fn new(value: String) -> Self {
        Self(value)
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for IntegratorScopeKey {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}
