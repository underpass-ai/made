use made_embedded::EmbeddedMade;

use super::EmbeddedMadeMcpBackend;
use crate::human_approval_source::HumanApprovalSource;

impl EmbeddedMadeMcpBackend {
    #[must_use]
    pub fn new(made: EmbeddedMade) -> Self {
        Self {
            made,
            authorization: None,
            human_approval_source: HumanApprovalSource::Host,
        }
    }

    /// Choose which channel records a human guard approval. In
    /// `terminal` mode `made_approve_ceremony_guard` is refused and the
    /// person runs `made-mcp approve-guard` themselves.
    #[must_use]
    pub fn with_human_approval_source(mut self, source: HumanApprovalSource) -> Self {
        self.human_approval_source = source;
        self
    }
}
