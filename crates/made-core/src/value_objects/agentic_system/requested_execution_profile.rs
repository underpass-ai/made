use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

use crate::error::DomainError;
use crate::value_objects::ExecutionProfileFallbackPolicy;

use super::{Capability, ModelName, ReasoningEffort};

/// What a role's work asks of whatever executes it.
///
/// Requested, and only requested. The host decides what actually runs
/// and the claim already records it; a field here named for what ran
/// would be a promise the design cannot keep.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RequestedExecutionProfile {
    requested_model: ModelName,
    requested_reasoning_effort: ReasoningEffort,
    #[serde(default, skip_serializing_if = "BTreeSet::is_empty")]
    required_capabilities: BTreeSet<Capability>,
    fallback_policy: ExecutionProfileFallbackPolicy,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    fallback_model: Option<ModelName>,
}

impl RequestedExecutionProfile {
    /// Construct a request whose fallback declaration is coherent.
    ///
    /// Declaring a fallback model under a `reject` policy states two
    /// incompatible things about the same situation, and a run would
    /// have to pick one of them in silence.
    pub fn new(
        requested_model: ModelName,
        requested_reasoning_effort: ReasoningEffort,
        required_capabilities: impl IntoIterator<Item = Capability>,
        fallback_policy: ExecutionProfileFallbackPolicy,
        fallback_model: Option<ModelName>,
    ) -> Result<Self, DomainError> {
        if matches!(fallback_policy, ExecutionProfileFallbackPolicy::Reject)
            && fallback_model.is_some()
        {
            return Err(DomainError::InvariantViolated {
                reason: "a requested profile that refuses fallback cannot declare one",
            });
        }
        Ok(Self {
            requested_model,
            requested_reasoning_effort,
            required_capabilities: required_capabilities.into_iter().collect(),
            fallback_policy,
            fallback_model,
        })
    }

    #[must_use]
    pub const fn requested_model(&self) -> &ModelName {
        &self.requested_model
    }

    #[must_use]
    pub const fn requested_reasoning_effort(&self) -> &ReasoningEffort {
        &self.requested_reasoning_effort
    }

    #[must_use]
    pub const fn required_capabilities(&self) -> &BTreeSet<Capability> {
        &self.required_capabilities
    }

    #[must_use]
    pub const fn fallback_policy(&self) -> ExecutionProfileFallbackPolicy {
        self.fallback_policy
    }

    #[must_use]
    pub const fn fallback_model(&self) -> Option<&ModelName> {
        self.fallback_model.as_ref()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn profile(
        policy: ExecutionProfileFallbackPolicy,
        fallback: Option<&str>,
    ) -> Result<RequestedExecutionProfile, DomainError> {
        RequestedExecutionProfile::new(
            ModelName::new("strong-model").unwrap(),
            ReasoningEffort::new("high").unwrap(),
            [Capability::new("reasoning").unwrap()],
            policy,
            fallback.map(|raw| ModelName::new(raw).unwrap()),
        )
    }

    #[test]
    fn refusing_fallback_and_declaring_one_is_two_answers_to_one_question() {
        assert!(profile(ExecutionProfileFallbackPolicy::Reject, None).is_ok());
        assert!(profile(ExecutionProfileFallbackPolicy::Reject, Some("weak")).is_err());
        assert!(profile(ExecutionProfileFallbackPolicy::Fallback, Some("weak")).is_ok());
    }
}
