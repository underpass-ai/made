use crate::value_objects::ProposalContent;

/// Revised proposal content returned by an agent.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Revision {
    pub content: ProposalContent,
}
