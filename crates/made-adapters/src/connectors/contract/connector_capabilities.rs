use super::ConnectorCapability;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConnectorCapabilities(BTreeSet<ConnectorCapability>);

impl ConnectorCapabilities {
    #[must_use]
    pub fn new(capabilities: impl IntoIterator<Item = ConnectorCapability>) -> Self {
        Self(capabilities.into_iter().collect())
    }
    #[must_use]
    pub fn empty() -> Self {
        Self(BTreeSet::new())
    }
    #[must_use]
    pub fn contains(&self, capability: ConnectorCapability) -> bool {
        self.0.contains(&capability)
    }
    pub fn iter(&self) -> impl Iterator<Item = ConnectorCapability> + '_ {
        self.0.iter().copied()
    }
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

impl fmt::Display for ConnectorCapabilities {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.debug_list().entries(self.0.iter()).finish()
    }
}
