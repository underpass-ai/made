use crate::entities::{ExternalContextBundle, TaskConstraints};
use crate::value_objects::{DiversityPreference, TaskDescription};

/// Input for a fresh proposal.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DraftRequest {
    pub task: TaskDescription,
    pub constraints: TaskConstraints,
    pub diversity: DiversityPreference,
    pub external_context: Option<ExternalContextBundle>,
}
