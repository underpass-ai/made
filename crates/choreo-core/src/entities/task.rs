//! [`Task`] entity — a unit of work submitted for deliberation.

use serde::{Deserialize, Serialize};

use crate::entities::ExternalContextBundle;
use crate::error::DomainError;
use crate::events::EventEnvelope;
use crate::value_objects::{
    Attributes, DurationMs, EventId, NumAgents, OutputContract, Rounds, Rubric, Specialty,
    TaskDescription, TaskId,
};

const MAX_METADATA_TEXT_LEN: usize = 128;

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
    #[serde(default)]
    metadata: TaskMetadata,
}

/// First-class metadata that travels with a task through deliberation,
/// execution, and lifecycle events.
///
/// The fields are deliberately integration-neutral. Application-owned
/// identifiers stay in `Task::attributes` or `ExternalContextBundle`
/// metadata; the core only understands causality and Choreographer
/// contract/execution hints.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct TaskMetadata {
    source_event_id: Option<EventId>,
    causation_id: Option<EventId>,
    correlation_id: Option<EventId>,
    council_contract_id: Option<String>,
    output_contract_id: Option<String>,
    execution_profile: Attributes,
}

impl TaskMetadata {
    pub fn new(
        source_event_id: Option<EventId>,
        causation_id: Option<EventId>,
        correlation_id: Option<EventId>,
        council_contract_id: Option<String>,
        output_contract_id: Option<String>,
        execution_profile: Attributes,
    ) -> Result<Self, DomainError> {
        Ok(Self {
            source_event_id,
            causation_id,
            correlation_id,
            council_contract_id: validate_optional_text(
                council_contract_id,
                "task_metadata.council_contract_id",
            )?,
            output_contract_id: validate_optional_text(
                output_contract_id,
                "task_metadata.output_contract_id",
            )?,
            execution_profile,
        })
    }

    #[must_use]
    pub fn from_trigger_envelope(envelope: &EventEnvelope) -> Self {
        Self::default().with_trigger_envelope(envelope)
    }

    #[must_use]
    pub fn with_trigger_envelope(mut self, envelope: &EventEnvelope) -> Self {
        if self.source_event_id.is_none() {
            self.source_event_id = Some(envelope.event_id().clone());
        }
        if self.causation_id.is_none() {
            self.causation_id = envelope
                .causation_id()
                .cloned()
                .or_else(|| Some(envelope.event_id().clone()));
        }
        if self.correlation_id.is_none() {
            self.correlation_id = envelope
                .correlation_id()
                .cloned()
                .or_else(|| Some(envelope.event_id().clone()));
        }
        self
    }

    #[must_use]
    pub fn source_event_id(&self) -> Option<&EventId> {
        self.source_event_id.as_ref()
    }

    #[must_use]
    pub fn causation_id(&self) -> Option<&EventId> {
        self.causation_id.as_ref()
    }

    #[must_use]
    pub fn correlation_id(&self) -> Option<&EventId> {
        self.correlation_id.as_ref()
    }

    #[must_use]
    pub fn council_contract_id(&self) -> Option<&str> {
        self.council_contract_id.as_deref()
    }

    #[must_use]
    pub fn output_contract_id(&self) -> Option<&str> {
        self.output_contract_id.as_deref()
    }

    #[must_use]
    pub fn execution_profile(&self) -> &Attributes {
        &self.execution_profile
    }
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
        Self::new_with_metadata(
            id,
            specialty,
            description,
            constraints,
            attributes,
            external_context,
            TaskMetadata::default(),
        )
    }

    #[must_use]
    pub fn new_with_metadata(
        id: TaskId,
        specialty: Specialty,
        description: TaskDescription,
        constraints: TaskConstraints,
        attributes: Attributes,
        external_context: Option<ExternalContextBundle>,
        metadata: TaskMetadata,
    ) -> Self {
        Self {
            id,
            specialty,
            description,
            constraints,
            attributes,
            external_context,
            metadata,
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

    #[must_use]
    pub fn metadata(&self) -> &TaskMetadata {
        &self.metadata
    }
}

fn validate_optional_text(
    value: Option<String>,
    field: &'static str,
) -> Result<Option<String>, DomainError> {
    let Some(value) = value else {
        return Ok(None);
    };
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Ok(None);
    }
    if trimmed.len() > MAX_METADATA_TEXT_LEN {
        return Err(DomainError::FieldTooLong {
            field,
            actual: trimmed.len(),
            max: MAX_METADATA_TEXT_LEN,
        });
    }
    Ok(Some(trimmed.to_owned()))
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
        assert_eq!(t.metadata(), &TaskMetadata::default());
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

    #[test]
    fn metadata_can_be_derived_from_trigger_envelope() {
        use crate::value_objects::EventId;
        use time::macros::datetime;

        let envelope = EventEnvelope::new_with_causation(
            EventId::new("trigger-1").unwrap(),
            datetime!(2026-04-15 12:00:00 UTC),
            "pir",
            Some(EventId::new("corr-1").unwrap()),
            Some(EventId::new("cause-1").unwrap()),
        )
        .unwrap();

        let metadata = TaskMetadata::from_trigger_envelope(&envelope);

        assert_eq!(metadata.source_event_id().unwrap().as_str(), "trigger-1");
        assert_eq!(metadata.causation_id().unwrap().as_str(), "cause-1");
        assert_eq!(metadata.correlation_id().unwrap().as_str(), "corr-1");
    }
}
