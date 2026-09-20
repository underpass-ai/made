use std::sync::Arc;

use made_core::ports::{HostActivationPort, HostDeliveryLedgerPort, IntegratorBindingPort};

/// The delivery core the service composed, held for the commands that
/// will use it.
///
/// One handle rather than three fields on the application, because the
/// three are never chosen separately: a durable ledger beside in-memory
/// bindings would restart into a service holding work addressed to
/// hosts it no longer knows.
#[derive(Clone)]
pub struct HostDeliveryHandles {
    pub ledger: Arc<dyn HostDeliveryLedgerPort>,
    pub bindings: Arc<dyn IntegratorBindingPort>,
    pub activation: Arc<dyn HostActivationPort>,
}

impl std::fmt::Debug for HostDeliveryHandles {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("HostDeliveryHandles")
            .field("activation", &self.activation.kind())
            .finish()
    }
}
