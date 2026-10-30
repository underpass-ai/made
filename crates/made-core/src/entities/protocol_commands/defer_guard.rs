use time::OffsetDateTime;

use crate::value_objects::{AuditActorKind, GuardName, ProtocolGuardDeferralContent, RoleId};

/// Leave a human guard undecided, on purpose and on the record.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeferGuard {
    pub guard_name: GuardName,
    pub content: ProtocolGuardDeferralContent,
    pub deferred_by: RoleId,
    pub deferred_by_kind: AuditActorKind,
    pub now: OffsetDateTime,
}
