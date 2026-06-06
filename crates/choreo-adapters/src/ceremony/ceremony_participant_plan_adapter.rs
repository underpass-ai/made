use std::collections::BTreeMap;

use choreo_app::usecases::{CeremonyParticipantDescriptor, PrepareCeremonyParticipantsInput};
use choreo_core::entities::CeremonyDefinition;
use choreo_core::error::DomainError;
use choreo_core::value_objects::{
    AgentId, AgentKind, Attributes, CeremonyStep, NumAgents, Specialty,
};
use serde_json::{json, Value};

const DEFAULT_AGENT_KIND: &str = "noop";

#[derive(Debug, Clone, Copy)]
pub struct CeremonyParticipantPlanAdapter;

impl CeremonyParticipantPlanAdapter {
    pub fn from_definition(
        definition: &CeremonyDefinition,
    ) -> Result<PrepareCeremonyParticipantsInput, DomainError> {
        let mut participants = BTreeMap::new();
        for step in definition.steps().values() {
            for participant in participants_from_step(definition, step)? {
                participants
                    .entry(participant.id().clone())
                    .or_insert(participant);
            }
        }
        Ok(PrepareCeremonyParticipantsInput::new(
            participants.into_values().collect(),
        ))
    }
}

fn participants_from_step(
    definition: &CeremonyDefinition,
    step: &CeremonyStep,
) -> Result<Vec<CeremonyParticipantDescriptor>, DomainError> {
    let config = step.handler_config().attributes();
    let specialty = Specialty::new(
        optional_string(config.get("specialty")).unwrap_or(step.handler_kind().as_str()),
    )?;
    let kind = AgentKind::new(
        optional_string(config.get("agent_kind"))
            .or_else(|| optional_string(config.get("agent.kind")))
            .unwrap_or(DEFAULT_AGENT_KIND),
    )?;
    let labels = participant_labels(config.get("participants"))?;
    let count = participant_count(config.get("num_agents"), labels.len())?;

    (0..count)
        .map(|index| {
            let label = labels.get(index).map(String::as_str);
            Ok(CeremonyParticipantDescriptor::new(
                AgentId::new(format!("agent-{}-{index}", specialty.as_str()))?,
                specialty.clone(),
                kind.clone(),
                Attributes::new(participant_attributes(definition, step, index, label))?,
            ))
        })
        .collect()
}

fn optional_string(value: Option<&Value>) -> Option<&str> {
    value
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|raw| !raw.is_empty())
}

fn participant_labels(value: Option<&Value>) -> Result<Vec<String>, DomainError> {
    let Some(value) = value else {
        return Ok(Vec::new());
    };
    if value.is_null() {
        return Ok(Vec::new());
    }
    let Some(items) = value.as_array() else {
        return Err(DomainError::InvalidCharacters {
            field: "ceremony_step.config.participants",
        });
    };
    items
        .iter()
        .map(|item| {
            item.as_str()
                .map(str::trim)
                .filter(|label| !label.is_empty())
                .map(str::to_owned)
                .ok_or(DomainError::InvalidCharacters {
                    field: "ceremony_step.config.participants",
                })
        })
        .collect()
}

fn participant_count(value: Option<&Value>, label_count: usize) -> Result<usize, DomainError> {
    if label_count > 0 {
        return Ok(label_count);
    }
    let Some(value) = value else { return Ok(1) };
    if value.is_null() {
        return Ok(1);
    }
    let Some(raw) = value.as_u64() else {
        return Err(DomainError::InvalidCharacters {
            field: "ceremony_step.config.num_agents",
        });
    };
    let count = u32::try_from(raw).map_err(|_| DomainError::OutOfRange {
        field: "ceremony_step.config.num_agents",
        value: raw as f64,
        min: 1.0,
        max: f64::from(u32::MAX),
    })?;
    usize::try_from(NumAgents::new(count)?.get()).map_err(|_| DomainError::OutOfRange {
        field: "ceremony_step.config.num_agents",
        value: f64::from(count),
        min: 1.0,
        max: f64::from(u32::MAX),
    })
}

fn participant_attributes(
    definition: &CeremonyDefinition,
    step: &CeremonyStep,
    index: usize,
    label: Option<&str>,
) -> BTreeMap<String, Value> {
    let mut attributes = step.handler_config().attributes().as_map().clone();
    attributes.insert(
        "ceremony.definition_name".to_owned(),
        json!(definition.name().as_str()),
    );
    attributes.insert(
        "ceremony.definition_version".to_owned(),
        json!(definition.version().as_str()),
    );
    attributes.insert("ceremony.step_id".to_owned(), json!(step.id().as_str()));
    attributes.insert(
        "ceremony.handler_kind".to_owned(),
        json!(step.handler_kind().as_str()),
    );
    attributes.insert("ceremony.participant.index".to_owned(), json!(index));
    if let Some(label) = label {
        attributes.insert("ceremony.participant.label".to_owned(), json!(label));
    }
    attributes
}

#[cfg(test)]
mod tests {
    use choreo_core::value_objects::CeremonyName;

    use crate::yaml::CeremonyDefinitionYaml;

    use super::*;

    const CEREMONY: &str = r#"
version: "1.0"
name: "participant_ceremony"
states:
  - id: STARTED
    initial: true
  - id: DONE
    terminal: true
transitions:
  - from: STARTED
    to: DONE
    trigger: finish
    guards:
      - work_completed
steps:
  - id: work
    state: STARTED
    handler: multiagent_round
    config:
      participants:
        - facilitator
        - critic
      prompt: "Discuss the brief"
guards:
  work_completed:
    type: automated
    check: "step_status:work:COMPLETED"
roles:
  - id: RUNNER
    allowed_actions:
      - work
      - finish
"#;

    #[test]
    fn builds_participants_from_step_participant_labels() {
        let definition = CeremonyDefinitionYaml::parse_str(CEREMONY).unwrap();
        let input = CeremonyParticipantPlanAdapter::from_definition(&definition).unwrap();

        assert_eq!(
            definition.name(),
            &CeremonyName::new("participant_ceremony").unwrap()
        );
        assert_eq!(input.participants().len(), 2);
        assert_eq!(
            input.participants()[0].id().as_str(),
            "agent-multiagent_round-0"
        );
        assert_eq!(
            input.participants()[0].specialty().as_str(),
            "multiagent_round"
        );
        assert_eq!(input.participants()[0].kind().as_str(), "noop");
        assert_eq!(
            input.participants()[0]
                .attributes()
                .get("ceremony.participant.label")
                .and_then(Value::as_str),
            Some("facilitator")
        );
        assert_eq!(
            input.participants()[1]
                .attributes()
                .get("ceremony.participant.label")
                .and_then(Value::as_str),
            Some("critic")
        );
    }

    #[test]
    fn explicit_specialty_kind_and_num_agents_override_defaults() {
        let yaml = CEREMONY.replace(
            "      participants:\n        - facilitator\n        - critic",
            "      specialty: editorial\n      agent_kind: vllm\n      num_agents: 2",
        );
        let definition = CeremonyDefinitionYaml::parse_str(&yaml).unwrap();
        let input = CeremonyParticipantPlanAdapter::from_definition(&definition).unwrap();

        assert_eq!(input.participants().len(), 2);
        assert!(input
            .participants()
            .iter()
            .all(|participant| participant.specialty().as_str() == "editorial"));
        assert!(input
            .participants()
            .iter()
            .all(|participant| participant.kind().as_str() == "vllm"));
    }

    #[test]
    fn invalid_participants_shape_is_rejected() {
        let yaml = CEREMONY.replace(
            "      participants:\n        - facilitator\n        - critic",
            "      participants: facilitator",
        );
        let definition = CeremonyDefinitionYaml::parse_str(&yaml).unwrap();
        let err = CeremonyParticipantPlanAdapter::from_definition(&definition).unwrap_err();

        assert!(matches!(
            err,
            DomainError::InvalidCharacters {
                field: "ceremony_step.config.participants"
            }
        ));
    }
}
