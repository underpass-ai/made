use anyhow::{Context, Result};
use serde_yaml::{Mapping, Value};

use super::ceremony_vllm_provider_config::CeremonyVllmProviderConfig;

const CHILD: &str = include_str!("../../../../tests/e2e/ceremonies/children-review.yaml");
const PARENT: &str = include_str!("../../../../tests/e2e/ceremonies/children-parent.yaml");

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ChildrenCeremonyDefinitions {
    child: String,
}

impl ChildrenCeremonyDefinitions {
    pub(crate) fn new(provider: &CeremonyVllmProviderConfig) -> Result<Self> {
        let mut document: Value =
            serde_yaml::from_str(CHILD).context("parse child ceremony fixture")?;
        let steps = document["steps"]
            .as_sequence_mut()
            .context("child ceremony steps must be an array")?;
        for step in steps {
            let config = step["config"]
                .as_mapping_mut()
                .context("child ceremony step config must be an object")?;
            insert(config, "specialty", "children_compose_e2e");
            insert(config, "agent_kind", "vllm");
            insert(config, "provider.endpoint", provider.endpoint());
            insert(config, "provider.model", provider.model());
            insert(config, "provider.max_tokens", provider.max_tokens());
            insert(config, "provider.timeout_secs", provider.timeout_secs());
        }
        Ok(Self {
            child: serde_yaml::to_string(&document).context("render child ceremony fixture")?,
        })
    }

    pub(crate) fn child(&self) -> &str {
        &self.child
    }

    pub(crate) const fn parent(&self) -> &'static str {
        PARENT
    }
}

fn insert(mapping: &mut Mapping, key: &str, value: impl Into<Value>) {
    mapping.insert(Value::String(key.to_owned()), value.into());
}

#[cfg(test)]
mod tests {
    use made_adapters::yaml::CeremonyDefinitionYaml;
    use serde_json::Value as JsonValue;

    use super::*;

    fn provider() -> CeremonyVllmProviderConfig {
        CeremonyVllmProviderConfig::new("http://stub-llm:8000", "stub-children-v1", 256, 30)
            .unwrap()
    }

    #[test]
    fn fixtures_parse_with_two_children_and_stub_only_wiring() {
        let rendered = ChildrenCeremonyDefinitions::new(&provider()).unwrap();
        let child = CeremonyDefinitionYaml::parse_str(rendered.child()).unwrap();
        let parent = CeremonyDefinitionYaml::parse_str(rendered.parent()).unwrap();

        let review = child
            .step(&made_core::value_objects::StepId::new("review").unwrap())
            .unwrap();
        assert_eq!(
            review.handler_config().attributes().get("agent_kind"),
            Some(&JsonValue::from("vllm"))
        );
        let spawn = parent
            .step(&made_core::value_objects::StepId::new("spawn_reviews").unwrap())
            .unwrap()
            .spawn()
            .unwrap();
        assert_eq!(spawn.children().len(), 2);
    }
}
