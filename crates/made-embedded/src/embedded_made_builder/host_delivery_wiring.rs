use std::fmt;
use std::sync::Arc;

use made_adapters::activation::NoHostActivation;
use made_adapters::memory::{InMemoryHostDeliveryLedger, InMemoryIntegratorBindings};
use made_core::ports::{HostActivationPort, HostDeliveryLedgerPort, IntegratorBindingPort};

use crate::host_delivery_ports::HostDeliveryPorts;
use crate::EmbeddedMadeBuilder;

/// The delivery adapters a host has chosen, before defaults fill the rest.
#[derive(Default)]
pub(crate) struct HostDeliveryWiring {
    ledger: Option<Arc<dyn HostDeliveryLedgerPort>>,
    bindings: Option<Arc<dyn IntegratorBindingPort>>,
    activation: Option<Arc<dyn HostActivationPort>>,
}

impl HostDeliveryWiring {
    /// Fill what the host did not choose.
    ///
    /// In memory, and no activation: a process that forgets its
    /// deliveries on restart is a normal embedded host, and a
    /// deployment that does not wake anybody says so rather than
    /// pretending a hand-off happened.
    pub(crate) fn resolve(self) -> HostDeliveryPorts {
        HostDeliveryPorts::new(
            self.ledger
                .unwrap_or_else(|| Arc::new(InMemoryHostDeliveryLedger::new())),
            self.bindings
                .unwrap_or_else(|| Arc::new(InMemoryIntegratorBindings::new())),
            self.activation
                .unwrap_or_else(|| Arc::new(NoHostActivation::new())),
        )
    }
}

impl EmbeddedMadeBuilder {
    /// Where work handed out to hosts is durably held.
    #[must_use]
    pub fn with_host_delivery_ledger(mut self, adapter: Arc<dyn HostDeliveryLedgerPort>) -> Self {
        self.host_delivery.ledger = Some(adapter);
        self
    }

    /// Where the record of who is driving each ceremony lives.
    #[must_use]
    pub fn with_integrator_bindings(mut self, adapter: Arc<dyn IntegratorBindingPort>) -> Self {
        self.host_delivery.bindings = Some(adapter);
        self
    }

    /// How a host that does not ask for its work gets woken.
    #[must_use]
    pub fn with_host_activation(mut self, adapter: Arc<dyn HostActivationPort>) -> Self {
        self.host_delivery.activation = Some(adapter);
        self
    }
}

impl fmt::Debug for HostDeliveryWiring {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("HostDeliveryWiring")
            .field("has_ledger", &self.ledger.is_some())
            .field("has_bindings", &self.bindings.is_some())
            .field("has_activation", &self.activation.is_some())
            .finish()
    }
}
