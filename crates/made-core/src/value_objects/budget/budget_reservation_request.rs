use serde::{Deserialize, Serialize};

use crate::value_objects::{
    ProtocolContext, ProtocolId, StepHandlerConfig, StepHandlerKind, StepId,
};

/// Semantic input a host or handler uses to estimate one operation before it is claimed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BudgetReservationRequest {
    protocol_id: ProtocolId,
    step_id: StepId,
    handler_kind: StepHandlerKind,
    handler_config: StepHandlerConfig,
    context: ProtocolContext,
}

impl BudgetReservationRequest {
    #[must_use]
    pub const fn new(
        protocol_id: ProtocolId,
        step_id: StepId,
        handler_kind: StepHandlerKind,
        handler_config: StepHandlerConfig,
        context: ProtocolContext,
    ) -> Self {
        Self {
            protocol_id,
            step_id,
            handler_kind,
            handler_config,
            context,
        }
    }

    #[must_use]
    pub const fn protocol_id(&self) -> &ProtocolId {
        &self.protocol_id
    }
    #[must_use]
    pub const fn step_id(&self) -> &StepId {
        &self.step_id
    }
    #[must_use]
    pub const fn handler_kind(&self) -> &StepHandlerKind {
        &self.handler_kind
    }
    #[must_use]
    pub const fn handler_config(&self) -> &StepHandlerConfig {
        &self.handler_config
    }
    #[must_use]
    pub const fn context(&self) -> &ProtocolContext {
        &self.context
    }
}
