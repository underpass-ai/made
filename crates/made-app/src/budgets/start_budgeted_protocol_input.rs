use made_core::value_objects::{BudgetLimits, ProtocolId};

use crate::usecases::StartProtocolInput;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StartBudgetedProtocolInput {
    pub(crate) protocol: StartProtocolInput,
    pub(crate) limits: BudgetLimits,
}

impl StartBudgetedProtocolInput {
    #[must_use]
    pub const fn new(protocol: StartProtocolInput, limits: BudgetLimits) -> Self {
        Self { protocol, limits }
    }

    #[must_use]
    pub const fn protocol_id(&self) -> &ProtocolId {
        self.protocol.id()
    }
}
