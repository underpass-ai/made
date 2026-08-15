use std::collections::BTreeMap;

use made_app::usecases::{CeremonyParticipantDescriptor, PrepareCeremonyParticipantsInput};
use made_core::entities::CeremonyDefinition;
use made_core::error::DomainError;
use made_core::value_objects::{AgentId, Attributes, CeremonyStep};
use serde_json::{json, Value};

use super::CeremonyStepConfig;

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
    let config = CeremonyStepConfig::new(step.handler_config().attributes(), step.handler_kind());
    let specialty = config.specialty()?;
    let kind = config.agent_kind()?;
    let labels = config.participant_labels()?;
    let count = if labels.is_empty() {
        config.num_agents()?.map_or(1, |num| num.get() as usize)
    } else {
        labels.len()
    };

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
    use made_core::value_objects::CeremonyName;

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
