//! [`Task`] entity — a unit of work submitted for deliberation.

use serde::{Deserialize, Serialize};

use crate::entities::ExternalContextBundle;
use crate::value_objects::{
    Attributes, DurationMs, NumAgents, OutputContract, Rounds, Rubric, Specialty, TaskDescription,
    TaskId,
};

/// The per-task configuration that shapes a deliberation.
///
/// Kept as a nested value object so a `Task` stays a small entity with
/// clear ownership of its configuration.
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

/// A task submitted to the Choreographer.
///
/// `description` is the free-form prompt that agents consume;
/// `attributes` carries arbitrary, opaque domain data that the
/// Choreographer does not interpret. The `specialty` selects which
/// council deliberates.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Task {
    id: TaskId,
    specialty: Specialty,
    description: TaskDescription,
    constraints: TaskConstraints,
    attributes: Attributes,
    external_context: Option<ExternalContextBundle>,
}

impl Task {
    #[must_use]
    pub fn new(
        id: TaskId,
        specialty: Specialty,
        description: TaskDescription,
        constraints: TaskConstraints,
        attributes: Attributes,
    ) -> Self {
        Self::new_with_context(id, specialty, description, constraints, attributes, None)
    }

    #[must_use]
    pub fn new_with_context(
        id: TaskId,
        specialty: Specialty,
        description: TaskDescription,
        constraints: TaskConstraints,
        attributes: Attributes,
        external_context: Option<ExternalContextBundle>,
    ) -> Self {
        Self {
            id,
            specialty,
            description,
            constraints,
            attributes,
            external_context,
        }
    }

    #[must_use]
    pub fn id(&self) -> &TaskId {
        &self.id
    }
    #[must_use]
    pub fn specialty(&self) -> &Specialty {
        &self.specialty
    }
    #[must_use]
    pub fn description(&self) -> &TaskDescription {
        &self.description
    }
    #[must_use]
    pub fn constraints(&self) -> &TaskConstraints {
        &self.constraints
    }
    #[must_use]
    pub fn attributes(&self) -> &Attributes {
        &self.attributes
    }

    #[must_use]
    pub fn external_context(&self) -> Option<&ExternalContextBundle> {
        self.external_context.as_ref()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make() -> Task {
        Task::new(
            TaskId::new("t1").unwrap(),
            Specialty::new("triage").unwrap(),
            TaskDescription::new("investigate alert").unwrap(),
            TaskConstraints::default(),
            Attributes::empty(),
        )
    }

    #[test]
    fn default_constraints_are_sane() {
        let c = TaskConstraints::default();
        assert_eq!(c.rounds(), Rounds::default());
        assert!(c.num_agents().is_none());
        assert!(c.deadline().is_none());
        assert!(c.rubric().is_empty());
    }

    #[test]
    fn task_accessors_return_fields() {
        let t = make();
        assert_eq!(t.id().as_str(), "t1");
        assert_eq!(t.specialty().as_str(), "triage");
        assert_eq!(t.description().as_str(), "investigate alert");
        assert!(t.attributes().is_empty());
        assert!(t.external_context().is_none());
    }

    #[test]
    fn constraints_accepts_optional_bounds() {
        let c = TaskConstraints::new(
            Rubric::empty(),
            Rounds::new(3).unwrap(),
            Some(NumAgents::new(4).unwrap()),
            Some(DurationMs::from_millis(1500)),
        );
        assert_eq!(c.rounds().get(), 3);
        assert_eq!(c.num_agents().unwrap().get(), 4);
        assert_eq!(c.deadline().unwrap().get(), 1500);
    }

    #[test]
    fn constraints_can_enable_structured_output_contract() {
        use crate::value_objects::{OutputFieldRule, OutputFormat};
        use std::collections::BTreeMap;

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
    fn empty_json_object_deserializes_to_default_constraints() {
        let c: TaskConstraints = serde_json::from_str("{}").unwrap();
        assert_eq!(c, TaskConstraints::default());
    }

    #[test]
    fn task_has_no_hardcoded_domain_vocabulary() {
        // Regression: neutral specialty + neutral description must
        // form a valid Task.
        let _ = Task::new(
            TaskId::new("t-clinical-01").unwrap(),
            Specialty::new("clinical-intake").unwrap(),
            TaskDescription::new("classify protocol deviation").unwrap(),
            TaskConstraints::default(),
            Attributes::empty(),
        );
    }
}
