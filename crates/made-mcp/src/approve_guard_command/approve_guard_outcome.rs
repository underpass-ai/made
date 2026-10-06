use made_core::value_objects::{AuthorizationDecisionId, GuardName, RoleId};
use time::OffsetDateTime;

/// How `approve-guard` ended once it was allowed to ask.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ApproveGuardOutcome {
    /// The person said yes and the approval is sealed in the journal
    /// under the authorization decision that admitted it.
    Recorded {
        guard_name: GuardName,
        role_id: RoleId,
        approved_at: OffsetDateTime,
        decision_id: AuthorizationDecisionId,
    },
    /// The person said no, or nothing. Nothing was written.
    Declined,
}
