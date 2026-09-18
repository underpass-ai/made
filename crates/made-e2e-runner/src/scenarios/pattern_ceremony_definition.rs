use anyhow::{Context, Result};
use serde_yaml::{Mapping, Value};

use super::ceremony_vllm_provider_config::CeremonyVllmProviderConfig;

const CONCURRENT_REVIEW: &str =
    include_str!("../../../../tests/e2e/ceremonies/concurrent-review.yaml");
const INCIDENT_REVIEW: &str = include_str!("../../../../tests/e2e/ceremonies/incident_review.yaml");

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PatternCeremonyDefinition {
    yaml: String,
}

impl PatternCeremonyDefinition {
    pub(crate) fn concurrent_review(config: &CeremonyVllmProviderConfig) -> Result<Self> {
        Self::from_template(CONCURRENT_REVIEW, config)
    }

    pub(crate) fn incident_review(config: &CeremonyVllmProviderConfig) -> Result<Self> {
        Self::from_template(INCIDENT_REVIEW, config)
    }

    fn from_template(template: &str, provider: &CeremonyVllmProviderConfig) -> Result<Self> {
        let mut document: Value =
            serde_yaml::from_str(template).context("parse pattern ceremony fixture")?;
        let steps = mapping_value_mut(&mut document, "steps")?
            .as_sequence_mut()
            .context("pattern ceremony steps must be an array")?;
        for step in steps {
            let config = mapping_value_mut(step, "config")?
                .as_mapping_mut()
                .context("pattern ceremony step config must be an object")?;
            insert(config, "specialty", "pattern_compose_e2e");
            insert(config, "agent_kind", "vllm");
            insert(config, "provider.endpoint", provider.endpoint());
            insert(config, "provider.model", provider.model());
            insert(config, "provider.max_tokens", provider.max_tokens());
            insert(config, "provider.timeout_secs", provider.timeout_secs());
        }
        Ok(Self {
            yaml: serde_yaml::to_string(&document).context("render pattern ceremony fixture")?,
        })
    }

    pub(crate) fn as_yaml(&self) -> &str {
        &self.yaml
    }
}

fn mapping_value_mut<'a>(value: &'a mut Value, key: &str) -> Result<&'a mut Value> {
    value
        .as_mapping_mut()
        .context("pattern ceremony document entry must be an object")?
        .get_mut(Value::String(key.to_owned()))
        .with_context(|| format!("pattern ceremony document is missing `{key}`"))
}

fn insert(mapping: &mut Mapping, key: &str, value: impl Into<Value>) {
    mapping.insert(Value::String(key.to_owned()), value.into());
}

#[cfg(test)]
mod tests {
    use made_adapters::ceremony::CeremonyParticipantPlanAdapter;
    use made_adapters::yaml::CeremonyDefinitionYaml;
    use serde_json::Value as JsonValue;

    use super::*;

    fn provider() -> CeremonyVllmProviderConfig {
        CeremonyVllmProviderConfig::new("http://stub-llm:8000", "stub-pattern-v1", 256, 30).unwrap()
    }

    #[test]
    fn both_pattern_fixtures_mount_the_stub_provider_on_every_step() {
        for (template, rendered) in [
            (
                CONCURRENT_REVIEW,
                PatternCeremonyDefinition::concurrent_review(&provider()).unwrap(),
            ),
            (
                INCIDENT_REVIEW,
                PatternCeremonyDefinition::incident_review(&provider()).unwrap(),
            ),
        ] {
            let definition = CeremonyDefinitionYaml::parse_str(rendered.as_yaml()).unwrap();
            let participants =
                CeremonyParticipantPlanAdapter::from_definition(&definition).unwrap();
            assert_eq!(participants.participants().len(), 1);
            assert!(definition.steps().values().all(|step| {
                let attributes = step.handler_config().attributes();
                attributes.get("agent_kind") == Some(&JsonValue::from("vllm"))
                    && attributes.get("provider.endpoint")
                        == Some(&JsonValue::from("http://stub-llm:8000"))
            }));
            assert_eq!(
                without_runtime_wiring(rendered.as_yaml()),
                serde_yaml::from_str::<Value>(template).unwrap()
            );
        }
    }

    #[test]
    fn incident_projection_survives_runtime_provider_injection() {
        let rendered = PatternCeremonyDefinition::incident_review(&provider()).unwrap();
        let document: Value = serde_yaml::from_str(rendered.as_yaml()).unwrap();
        let steps = document["steps"].as_sequence().unwrap();
        let check = steps
            .iter()
            .find(|step| step["id"].as_str() == Some("writeup_check_1"))
            .unwrap();
        assert_eq!(
            check["config"]["project_winner_fields"],
            Value::Sequence(vec![Value::String("approved".to_owned())])
        );
    }

    #[test]
    fn malformed_fixture_is_rejected() {
        let error = PatternCeremonyDefinition::from_template("name: broken", &provider())
            .unwrap_err()
            .to_string();
        assert!(error.contains("missing `steps`"), "{error}");
    }

    fn without_runtime_wiring(yaml: &str) -> Value {
        let mut document: Value = serde_yaml::from_str(yaml).unwrap();
        for step in document["steps"].as_sequence_mut().unwrap() {
            let config = step["config"].as_mapping_mut().unwrap();
            for key in [
                "specialty",
                "agent_kind",
                "provider.endpoint",
                "provider.model",
                "provider.max_tokens",
                "provider.timeout_secs",
            ] {
                config.remove(Value::String(key.to_owned()));
            }
        }
        document
    }
}
