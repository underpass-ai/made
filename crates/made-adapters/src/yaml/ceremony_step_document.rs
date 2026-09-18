use std::collections::BTreeMap;

use made_core::error::DomainError;
use made_core::value_objects::{
    Attributes, CeremonyStep, CeremonyStepAggregation, ContextKey, ContextWrites,
    DynamicRoleBinding, RetryPolicy, RoleId, StateId, StepHandlerConfig, StepHandlerKind, StepId,
    StepOutputField, StepTimeout,
};
use serde::Deserialize;
use serde_json::Value;

use super::step_repeat_policy_document::StepRepeatPolicyDocument;

#[derive(Debug, Clone, Deserialize)]
pub(super) struct CeremonyStepDocument {
    id: String,
    state: String,
    handler: String,
    #[serde(default)]
    config: BTreeMap<String, Value>,
    #[serde(default)]
    repeat: Option<StepRepeatPolicyDocument>,
    #[serde(default)]
    role_from: Option<String>,
    #[serde(default)]
    allowed_roles: Vec<String>,
    #[serde(default)]
    context_writes: BTreeMap<String, String>,
    #[serde(default)]
    aggregate: Option<CeremonyStepAggregation>,
}

impl CeremonyStepDocument {
    pub(super) fn into_domain(
        self,
        retry_policy: RetryPolicy,
        timeout: Option<StepTimeout>,
    ) -> Result<CeremonyStep, DomainError> {
        let step = CeremonyStep::new(
            StepId::new(self.id)?,
            StateId::new(self.state)?,
            StepHandlerKind::new(self.handler)?,
            StepHandlerConfig::new(Attributes::new(self.config)?),
            retry_policy,
            timeout,
        );
        let mut step = match self.repeat {
            Some(repeat) => step.with_repeat_policy(repeat.into_domain()?),
            None => step,
        };
        match (self.role_from, self.allowed_roles.is_empty()) {
            (Some(role_from), false) => {
                step = step.with_dynamic_role_binding(DynamicRoleBinding::new(
                    ContextKey::from_role_from(&role_from)?,
                    self.allowed_roles
                        .into_iter()
                        .map(RoleId::new)
                        .collect::<Result<Vec<_>, _>>()?,
                )?);
            }
            (None, true) => {}
            _ => {
                return Err(DomainError::InvariantViolated {
                    reason: "role_from and allowed_roles must be declared together",
                });
            }
        }
        let writes = self
            .context_writes
            .into_iter()
            .map(|(destination, source)| {
                Ok((ContextKey::new(destination)?, StepOutputField::new(source)?))
            })
            .collect::<Result<BTreeMap<_, _>, DomainError>>()?;
        let step = step.with_context_writes(ContextWrites::new(writes));
        Ok(match self.aggregate {
            Some(aggregation) => step.with_aggregation(aggregation),
            None => step,
        })
    }
}
