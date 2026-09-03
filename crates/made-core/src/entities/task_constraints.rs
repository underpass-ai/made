use serde::{Deserialize, Serialize};

use crate::value_objects::{DurationMs, NumAgents, OutputContract, Rounds, Rubric};

/// Domain configuration that shapes a task's deliberation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct TaskConstraints {
    rubric: Rubric,
    rounds: Rounds,
    num_agents: Option<NumAgents>,
    deadline: Option<DurationMs>,
    output_contract: Option<OutputContract>,
}

impl TaskConstraints {
    #[must_use]
    pub fn new(
        rubric: Rubric,
        rounds: Rounds,
        num_agents: Option<NumAgents>,
        deadline: Option<DurationMs>,
    ) -> Self {
        Self {
            rubric,
            rounds,
            num_agents,
            deadline,
            output_contract: None,
        }
    }

    #[must_use]
    pub fn rubric(&self) -> &Rubric {
        &self.rubric
    }

    #[must_use]
    pub fn rounds(&self) -> Rounds {
        self.rounds
    }

    #[must_use]
    pub fn num_agents(&self) -> Option<NumAgents> {
        self.num_agents
    }

    #[must_use]
    pub fn deadline(&self) -> Option<DurationMs> {
        self.deadline
    }

    #[must_use]
    pub fn output_contract(&self) -> Option<&OutputContract> {
        self.output_contract.as_ref()
    }

    #[must_use]
    pub fn with_output_contract(mut self, output_contract: OutputContract) -> Self {
        self.output_contract = Some(output_contract);
        self
    }
}

impl Default for TaskConstraints {
    fn default() -> Self {
        Self {
            rubric: Rubric::empty(),
            rounds: Rounds::default(),
            num_agents: None,
            deadline: None,
            output_contract: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use super::*;
    use crate::value_objects::{OutputFieldRule, OutputFormat};

    #[test]
    fn defaults_are_sane() {
        let constraints = TaskConstraints::default();
        assert_eq!(constraints.rounds(), Rounds::default());
        assert!(constraints.num_agents().is_none());
        assert!(constraints.deadline().is_none());
        assert!(constraints.rubric().is_empty());
    }

    #[test]
    fn accepts_optional_bounds() {
        let constraints = TaskConstraints::new(
            Rubric::empty(),
            Rounds::new(3).unwrap(),
            Some(NumAgents::new(4).unwrap()),
            Some(DurationMs::from_millis(1500)),
        );
        assert_eq!(constraints.rounds().get(), 3);
        assert_eq!(constraints.num_agents().unwrap().get(), 4);
        assert_eq!(constraints.deadline().unwrap().get(), 1500);
    }

    #[test]
    fn supports_structured_output_contract() {
        let contract = OutputContract::new(
            "decision-contract",
            OutputFormat::JsonObject,
            BTreeMap::from([(
                "decision".to_owned(),
                OutputFieldRule::new(true, ["emit_event", "escalate"]).unwrap(),
            )]),
        )
        .unwrap();

        let constraints = TaskConstraints::default().with_output_contract(contract.clone());
        assert_eq!(constraints.output_contract(), Some(&contract));
    }

    #[test]
    fn empty_json_object_deserializes_to_defaults() {
        let constraints: TaskConstraints = serde_json::from_str("{}").unwrap();
        assert_eq!(constraints, TaskConstraints::default());
    }
}
