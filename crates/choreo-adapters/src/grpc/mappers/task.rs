//! Task & constraints proto → domain conversion.

use choreo_core::entities::{Task, TaskConstraints, TaskMetadata};
use choreo_core::error::DomainError;
use choreo_core::value_objects::{
    DurationMs, EventId, NumAgents, Rounds, Specialty, TaskDescription, TaskId,
};
use choreo_proto::v1 as pb;

use super::attributes::{attributes_from_struct, rubric_from_struct};
use super::context::external_context_from_proto;
use super::output_contract::output_contract_from_proto;

/// Map a proto `Task` onto a domain [`Task`]. Validation failures
/// surface as [`DomainError`].
pub fn task_from_proto(t: pb::Task) -> Result<Task, DomainError> {
    let id = TaskId::new(t.task_id)?;
    let specialty = Specialty::new(t.specialty)?;
    let description = TaskDescription::new(t.description)?;
    let constraints = constraints_from_proto(t.constraints)?;
    let attributes = attributes_from_struct(t.attributes)?;
    let external_context = external_context_from_proto(t.external_context)?;
    let metadata = metadata_from_proto(t.metadata)?;
    Ok(Task::new_with_metadata(
        id,
        specialty,
        description,
        constraints,
        attributes,
        external_context,
        metadata,
    ))
}

fn constraints_from_proto(c: Option<pb::Constraints>) -> Result<TaskConstraints, DomainError> {
    let Some(c) = c else {
        return Ok(TaskConstraints::default());
    };
    let rounds = if c.rounds == 0 {
        Rounds::default()
    } else {
        Rounds::new(c.rounds)?
    };
    let num_agents = if c.num_agents == 0 {
        None
    } else {
        Some(NumAgents::new(c.num_agents)?)
    };
    let deadline = if c.deadline_ms == 0 {
        None
    } else {
        Some(DurationMs::from_millis(c.deadline_ms))
    };
    let rubric = rubric_from_struct(c.rubric)?;
    let mut constraints = TaskConstraints::new(rubric, rounds, num_agents, deadline);
    if let Some(output_contract) = output_contract_from_proto(c.output_contract)? {
        constraints = constraints.with_output_contract(output_contract);
    }
    Ok(constraints)
}

pub(crate) fn metadata_from_proto(
    metadata: Option<pb::TaskMetadata>,
) -> Result<TaskMetadata, DomainError> {
    let Some(metadata) = metadata else {
        return Ok(TaskMetadata::default());
    };

    TaskMetadata::new(
        optional_event_id(&metadata.source_event_id)?,
        optional_event_id(&metadata.causation_id)?,
        optional_event_id(&metadata.correlation_id)?,
        optional_text(&metadata.council_contract_id),
        optional_text(&metadata.output_contract_id),
        attributes_from_struct(metadata.execution_profile)?,
    )
}

fn optional_event_id(value: &str) -> Result<Option<EventId>, DomainError> {
    optional_text(value).map(EventId::new).transpose()
}

fn optional_text(value: &str) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_owned())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use prost_types::Struct as PbStruct;

    fn sample_proto(task_id: &str, specialty: &str, description: &str) -> pb::Task {
        pb::Task {
            task_id: task_id.to_owned(),
            specialty: specialty.to_owned(),
            description: description.to_owned(),
            constraints: Some(pb::Constraints {
                rubric: Some(PbStruct::default()),
                rounds: 2,
                num_agents: 3,
                deadline_ms: 1000,
                output_contract: None,
            }),
            attributes: Some(PbStruct::default()),
            external_context: None,
            metadata: None,
        }
    }

    #[test]
    fn well_formed_task_roundtrips() {
        let t = task_from_proto(sample_proto("t1", "triage", "describe alert")).unwrap();
        assert_eq!(t.id().as_str(), "t1");
        assert_eq!(t.specialty().as_str(), "triage");
        assert_eq!(t.description().as_str(), "describe alert");
        assert_eq!(t.constraints().rounds().get(), 2);
        assert_eq!(t.constraints().num_agents().unwrap().get(), 3);
        assert_eq!(t.constraints().deadline().unwrap().get(), 1000);
    }

    #[test]
    fn missing_constraints_block_yields_domain_defaults() {
        let mut t = sample_proto("t1", "triage", "x");
        t.constraints = None;
        let task = task_from_proto(t).unwrap();
        assert_eq!(task.constraints().rounds(), Rounds::default());
        assert!(task.constraints().num_agents().is_none());
        assert!(task.constraints().deadline().is_none());
    }

    #[test]
    fn zero_wire_values_are_mapped_to_domain_defaults() {
        // `rounds=0` on the wire means "use implementation default";
        // `num_agents=0` / `deadline_ms=0` mean "unspecified".
        let t = pb::Task {
            task_id: "t".to_owned(),
            specialty: "s".to_owned(),
            description: "d".to_owned(),
            constraints: Some(pb::Constraints {
                rubric: None,
                rounds: 0,
                num_agents: 0,
                deadline_ms: 0,
                output_contract: None,
            }),
            attributes: None,
            external_context: None,
            metadata: None,
        };
        let task = task_from_proto(t).unwrap();
        assert_eq!(task.constraints().rounds(), Rounds::default());
        assert!(task.constraints().num_agents().is_none());
        assert!(task.constraints().deadline().is_none());
    }

    #[test]
    fn empty_task_id_is_rejected() {
        let t = pb::Task {
            task_id: String::new(),
            specialty: "s".to_owned(),
            description: "d".to_owned(),
            constraints: None,
            attributes: None,
            external_context: None,
            metadata: None,
        };
        let err = task_from_proto(t).unwrap_err();
        assert!(matches!(err, DomainError::EmptyField { field: "task_id" }));
    }

    #[test]
    fn empty_specialty_is_rejected() {
        let mut t = sample_proto("t", "", "d");
        t.constraints = None;
        let err = task_from_proto(t).unwrap_err();
        assert!(matches!(
            err,
            DomainError::EmptyField { field: "specialty" }
        ));
    }

    #[test]
    fn empty_description_is_rejected() {
        let mut t = sample_proto("t", "s", "");
        t.constraints = None;
        let err = task_from_proto(t).unwrap_err();
        assert!(matches!(
            err,
            DomainError::EmptyField {
                field: "task_description"
            }
        ));
    }

    #[test]
    fn out_of_range_rounds_is_rejected() {
        let mut t = sample_proto("t", "s", "d");
        t.constraints.as_mut().unwrap().rounds = 1_000_000;
        let err = task_from_proto(t).unwrap_err();
        assert!(matches!(
            err,
            DomainError::OutOfRange {
                field: "rounds",
                ..
            }
        ));
    }

    #[test]
    fn task_metadata_maps_to_domain() {
        let mut t = sample_proto("t1", "triage", "describe alert");
        t.metadata = Some(pb::TaskMetadata {
            source_event_id: "source-1".to_owned(),
            causation_id: "cause-1".to_owned(),
            correlation_id: "corr-1".to_owned(),
            council_contract_id: "council-contract".to_owned(),
            output_contract_id: "output-contract".to_owned(),
            execution_profile: Some(PbStruct::default()),
        });

        let task = task_from_proto(t).unwrap();

        assert_eq!(
            task.metadata().source_event_id().unwrap().as_str(),
            "source-1"
        );
        assert_eq!(task.metadata().causation_id().unwrap().as_str(), "cause-1");
        assert_eq!(task.metadata().correlation_id().unwrap().as_str(), "corr-1");
        assert_eq!(
            task.metadata().council_contract_id(),
            Some("council-contract")
        );
        assert_eq!(
            task.metadata().output_contract_id(),
            Some("output-contract")
        );
    }
}
