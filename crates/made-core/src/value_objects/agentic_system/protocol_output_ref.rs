use serde::{Deserialize, Serialize};

use crate::value_objects::OutputName;

use super::SystemProtocolId;

/// One output of one composed protocol, named from outside it.
///
/// This is how work flows between compositions without the system
/// holding the work itself: a later protocol reads a named output of
/// an earlier one, and validation checks the pinned definition really
/// declares it.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct ProtocolOutputRef {
    protocol: SystemProtocolId,
    output: OutputName,
}

impl ProtocolOutputRef {
    #[must_use]
    pub const fn new(protocol: SystemProtocolId, output: OutputName) -> Self {
        Self { protocol, output }
    }

    #[must_use]
    pub const fn protocol(&self) -> &SystemProtocolId {
        &self.protocol
    }

    #[must_use]
    pub const fn output(&self) -> &OutputName {
        &self.output
    }
}
