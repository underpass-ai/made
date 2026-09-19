use serde::{Deserialize, Serialize};

use super::{ExecutionProfileFallbackPolicy, ExecutionProfileInheritance};
use crate::error::DomainError;

const MAX_PROFILE_TEXT: usize = 256;

/// Host-owned execution selection recorded with a claimed step.
///
/// The ceremony definition remains provider-neutral. The host supplies the
/// requested selection and records the actual selection it used, including
/// an explicit fallback, agent incarnation and any checkpoint handoff.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExecutionProfile {
    requested_model: String,
    requested_reasoning_effort: String,
    required_capabilities: Vec<String>,
    fallback_policy: ExecutionProfileFallbackPolicy,
    fallback_model: Option<String>,
    fallback_reasoning_effort: Option<String>,
    actual_model: String,
    actual_reasoning_effort: String,
    actual_capabilities: Vec<String>,
    host_agent_id: String,
    host_agent_incarnation: String,
    inherited_from: Option<ExecutionProfileInheritance>,
    checkpoint_id: Option<String>,
    handoff_from: Option<String>,
}

impl ExecutionProfile {
    pub fn from_json(value: serde_json::Value) -> Result<Self, DomainError> {
        let profile: Self =
            serde_json::from_value(value).map_err(|_| DomainError::InvariantViolated {
                reason: "invalid execution profile payload",
            })?;
        Self::new(
            profile.requested_model,
            profile.requested_reasoning_effort,
            profile.required_capabilities,
            profile.fallback_policy,
            profile.fallback_model,
            profile.fallback_reasoning_effort,
            profile.actual_model,
            profile.actual_reasoning_effort,
            profile.actual_capabilities,
            profile.host_agent_id,
            profile.host_agent_incarnation,
            profile.inherited_from,
            profile.checkpoint_id,
            profile.handoff_from,
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub fn new(
        requested_model: impl Into<String>,
        requested_reasoning_effort: impl Into<String>,
        required_capabilities: Vec<String>,
        fallback_policy: ExecutionProfileFallbackPolicy,
        fallback_model: Option<String>,
        fallback_reasoning_effort: Option<String>,
        actual_model: impl Into<String>,
        actual_reasoning_effort: impl Into<String>,
        actual_capabilities: Vec<String>,
        host_agent_id: impl Into<String>,
        host_agent_incarnation: impl Into<String>,
        inherited_from: Option<ExecutionProfileInheritance>,
        checkpoint_id: Option<String>,
        handoff_from: Option<String>,
    ) -> Result<Self, DomainError> {
        let profile = Self {
            requested_model: clean(requested_model.into(), "execution_profile.requested_model")?,
            requested_reasoning_effort: clean(
                requested_reasoning_effort.into(),
                "execution_profile.requested_reasoning_effort",
            )?,
            required_capabilities: clean_list(
                required_capabilities,
                "execution_profile.required_capabilities",
            )?,
            fallback_policy,
            fallback_model: fallback_model
                .map(|value| clean(value, "execution_profile.fallback_model"))
                .transpose()?,
            fallback_reasoning_effort: fallback_reasoning_effort
                .map(|value| clean(value, "execution_profile.fallback_reasoning_effort"))
                .transpose()?,
            actual_model: clean(actual_model.into(), "execution_profile.actual_model")?,
            actual_reasoning_effort: clean(
                actual_reasoning_effort.into(),
                "execution_profile.actual_reasoning_effort",
            )?,
            actual_capabilities: clean_list(
                actual_capabilities,
                "execution_profile.actual_capabilities",
            )?,
            host_agent_id: clean(host_agent_id.into(), "execution_profile.host_agent_id")?,
            host_agent_incarnation: clean(
                host_agent_incarnation.into(),
                "execution_profile.host_agent_incarnation",
            )?,
            inherited_from,
            checkpoint_id: checkpoint_id
                .map(|value| clean(value, "execution_profile.checkpoint_id"))
                .transpose()?,
            handoff_from: handoff_from
                .map(|value| clean(value, "execution_profile.handoff_from"))
                .transpose()?,
        };
        profile.validate_resolution()?;
        Ok(profile)
    }

    /// Validate the host's requested-versus-actual selection.
    pub fn validate_resolution(&self) -> Result<(), DomainError> {
        if self
            .required_capabilities
            .iter()
            .any(|required| !self.actual_capabilities.contains(required))
        {
            return Err(DomainError::InvariantViolated {
                reason:
                    "actual execution profile capabilities do not satisfy required capabilities",
            });
        }
        match self.fallback_policy {
            ExecutionProfileFallbackPolicy::Reject
                if self.actual_model != self.requested_model
                    || self.actual_reasoning_effort != self.requested_reasoning_effort =>
            {
                Err(DomainError::InvariantViolated {
                    reason:
                        "requested execution profile is unsupported and fallback_policy is reject",
                })
            }
            ExecutionProfileFallbackPolicy::Fallback => {
                if self.actual_model != self.requested_model
                    && self
                        .fallback_model
                        .as_deref()
                        .is_some_and(|fallback| fallback != self.actual_model)
                {
                    return Err(DomainError::InvariantViolated {
                        reason: "declared fallback model does not match actual model",
                    });
                }
                if self.actual_reasoning_effort != self.requested_reasoning_effort
                    && self
                        .fallback_reasoning_effort
                        .as_deref()
                        .is_some_and(|fallback| fallback != self.actual_reasoning_effort)
                {
                    return Err(DomainError::InvariantViolated {
                        reason: "declared fallback reasoning effort does not match actual effort",
                    });
                }
                if self.actual_model != self.requested_model
                    && self.fallback_model.as_deref() != Some(self.actual_model.as_str())
                {
                    return Err(DomainError::InvariantViolated {
                        reason: "actual model is not the declared execution profile fallback",
                    });
                }
                if self.actual_reasoning_effort != self.requested_reasoning_effort
                    && self.fallback_reasoning_effort.as_deref()
                        != Some(self.actual_reasoning_effort.as_str())
                {
                    return Err(DomainError::InvariantViolated {
                        reason:
                            "actual reasoning effort is not the declared execution profile fallback",
                    });
                }
                Ok(())
            }
            ExecutionProfileFallbackPolicy::Reject => Ok(()),
        }
    }

    #[must_use]
    pub fn requested_model(&self) -> &str {
        &self.requested_model
    }
    #[must_use]
    pub fn requested_reasoning_effort(&self) -> &str {
        &self.requested_reasoning_effort
    }
    #[must_use]
    pub fn required_capabilities(&self) -> &[String] {
        &self.required_capabilities
    }
    #[must_use]
    pub const fn fallback_policy(&self) -> ExecutionProfileFallbackPolicy {
        self.fallback_policy
    }
    #[must_use]
    pub fn actual_model(&self) -> &str {
        &self.actual_model
    }
    #[must_use]
    pub fn actual_reasoning_effort(&self) -> &str {
        &self.actual_reasoning_effort
    }
    #[must_use]
    pub fn actual_capabilities(&self) -> &[String] {
        &self.actual_capabilities
    }
    #[must_use]
    pub fn host_agent_id(&self) -> &str {
        &self.host_agent_id
    }
    #[must_use]
    pub fn host_agent_incarnation(&self) -> &str {
        &self.host_agent_incarnation
    }
    #[must_use]
    pub fn inherited_from(&self) -> Option<ExecutionProfileInheritance> {
        self.inherited_from
    }
    #[must_use]
    pub fn checkpoint_id(&self) -> Option<&str> {
        self.checkpoint_id.as_deref()
    }
    #[must_use]
    pub fn handoff_from(&self) -> Option<&str> {
        self.handoff_from.as_deref()
    }
}

fn clean(value: impl Into<String>, field: &'static str) -> Result<String, DomainError> {
    let value = value.into().trim().to_owned();
    if value.is_empty() {
        return Err(DomainError::EmptyField { field });
    }
    if value.len() > MAX_PROFILE_TEXT {
        return Err(DomainError::FieldTooLong {
            field,
            actual: value.len(),
            max: MAX_PROFILE_TEXT,
        });
    }
    if value.chars().any(char::is_control) {
        return Err(DomainError::InvalidCharacters { field });
    }
    Ok(value)
}

fn clean_list(values: Vec<String>, field: &'static str) -> Result<Vec<String>, DomainError> {
    values
        .into_iter()
        .map(|value| clean(value, field))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn profile(
        policy: ExecutionProfileFallbackPolicy,
        actual_model: &str,
    ) -> Result<ExecutionProfile, DomainError> {
        ExecutionProfile::new(
            "strong-model",
            "high",
            vec!["reasoning".into()],
            policy,
            Some("balanced-model".into()),
            Some("medium".into()),
            actual_model,
            if actual_model == "strong-model" {
                "high"
            } else {
                "medium"
            },
            vec!["reasoning".into()],
            "codex-agent-1",
            "inc-2",
            Some(ExecutionProfileInheritance::RoleDefault),
            Some("checkpoint-7".into()),
            Some("codex-agent-0/inc-1".into()),
        )
    }

    #[test]
    fn fallback_is_explicit_and_actionable() {
        assert!(profile(ExecutionProfileFallbackPolicy::Fallback, "balanced-model").is_ok());
        assert!(profile(ExecutionProfileFallbackPolicy::Reject, "balanced-model").is_err());
    }

    #[test]
    fn requested_and_actual_identity_are_retained() {
        let selected = profile(ExecutionProfileFallbackPolicy::Fallback, "balanced-model").unwrap();
        assert_eq!(selected.requested_model(), "strong-model");
        assert_eq!(selected.actual_model(), "balanced-model");
        assert_eq!(
            selected.inherited_from(),
            Some(ExecutionProfileInheritance::RoleDefault)
        );
        assert_eq!(selected.handoff_from(), Some("codex-agent-0/inc-1"));
    }

    #[test]
    fn required_capabilities_must_be_realised_by_actual_selection() {
        assert!(ExecutionProfile::new(
            "strong-model",
            "high",
            vec!["reasoning".into(), "vision".into()],
            ExecutionProfileFallbackPolicy::Reject,
            None,
            None,
            "strong-model",
            "high",
            vec!["reasoning".into()],
            "codex-agent-1",
            "inc-2",
            None,
            None,
            None,
        )
        .is_err());
    }

    #[test]
    fn fallback_model_and_effort_are_one_coherent_selection() {
        assert!(ExecutionProfile::new(
            "strong-model",
            "high",
            vec!["reasoning".into()],
            ExecutionProfileFallbackPolicy::Fallback,
            Some("balanced-model".into()),
            Some("high".into()),
            "balanced-model",
            "medium",
            vec!["reasoning".into()],
            "codex-agent-1",
            "inc-2",
            None,
            None,
            None,
        )
        .is_err());
    }

    #[test]
    fn supported_requested_selection_does_not_use_declared_fallback() {
        assert!(ExecutionProfile::new(
            "strong-model",
            "high",
            vec!["reasoning".into()],
            ExecutionProfileFallbackPolicy::Fallback,
            Some("balanced-model".into()),
            Some("medium".into()),
            "strong-model",
            "high",
            vec!["reasoning".into()],
            "codex-agent-1",
            "inc-2",
            None,
            None,
            None,
        )
        .is_ok());
    }
}
