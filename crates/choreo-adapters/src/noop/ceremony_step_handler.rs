//! No-op [`CeremonyStepHandlerPort`] implementation.

use async_trait::async_trait;
use choreo_core::error::DomainError;
use choreo_core::ports::{CeremonyStepHandlerPort, CeremonyStepHandlerRequest};
use choreo_core::value_objects::{StepOutput, StepResult};
use tracing::debug;

#[derive(Debug, Default, Clone, Copy)]
pub struct NoopCeremonyStepHandler;

impl NoopCeremonyStepHandler {
    #[must_use]
    pub const fn new() -> Self {
        Self
    }
}

#[async_trait]
impl CeremonyStepHandlerPort for NoopCeremonyStepHandler {
    async fn execute(
        &self,
        request: CeremonyStepHandlerRequest,
    ) -> Result<StepResult, DomainError> {
        debug!(
            ceremony_id = request.instance_id().as_str(),
            step_id = request.step_id().as_str(),
            handler_kind = request.handler_kind().as_str(),
            attempt = request.attempt().get(),
            "noop ceremony step handler: completing step"
        );
        StepResult::completed(StepOutput::empty())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use choreo_core::value_objects::Attributes;
    use choreo_core::value_objects::{
        CeremonyContext, CeremonyId, CeremonyName, CeremonyVersion, StateId, StepAttempt,
        StepHandlerConfig, StepHandlerKind, StepId, StepStatus,
    };
    use serde_json::json;
    use std::collections::BTreeMap;

    #[tokio::test]
    async fn completes_any_step() {
        let result = NoopCeremonyStepHandler::new()
            .execute(CeremonyStepHandlerRequest::new(
                CeremonyId::new("ceremony-1").unwrap(),
                CeremonyName::new("editorial_meeting").unwrap(),
                CeremonyVersion::v1(),
                StateId::new("STARTED").unwrap(),
                StepId::new("roundtable").unwrap(),
                StepHandlerKind::new("multiagent_round").unwrap(),
                StepHandlerConfig::empty(),
                CeremonyContext::empty(),
                StepAttempt::FIRST,
            ))
            .await
            .unwrap();

        assert_eq!(result.status(), StepStatus::Completed);
    }

    #[tokio::test]
    async fn ignores_handler_kind_and_config_without_leaking_output() {
        let mut config = BTreeMap::new();
        config.insert("prompt".to_owned(), json!("Discuss the brief"));
        let result = NoopCeremonyStepHandler::new()
            .execute(CeremonyStepHandlerRequest::new(
                CeremonyId::new("ceremony-1").unwrap(),
                CeremonyName::new("editorial_meeting").unwrap(),
                CeremonyVersion::v1(),
                StateId::new("STARTED").unwrap(),
                StepId::new("roundtable").unwrap(),
                StepHandlerKind::new("custom.handler").unwrap(),
                StepHandlerConfig::new(Attributes::new(config).unwrap()),
                CeremonyContext::empty(),
                StepAttempt::new(2).unwrap(),
            ))
            .await
            .unwrap();

        assert_eq!(result.status(), StepStatus::Completed);
        assert!(result.output().attributes().is_empty());
    }
}
