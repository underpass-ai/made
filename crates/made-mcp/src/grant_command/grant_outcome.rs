use made_core::value_objects::{AuthorizationGrantId, AuthorizationPolicyVersion};

/// How `grant` ended once it was allowed to ask.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GrantOutcome {
    /// The person said yes and the grant is in the policy's journal.
    /// `existing` says an identical grant under this id was already
    /// there, so nothing was appended.
    Recorded {
        grant_id: AuthorizationGrantId,
        version: AuthorizationPolicyVersion,
        existing: bool,
    },
    /// The person said no, or nothing. Nothing was written.
    Declined,
}
