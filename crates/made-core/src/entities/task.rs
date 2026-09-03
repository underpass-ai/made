//! [`Task`] entity — a unit of work submitted for deliberation.

use serde::{Deserialize, Serialize};

use crate::entities::{ExternalContextBundle, TaskConstraints, TaskMetadata};
use crate::value_objects::{Attributes, Specialty, TaskDescription, TaskId};

/// A task submitted to MADE.
///
/// `description` is the free-form prompt that agents consume;
/// `attributes` carries arbitrary, opaque domain data that
/// MADE does not interpret. The `specialty` selects which
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::value_objects::{Attributes, Specialty, TaskDescription, TaskId};

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
