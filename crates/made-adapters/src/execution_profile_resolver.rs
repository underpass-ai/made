use std::collections::BTreeSet;

use made_core::error::DomainError;
use made_core::ports::ExecutionProfileResolverPort;
use made_core::value_objects::ExecutionProfile;

/// Host adapter for a finite model/capability inventory.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StaticExecutionProfileResolver {
    models: BTreeSet<String>,
    capabilities: BTreeSet<String>,
}

impl StaticExecutionProfileResolver {
    #[must_use]
    pub fn new(
        models: impl IntoIterator<Item = impl Into<String>>,
        capabilities: impl IntoIterator<Item = impl Into<String>>,
    ) -> Self {
        Self {
            models: models.into_iter().map(Into::into).collect(),
            capabilities: capabilities.into_iter().map(Into::into).collect(),
        }
    }
}

impl ExecutionProfileResolverPort for StaticExecutionProfileResolver {
    fn resolve(&self, profile: &ExecutionProfile) -> Result<ExecutionProfile, DomainError> {
        if !self.models.contains(profile.actual_model()) {
            return Err(DomainError::InvariantViolated {
                reason: "host does not support actual execution profile model; provide a supported fallback",
            });
        }
        if profile
            .required_capabilities()
            .iter()
            .any(|capability| !self.capabilities.contains(capability))
        {
            return Err(DomainError::InvariantViolated {
                reason: "host lacks a required execution profile capability; provide an actionable fallback",
            });
        }
        if profile
            .actual_capabilities()
            .iter()
            .any(|capability| !self.capabilities.contains(capability))
        {
            return Err(DomainError::InvariantViolated {
                reason: "actual execution capability is not in the host inventory; select a supported fallback",
            });
        }
        profile.validate_resolution()?;
        Ok(profile.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use made_core::ports::ExecutionProfileResolverPort;
    use made_core::value_objects::ExecutionProfileFallbackPolicy;

    fn profile() -> ExecutionProfile {
        ExecutionProfile::new(
            "strong",
            "high",
            vec!["review".into()],
            ExecutionProfileFallbackPolicy::Fallback,
            Some("balanced".into()),
            Some("medium".into()),
            "balanced",
            "medium",
            vec!["review".into()],
            "agent",
            "inc-1",
            None,
            None,
            None,
        )
        .unwrap()
    }

    #[test]
    fn unsupported_model_is_actionable() {
        let resolver = StaticExecutionProfileResolver::new(["small"], ["review"]);
        assert!(resolver
            .resolve(&profile())
            .unwrap_err()
            .to_string()
            .contains("supported fallback"));
    }

    #[test]
    fn supported_fallback_is_accepted() {
        let resolver = StaticExecutionProfileResolver::new(["balanced"], ["review"]);
        assert!(resolver.resolve(&profile()).is_ok());
    }

    #[test]
    fn invented_actual_capability_is_rejected() {
        let resolver = StaticExecutionProfileResolver::new(["balanced"], ["review"]);
        let selected = ExecutionProfile::new(
            "strong",
            "high",
            vec!["review".into()],
            ExecutionProfileFallbackPolicy::Fallback,
            Some("balanced".into()),
            Some("medium".into()),
            "balanced",
            "medium",
            vec!["review".into(), "invented".into()],
            "agent",
            "inc-1",
            None,
            None,
            None,
        )
        .unwrap();
        let error = resolver.resolve(&selected).unwrap_err();
        assert!(error.to_string().contains("not in the host inventory"));
    }
}
