use made_core::entities::Deliberation;
use made_core::value_objects::ProposalId;

/// Completed deliberation and the selected proposal identity.
#[derive(Debug, Clone)]
pub struct DeliberateOutput {
    pub deliberation: Deliberation,
    pub winner_proposal_id: ProposalId,
}
