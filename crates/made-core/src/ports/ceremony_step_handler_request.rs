//! Request passed to ceremony step handler adapters.

use crate::entities::CeremonyIntervention;
use crate::value_objects::{
    CeremonyContext, CeremonyId, CeremonyName, CeremonyTranscript, CeremonyVersion,
    ExecutionRequestBytes, RoleId, Specialty, StateId, StepAttempt, StepHandlerConfig,
    StepHandlerKind, StepId,
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
    transcript: CeremonyTranscript,
    interventions: Vec<CeremonyIntervention>,
    role_id: Option<RoleId>,
    /// The specialty this session seated for the role, when it seated
    /// one. Absent means the step's own configuration decides, which
    /// is how every session behaved before seating existed.
    bound_specialty: Option<Specialty>,
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
            transcript: CeremonyTranscript::empty(),
            interventions: Vec::new(),
            role_id: None,
            bound_specialty: None,
        }
    }

    /// Attach the prior-step transcript the engine accumulated for this
    /// ceremony, so the handler can deliberate with earlier interventions
    /// in view. Defaults to an empty transcript when not set.
    #[must_use]
    pub fn with_transcript(mut self, transcript: CeremonyTranscript) -> Self {
        self.transcript = transcript;
        self
    }

    /// Attach the participant-created agenda accumulated by the running
    /// ceremony. Declaration order is retained so handlers see the same
    /// conversational sequence as the participants.
    #[must_use]
    pub fn with_interventions(mut self, interventions: Vec<CeremonyIntervention>) -> Self {
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

    /// Attach the ceremony role this step is executed as, so the handler
    /// can frame the agent's persona. Defaults to none when not set.
    #[must_use]
    pub fn with_role(mut self, role_id: RoleId) -> Self {
        self.role_id = Some(role_id);
        self
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

    /// The transcript of interventions produced by earlier steps of this
    /// ceremony.
    #[must_use]
    pub fn transcript(&self) -> &CeremonyTranscript {
        &self.transcript
    }

    /// Dynamic participant requests and responses known to the ceremony.
    #[must_use]
    pub fn interventions(&self) -> &[CeremonyIntervention] {
        &self.interventions
    }

    /// The panel this session seated for the role, if it seated one.
    #[must_use]
    pub fn bound_specialty(&self) -> Option<&Specialty> {
        self.bound_specialty.as_ref()
    }

    /// The ceremony role this step is executed as, if set.
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
            "schema": "made.ceremony-step-request.v1",
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
                reason: "ceremony step semantic request cannot be encoded",
            })?;
        ExecutionRequestBytes::new(bytes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::value_objects::Attributes;

    fn request(attempt: u32) -> CeremonyStepHandlerRequest {
        CeremonyStepHandlerRequest::new(
            CeremonyId::new("ceremony").unwrap(),
            CeremonyName::new("review").unwrap(),
            CeremonyVersion::v1(),
            StateId::new("OPEN").unwrap(),
            StepId::new("work").unwrap(),
            StepHandlerKind::new("no_op").unwrap(),
            StepHandlerConfig::new(Attributes::empty()),
            CeremonyContext::empty(),
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
