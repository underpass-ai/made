use serde::{Deserialize, Serialize};

use crate::value_objects::{
    CeremonyContext, CeremonyId, StepHandlerConfig, StepHandlerKind, StepId,
};

/// Semantic input a host or handler uses to estimate one operation before it is claimed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BudgetReservationRequest {
    ceremony_id: CeremonyId,
    step_id: StepId,
    handler_kind: StepHandlerKind,
    handler_config: StepHandlerConfig,
    context: CeremonyContext,
}

impl BudgetReservationRequest {
    #[must_use]
    pub const fn new(
        ceremony_id: CeremonyId,
        step_id: StepId,
        handler_kind: StepHandlerKind,
        handler_config: StepHandlerConfig,
        context: CeremonyContext,
    ) -> Self {
        Self {
            ceremony_id,
            step_id,
            handler_kind,
            handler_config,
            context,
        }
    }

    #[must_use]
    pub const fn ceremony_id(&self) -> &CeremonyId {
        &self.ceremony_id
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
    pub const fn context(&self) -> &CeremonyContext {
        &self.context
    }
}
