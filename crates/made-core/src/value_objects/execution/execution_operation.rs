use serde::{Deserialize, Serialize};

use super::{ExecutionOperationId, ExecutionRequestBytes, ExecutionRequestDigest};
use crate::error::DomainError;
use crate::value_objects::ceremony::{
    CeremonyId, StateIteration, StateVisit, StepId, StepIteration,
};

/// Semantic operation root shared by every technical claim that may execute it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExecutionOperation {
    operation_id: ExecutionOperationId,
    ceremony_id: CeremonyId,
    step_id: StepId,
    state_visit: StateVisit,
    state_iteration: StateIteration,
    step_iteration: StepIteration,
    request: ExecutionRequestBytes,
    request_digest: ExecutionRequestDigest,
}

impl ExecutionOperation {
    #[must_use]
    pub fn new(
        ceremony_id: CeremonyId,
        step_id: StepId,
        state_visit: StateVisit,
        state_iteration: StateIteration,
        step_iteration: StepIteration,
        request: ExecutionRequestBytes,
    ) -> Self {
        let operation_id = ExecutionOperationId::for_step(
            &ceremony_id,
            &step_id,
            state_visit,
            state_iteration,
            step_iteration,
        );
        let request_digest = request.digest();
        Self {
            operation_id,
            ceremony_id,
            step_id,
            state_visit,
            state_iteration,
            step_iteration,
            request,
            request_digest,
        }
    }

    #[must_use]
    pub const fn operation_id(&self) -> &ExecutionOperationId {
        &self.operation_id
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
    pub const fn state_visit(&self) -> StateVisit {
        self.state_visit
    }

    #[must_use]
    pub const fn state_iteration(&self) -> StateIteration {
        self.state_iteration
    }

    #[must_use]
    pub const fn step_iteration(&self) -> StepIteration {
        self.step_iteration
    }

    #[must_use]
    pub const fn request(&self) -> &ExecutionRequestBytes {
        &self.request
    }

    #[must_use]
    pub const fn request_digest(&self) -> &ExecutionRequestDigest {
        &self.request_digest
    }

    /// Re-check derived identity and request digest after deserialization.
    pub fn validate(&self) -> Result<(), DomainError> {
        let expected_id = ExecutionOperationId::for_step(
            &self.ceremony_id,
            &self.step_id,
            self.state_visit,
            self.state_iteration,
            self.step_iteration,
        );
        if self.operation_id != expected_id || self.request_digest != self.request.digest() {
            return Err(DomainError::InvariantViolated {
                reason: "execution operation derived fields do not match its semantic input",
            });
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    fn operation() -> ExecutionOperation {
        ExecutionOperation::new(
            CeremonyId::new("ceremony").unwrap(),
            StepId::new("work").unwrap(),
            StateVisit::FIRST,
            StateIteration::FIRST,
            StepIteration::FIRST,
            ExecutionRequestBytes::new(b"semantic request".to_vec()).unwrap(),
        )
    }

    #[test]
    fn deserialized_derived_fields_are_revalidated_before_use() {
        let original = operation();
        original.validate().unwrap();
        let mut raw = serde_json::to_value(original).unwrap();
        raw["request_digest"] = json!("0".repeat(64));
        let corrupted: ExecutionOperation = serde_json::from_value(raw).unwrap();
        assert!(corrupted.validate().is_err());
    }
}
