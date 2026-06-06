//! Request passed to ceremony step handler adapters.

use crate::value_objects::{
    CeremonyContext, CeremonyId, CeremonyName, CeremonyVersion, StateId, StepAttempt,
    StepHandlerConfig, StepHandlerKind, StepId,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CeremonyStepHandlerRequest {
    instance_id: CeremonyId,
    definition_name: CeremonyName,
    definition_version: CeremonyVersion,
    current_state: StateId,
    step_id: StepId,
    handler_kind: StepHandlerKind,
    handler_config: StepHandlerConfig,
    context: CeremonyContext,
    attempt: StepAttempt,
}

impl CeremonyStepHandlerRequest {
    #[must_use]
    pub fn new(
        instance_id: CeremonyId,
        definition_name: CeremonyName,
        definition_version: CeremonyVersion,
        current_state: StateId,
        step_id: StepId,
        handler_kind: StepHandlerKind,
        handler_config: StepHandlerConfig,
        context: CeremonyContext,
        attempt: StepAttempt,
    ) -> Self {
        Self {
            instance_id,
            definition_name,
            definition_version,
            current_state,
            step_id,
            handler_kind,
            handler_config,
            context,
            attempt,
        }
    }

    #[must_use]
    pub fn instance_id(&self) -> &CeremonyId {
        &self.instance_id
    }

    #[must_use]
    pub fn definition_name(&self) -> &CeremonyName {
        &self.definition_name
    }

    #[must_use]
    pub fn definition_version(&self) -> &CeremonyVersion {
        &self.definition_version
    }

    #[must_use]
    pub fn current_state(&self) -> &StateId {
        &self.current_state
    }

    #[must_use]
    pub fn step_id(&self) -> &StepId {
        &self.step_id
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
    pub fn context(&self) -> &CeremonyContext {
        &self.context
    }

    #[must_use]
    pub fn attempt(&self) -> StepAttempt {
        self.attempt
    }
}
