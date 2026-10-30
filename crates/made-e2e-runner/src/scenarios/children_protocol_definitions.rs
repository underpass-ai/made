const CHILD: &str = include_str!("../../../../tests/e2e/protocols/children-review.yaml");
const PARENT: &str = include_str!("../../../../tests/e2e/protocols/children-parent.yaml");

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ChildrenProtocolDefinitions;

impl ChildrenProtocolDefinitions {
    pub(crate) const fn child() -> &'static str {
        CHILD
    }

    pub(crate) const fn parent() -> &'static str {
        PARENT
    }
}

#[cfg(test)]
mod tests {
    use made_adapters::yaml::ProtocolDefinitionYaml;
    use serde_json::Value as JsonValue;

    use super::*;

    #[test]
    fn fixtures_parse_with_two_children_and_stub_only_wiring() {
        let child =
            ProtocolDefinitionYaml::parse_str(ChildrenProtocolDefinitions::child()).unwrap();
        let parent =
            ProtocolDefinitionYaml::parse_str(ChildrenProtocolDefinitions::parent()).unwrap();

        let review = child
            .step(&made_core::value_objects::StepId::new("review").unwrap())
            .unwrap();
        assert_eq!(
            review.handler_config().attributes().get("agent_kind"),
            Some(&JsonValue::from("noop"))
        );
        assert_eq!(
            review.handler_config().attributes().get("specialty"),
            Some(&JsonValue::from("triage"))
        );
        let spawn = parent
            .step(&made_core::value_objects::StepId::new("spawn_reviews").unwrap())
            .unwrap()
            .spawn()
            .unwrap();
        assert_eq!(spawn.children().len(), 2);
    }
}
