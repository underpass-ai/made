use serde::{Deserialize, Serialize};

use crate::value_objects::GuardName;

use super::SystemCeremonyId;

/// One guard of one composed ceremony, named from the system above it.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct GuardRef {
    ceremony: SystemCeremonyId,
    guard_name: GuardName,
}

impl GuardRef {
    #[must_use]
    pub const fn new(ceremony: SystemCeremonyId, guard_name: GuardName) -> Self {
        Self {
            ceremony,
            guard_name,
        }
    }

    #[must_use]
    pub const fn ceremony(&self) -> &SystemCeremonyId {
        &self.ceremony
    }

    #[must_use]
    pub const fn guard_name(&self) -> &GuardName {
        &self.guard_name
    }
}
