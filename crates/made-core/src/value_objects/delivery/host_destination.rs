use serde::{Deserialize, Serialize};

use super::{HostActivationMode, HostAddress, HostKind};

/// Where an integrator lives, in terms the engine keeps opaque.
///
/// The address is never parsed and never executed: it is handed to the
/// activation adapter as an environment value, and what it means is the
/// operator's business. That is what keeps a payload from becoming a
/// command.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HostDestination {
    host_kind: HostKind,
    address: HostAddress,
    activation: HostActivationMode,
}

impl HostDestination {
    #[must_use]
    pub const fn new(
        host_kind: HostKind,
        address: HostAddress,
        activation: HostActivationMode,
    ) -> Self {
        Self {
            host_kind,
            address,
            activation,
        }
    }

    #[must_use]
    pub const fn host_kind(&self) -> &HostKind {
        &self.host_kind
    }

    #[must_use]
    pub const fn address(&self) -> &HostAddress {
        &self.address
    }

    #[must_use]
    pub const fn activation(&self) -> HostActivationMode {
        self.activation
    }
}
