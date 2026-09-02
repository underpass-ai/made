use serde::{Deserialize, Serialize};

use super::{
    RetryPolicy, StateId, StepHandlerConfig, StepHandlerKind, StepId, StepRepeatPolicy, StepTimeout,
};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CeremonyStep {
    id: StepId,
    state_id: StateId,
    handler_kind: StepHandlerKind,
    handler_config: StepHandlerConfig,
    retry_policy: RetryPolicy,
    timeout: Option<StepTimeout>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    repeat_policy: Option<StepRepeatPolicy>,
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
            repeat_policy: None,
        }
    }

    #[must_use]
    pub fn with_repeat_policy(mut self, repeat_policy: StepRepeatPolicy) -> Self {
        self.repeat_policy = Some(repeat_policy);
        self
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

    #[must_use]
    pub fn repeat_policy(&self) -> Option<&StepRepeatPolicy> {
        self.repeat_policy.as_ref()
    }
}
