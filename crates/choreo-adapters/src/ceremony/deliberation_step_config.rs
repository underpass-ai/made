use choreo_core::error::DomainError;
use choreo_core::ports::CeremonyStepHandlerRequest;
use choreo_core::value_objects::{NumAgents, Rounds, Specialty, TaskDescription};

use super::CeremonyStepConfig;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeliberationStepConfig {
    specialty: Specialty,
    task_description: TaskDescription,
    rounds: Rounds,
    num_agents: Option<NumAgents>,
    see_prior: bool,
}

impl DeliberationStepConfig {
    pub fn from_request(request: &CeremonyStepHandlerRequest) -> Result<Self, DomainError> {
        let config = CeremonyStepConfig::new(
            request.handler_config().attributes(),
            request.handler_kind(),
        );

        Ok(Self {
            task_description: config.prompt()?,
            specialty: config.specialty()?,
            rounds: config.rounds()?,
            num_agents: config.num_agents()?,
            see_prior: config.see_prior_steps()?,
        })
    }

    #[must_use]
    pub fn specialty(&self) -> &Specialty {
        &self.specialty
    }

    #[must_use]
    pub fn task_description(&self) -> &TaskDescription {
        &self.task_description
    }

    #[must_use]
    pub fn rounds(&self) -> Rounds {
        self.rounds
    }

    #[must_use]
    pub fn num_agents(&self) -> Option<NumAgents> {
        self.num_agents
    }

    /// Whether the step deliberates with the prior ceremony transcript in
    /// view (defaults to `true`).
    #[must_use]
    pub fn see_prior(&self) -> bool {
        self.see_prior
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use choreo_core::value_objects::{
        Attributes, CeremonyContext, CeremonyId, CeremonyName, CeremonyVersion, StateId,
        StepAttempt, StepHandlerConfig, StepHandlerKind, StepId,
    };
    use serde_json::{json, Value};

    use super::*;

    fn request(config: BTreeMap<String, Value>) -> CeremonyStepHandlerRequest {
        CeremonyStepHandlerRequest::new(
            CeremonyId::new("ceremony-1").unwrap(),
            CeremonyName::new("editorial").unwrap(),
            CeremonyVersion::v1(),
            StateId::new("OPENING").unwrap(),
            StepId::new("open_room").unwrap(),
            StepHandlerKind::new("facilitation_prompt").unwrap(),
            StepHandlerConfig::new(Attributes::new(config).unwrap()),
            CeremonyContext::empty(),
            StepAttempt::FIRST,
        )
    }

    #[test]
    fn defaults_specialty_to_handler_kind() {
        let config = DeliberationStepConfig::from_request(&request(BTreeMap::from([(
            "prompt".to_owned(),
            json!("Open the meeting"),
        )])))
        .unwrap();

        assert_eq!(config.specialty().as_str(), "facilitation_prompt");
        assert_eq!(config.task_description().as_str(), "Open the meeting");
        assert_eq!(config.rounds().get(), 1);
        assert!(config.num_agents().is_none());
    }

    #[test]
    fn accepts_explicit_specialty_and_bounds() {
        let config = DeliberationStepConfig::from_request(&request(BTreeMap::from([
            ("prompt".to_owned(), json!("Open the meeting")),
            ("specialty".to_owned(), json!("facilitator")),
            ("rounds".to_owned(), json!(0)),
            ("num_agents".to_owned(), json!(1)),
        ])))
        .unwrap();

        assert_eq!(config.specialty().as_str(), "facilitator");
        assert_eq!(config.rounds().get(), 0);
        assert_eq!(config.num_agents().unwrap().get(), 1);
    }

    #[test]
    fn rejects_missing_prompt() {
        let err = DeliberationStepConfig::from_request(&request(BTreeMap::new())).unwrap_err();

        assert!(matches!(
            err,
            DomainError::EmptyField {
                field: "ceremony_step.config.prompt"
            }
        ));
    }
}
