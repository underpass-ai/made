use std::fmt;

use serde::{Deserialize, Serialize};

/// The stable spelling of a delivery destination.
///
/// Its own type because it is an index key as much as a label: the
/// ledger scans by it, and a destination whose key changed between two
/// enqueues would hand the same host the same item twice.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct HostDeliveryTargetKey(String);

impl HostDeliveryTargetKey {
    pub(super) fn new(value: String) -> Self {
        Self(value)
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for HostDeliveryTargetKey {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}
