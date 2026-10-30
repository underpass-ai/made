//! Request passed to protocol step handler adapters.

use crate::entities::ProtocolIntervention;
use crate::value_objects::{
    ExecutionRequestBytes, ProtocolContext, ProtocolId, ProtocolName, ProtocolTranscript,
    ProtocolVersion, RoleId, Specialty, StateId, StepAttempt, StepHandlerConfig, StepHandlerKind,
    StepId,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProtocolStepHandlerRequest {
    instance_id: ProtocolId,
    definition_name: ProtocolName,
    definition_version: ProtocolVersion,
    current_state: StateId,
    step_id: StepId,
    handler_kind: StepHandlerKind,
    handler_config: StepHandlerConfig,
    context: ProtocolContext,
    attempt: StepAttempt,
    transcript: ProtocolTranscript,
    interventions: Vec<ProtocolIntervention>,
    role_id: Option<RoleId>,
    /// The specialty this session seated for the role, when it seated
    /// one. Absent means the step's own configuration decides, which
    /// is how every session behaved before seating existed.
    bound_specialty: Option<Specialty>,
}

impl ProtocolStepHandlerRequest {
    #[must_use]
    pub fn new(
        instance_id: ProtocolId,
        definition_name: ProtocolName,
        definition_version: ProtocolVersion,
        current_state: StateId,
        step_id: StepId,
        handler_kind: StepHandlerKind,
        handler_config: StepHandlerConfig,
        context: ProtocolContext,
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
            transcript: ProtocolTranscript::empty(),
            interventions: Vec::new(),
            role_id: None,
            bound_specialty: None,
        }
    }

    /// Attach the prior-step transcript the engine accumulated for this
    /// protocol, so the handler can deliberate with earlier interventions
    /// in view. Defaults to an empty transcript when not set.
    #[must_use]
    pub fn with_transcript(mut self, transcript: ProtocolTranscript) -> Self {
        self.transcript = transcript;
        self
    }

    /// Attach the participant-created agenda accumulated by the running
    /// protocol. Declaration order is retained so handlers see the same
    /// conversational sequence as the participants.
    #[must_use]
    pub fn with_interventions(mut self, interventions: Vec<ProtocolIntervention>) -> Self {
        self.interventions = interventions;
        self
    }

    /// Attach the specialty this session seated for the role, so the
    /// handler puts the work to that panel instead of the one the step
    /// declares. `None` leaves the step's own configuration deciding.
    #[must_use]
    pub fn with_bound_specialty(mut self, specialty: Option<Specialty>) -> Self {
        self.bound_specialty = specialty;
        self
    }

    /// Attach the protocol role this step is executed as, so the handler
    /// can frame the agent's persona. Defaults to none when not set.
    #[must_use]
    pub fn with_role(mut self, role_id: RoleId) -> Self {
        self.role_id = Some(role_id);
        self
    }

    #[must_use]
    pub fn instance_id(&self) -> &ProtocolId {
        &self.instance_id
    }

    #[must_use]
    pub fn definition_name(&self) -> &ProtocolName {
        &self.definition_name
    }

    #[must_use]
    pub fn definition_version(&self) -> &ProtocolVersion {
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
    pub fn context(&self) -> &ProtocolContext {
        &self.context
    }

    #[must_use]
    pub fn attempt(&self) -> StepAttempt {
        self.attempt
    }

    /// The transcript of interventions produced by earlier steps of this
    /// protocol.
    #[must_use]
    pub fn transcript(&self) -> &ProtocolTranscript {
        &self.transcript
    }

    /// Dynamic participant requests and responses known to the protocol.
    #[must_use]
    pub fn interventions(&self) -> &[ProtocolIntervention] {
        &self.interventions
    }

    /// The panel this session seated for the role, if it seated one.
    #[must_use]
    pub fn bound_specialty(&self) -> Option<&Specialty> {
        self.bound_specialty.as_ref()
    }

    /// The protocol role this step is executed as, if set.
    #[must_use]
    pub fn role_id(&self) -> Option<&RoleId> {
        self.role_id.as_ref()
    }

    /// Canonical semantic input for recoverable execution.
    ///
    /// Technical attempts and lease timestamps are deliberately absent: a
    /// reclaim is another claim of the same external operation. All fields
    /// that may change what the handler does remain sealed here.
    pub fn semantic_request_bytes(&self) -> Result<ExecutionRequestBytes, crate::DomainError> {
        let value = serde_json::json!({
            "schema": "made.protocol-step-request.v1",
            "instance_id": self.instance_id,
            "definition_name": self.definition_name,
            "definition_version": self.definition_version,
            "current_state": self.current_state,
            "step_id": self.step_id,
            "handler_kind": self.handler_kind,
            "handler_config": self.handler_config,
            "context": self.context,
            "transcript": self.transcript,
            "interventions": self.interventions,
            "role_id": self.role_id,
            "bound_specialty": self.bound_specialty,
        });
        let bytes =
            serde_json::to_vec(&value).map_err(|_| crate::DomainError::InvariantViolated {
                reason: "protocol step semantic request cannot be encoded",
            })?;
        ExecutionRequestBytes::new(bytes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::value_objects::Attributes;

    fn request(attempt: u32) -> ProtocolStepHandlerRequest {
        ProtocolStepHandlerRequest::new(
            ProtocolId::new("protocol").unwrap(),
            ProtocolName::new("review").unwrap(),
            ProtocolVersion::v1(),
            StateId::new("OPEN").unwrap(),
            StepId::new("work").unwrap(),
            StepHandlerKind::new("no_op").unwrap(),
            StepHandlerConfig::new(Attributes::empty()),
            ProtocolContext::empty(),
            StepAttempt::new(attempt).unwrap(),
        )
    }

    #[test]
    fn reclaim_attempts_have_the_same_semantic_request_digest() {
        let first = request(1);
        let reclaimed = request(2);

        assert_ne!(first.attempt(), reclaimed.attempt());
        assert_eq!(
            first.semantic_request_bytes().unwrap().digest(),
            reclaimed.semantic_request_bytes().unwrap().digest()
        );
    }
}
