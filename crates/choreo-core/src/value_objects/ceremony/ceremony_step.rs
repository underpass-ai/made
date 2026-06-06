use serde::{Deserialize, Serialize};

use super::{RetryPolicy, StateId, StepHandlerConfig, StepHandlerKind, StepId, StepTimeout};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CeremonyStep {
    id: StepId,
    state_id: StateId,
    handler_kind: StepHandlerKind,
    handler_config: StepHandlerConfig,
    retry_policy: RetryPolicy,
    timeout: Option<StepTimeout>,
}

impl CeremonyStep {
    #[must_use]
    pub fn new(
        id: StepId,
        state_id: StateId,
        handler_kind: StepHandlerKind,
        handler_config: StepHandlerConfig,
        retry_policy: RetryPolicy,
        timeout: Option<StepTimeout>,
    ) -> Self {
        Self {
            id,
            state_id,
            handler_kind,
            handler_config,
            retry_policy,
            timeout,
        }
    }

    #[must_use]
    pub fn id(&self) -> &StepId {
        &self.id
    }

    #[must_use]
    pub fn state_id(&self) -> &StateId {
        &self.state_id
    }

    #[must_use]
    pub fn handler_kind(&self) -> &StepHandlerKind {
        &self.handler_kind
    }

    #[must_use]
    pub fn handler_config(&self) -> &StepHandlerConfig {
        &self.handler_config
    }

    #[must_use]
    pub fn retry_policy(&self) -> RetryPolicy {
        self.retry_policy
    }

    #[must_use]
    pub fn timeout(&self) -> Option<StepTimeout> {
        self.timeout
    }
}
