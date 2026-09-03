use made_core::entities::{ExternalContextBundle, TaskMetadata};
use made_core::value_objects::{
    CouncilSelector, OutputContractId, TaskDescription, ValidationMode,
};

/// Input for a validated council decision without execution.
#[derive(Debug, Clone)]
pub struct RunCouncilDecisionInput {
    pub council_selector: CouncilSelector,
    pub contract_id: OutputContractId,
    pub task_description: TaskDescription,
    pub external_context: Option<ExternalContextBundle>,
    pub validation_mode: ValidationMode,
    pub metadata: TaskMetadata,
}
