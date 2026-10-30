use serde::{Deserialize, Serialize};

use crate::value_objects::GuardName;

use super::SystemProtocolId;

/// One guard of one composed protocol, named from the system above it.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct GuardRef {
    protocol: SystemProtocolId,
    guard_name: GuardName,
}

impl GuardRef {
    #[must_use]
    pub const fn new(protocol: SystemProtocolId, guard_name: GuardName) -> Self {
        Self {
            protocol,
            guard_name,
        }
    }

    #[must_use]
    pub const fn protocol(&self) -> &SystemProtocolId {
        &self.protocol
    }

    #[must_use]
    pub const fn guard_name(&self) -> &GuardName {
        &self.guard_name
    }
}
