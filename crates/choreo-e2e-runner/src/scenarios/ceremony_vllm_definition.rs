use anyhow::{Context, Result};

use super::ceremony_vllm_provider_config::CeremonyVllmProviderConfig;

const TEMPLATE: &str =
    include_str!("../../../../tests/e2e/ceremonies/editorial-planning-meeting-vllm.yaml");
const ENDPOINT_PLACEHOLDER: &str = "__CHOREO_VLLM_ENDPOINT__";
const MODEL_PLACEHOLDER: &str = "__CHOREO_VLLM_MODEL__";
const MAX_TOKENS_PLACEHOLDER: &str = "__CHOREO_VLLM_MAX_TOKENS__";
const TIMEOUT_SECS_PLACEHOLDER: &str = "__CHOREO_VLLM_TIMEOUT_SECS__";

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CeremonyVllmDefinition {
    yaml: String,
}

impl CeremonyVllmDefinition {
    pub(crate) fn from_provider_config(config: &CeremonyVllmProviderConfig) -> Result<Self> {
        let yaml = TEMPLATE
            .replace(ENDPOINT_PLACEHOLDER, &yaml_string(config.endpoint())?)
            .replace(MODEL_PLACEHOLDER, &yaml_string(config.model())?)
            .replace(MAX_TOKENS_PLACEHOLDER, &config.max_tokens().to_string())
            .replace(TIMEOUT_SECS_PLACEHOLDER, &config.timeout_secs().to_string());
        Ok(Self { yaml })
    }

    pub(crate) fn as_yaml(&self) -> &str {
        &self.yaml
    }
}

fn yaml_string(value: &str) -> Result<String> {
    serde_json::to_string(value).context("serialize vLLM provider value as YAML scalar")
}

#[cfg(test)]
mod tests {
    use choreo_adapters::ceremony::CeremonyParticipantPlanAdapter;
    use choreo_adapters::yaml::CeremonyDefinitionYaml;

    use super::*;

    #[test]
    fn renders_provider_values_into_yaml_template() {
        let config =
            CeremonyVllmProviderConfig::new("http://stub-llm:8000", "stub-report-vllm-v1", 256, 45)
                .unwrap();
        let definition = CeremonyVllmDefinition::from_provider_config(&config).unwrap();
        let yaml = definition.as_yaml();

        assert!(!yaml.contains("__CHOREO_VLLM_"));
        assert!(yaml.contains(r#"provider.endpoint: "http://stub-llm:8000""#));
        assert!(yaml.contains(r#"provider.model: "stub-report-vllm-v1""#));
        assert!(yaml.contains("provider.max_tokens: 256"));
        assert!(yaml.contains("provider.timeout_secs: 45"));
        assert!(yaml.contains("agent_kind: vllm"));
        assert!(yaml.contains("num_agents: 4"));
        assert!(yaml.contains("rounds: 0"));
    }

    #[test]
    fn rendered_yaml_mounts_four_vllm_participants() {
        let config =
            CeremonyVllmProviderConfig::new("http://stub-llm:8000", "stub-report-vllm-v1", 256, 45)
                .unwrap();
        let definition_yaml = CeremonyVllmDefinition::from_provider_config(&config).unwrap();
        let definition = CeremonyDefinitionYaml::parse_str(definition_yaml.as_yaml()).unwrap();
        let participants = CeremonyParticipantPlanAdapter::from_definition(&definition).unwrap();

        assert_eq!(definition.steps().len(), 4);
        assert_eq!(participants.participants().len(), 4);
        assert!(participants
            .participants()
            .iter()
            .all(|participant| participant.kind().as_str() == "vllm"));
        assert!(participants
            .participants()
            .iter()
            .all(|participant| participant.specialty().as_str() == "editorial_meeting_vllm"));
    }
}
