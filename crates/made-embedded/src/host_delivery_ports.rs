use std::fmt;
use std::sync::Arc;

use made_core::ports::{HostActivationPort, HostDeliveryLedgerPort, IntegratorBindingPort};

/// The three ports a host's work travels through, composed as one.
///
/// One field rather than three because they are never chosen
/// separately: a durable ledger with in-memory bindings would hand a
/// restarted host work addressed to a binding that no longer exists.
#[derive(Clone)]
pub struct HostDeliveryPorts {
    ledger: Arc<dyn HostDeliveryLedgerPort>,
    bindings: Arc<dyn IntegratorBindingPort>,
    activation: Arc<dyn HostActivationPort>,
}

impl HostDeliveryPorts {
    #[must_use]
    pub const fn new(
        ledger: Arc<dyn HostDeliveryLedgerPort>,
        bindings: Arc<dyn IntegratorBindingPort>,
        activation: Arc<dyn HostActivationPort>,
    ) -> Self {
        Self {
            ledger,
            bindings,
            activation,
        }
    }

    /// Work handed out to hosts.
    #[must_use]
    pub fn ledger(&self) -> &Arc<dyn HostDeliveryLedgerPort> {
        &self.ledger
    }

    /// Who is driving each ceremony.
    #[must_use]
    pub fn bindings(&self) -> &Arc<dyn IntegratorBindingPort> {
        &self.bindings
    }

    /// How a host that does not ask gets woken, if it does.
    #[must_use]
    pub fn activation(&self) -> &Arc<dyn HostActivationPort> {
        &self.activation
    }
}

impl fmt::Debug for HostDeliveryPorts {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("HostDeliveryPorts")
            .field("activation", &self.activation.kind())
            .finish()
    }
}
