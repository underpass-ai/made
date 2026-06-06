use choreo_core::error::DomainError;
use choreo_core::ports::CeremonyStepHandlerRequest;
use choreo_core::value_objects::{NumAgents, Rounds, Specialty, TaskDescription};
use serde_json::Value;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeliberationStepConfig {
    specialty: Specialty,
    task_description: TaskDescription,
    rounds: Rounds,
    num_agents: Option<NumAgents>,
}

impl DeliberationStepConfig {
    pub fn from_request(request: &CeremonyStepHandlerRequest) -> Result<Self, DomainError> {
        let attributes = request.handler_config().attributes();
        let task_description = TaskDescription::new(required_string(
            attributes.get("prompt"),
            "ceremony_step.config.prompt",
        )?)?;
        let specialty = Specialty::new(
            optional_string(attributes.get("specialty")).unwrap_or(request.handler_kind().as_str()),
        )?;
        let rounds = optional_u32(attributes.get("rounds"), "ceremony_step.config.rounds")?
            .map(Rounds::new)
            .transpose()?
            .unwrap_or_default();
        let num_agents = optional_u32(
            attributes.get("num_agents"),
            "ceremony_step.config.num_agents",
        )?
        .map(NumAgents::new)
        .transpose()?;

        Ok(Self {
            specialty,
            task_description,
            rounds,
            num_agents,
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
}

fn required_string(value: Option<&Value>, field: &'static str) -> Result<String, DomainError> {
    let Some(value) = value else {
        return Err(DomainError::EmptyField { field });
    };
    let Some(raw) = value.as_str() else {
        return Err(DomainError::InvalidCharacters { field });
    };
    if raw.trim().is_empty() {
        return Err(DomainError::EmptyField { field });
    }
    Ok(raw.to_owned())
}

fn optional_string(value: Option<&Value>) -> Option<&str> {
    value
        .and_then(Value::as_str)
        .filter(|raw| !raw.trim().is_empty())
}

fn optional_u32(value: Option<&Value>, field: &'static str) -> Result<Option<u32>, DomainError> {
    let Some(value) = value else { return Ok(None) };
    if value.is_null() {
        return Ok(None);
    }
    let Some(raw) = value.as_u64() else {
        return Err(DomainError::InvalidCharacters { field });
    };
    u32::try_from(raw)
        .map(Some)
        .map_err(|_| DomainError::OutOfRange {
            field,
            value: raw as f64,
            min: 0.0,
            max: f64::from(u32::MAX),
        })
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use choreo_core::value_objects::{
        Attributes, CeremonyContext, CeremonyId, CeremonyName, CeremonyVersion, StateId,
        StepAttempt, StepHandlerConfig, StepHandlerKind, StepId,
    };
    use serde_json::json;

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
