use std::collections::BTreeMap;

use made_core::error::DomainError;
use made_core::value_objects::{
    Attributes, CeremonyStep, RetryPolicy, StateId, StepHandlerConfig, StepHandlerKind, StepId,
    StepTimeout,
};
use serde::Deserialize;
use serde_json::Value;

#[derive(Debug, Clone, Deserialize)]
pub(super) struct CeremonyStepDocument {
    id: String,
    state: String,
    handler: String,
    #[serde(default)]
    config: BTreeMap<String, Value>,
}

impl CeremonyStepDocument {
    pub(super) fn into_domain(
        self,
        retry_policy: RetryPolicy,
        timeout: Option<StepTimeout>,
    ) -> Result<CeremonyStep, DomainError> {
        Ok(CeremonyStep::new(
            StepId::new(self.id)?,
            StateId::new(self.state)?,
            StepHandlerKind::new(self.handler)?,
            StepHandlerConfig::new(Attributes::new(self.config)?),
            retry_policy,
            timeout,
        ))
    }
}
