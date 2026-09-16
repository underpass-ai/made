use time::OffsetDateTime;

use crate::value_objects::{AuditActorKind, GuardName, RoleId};

/// Let a human guard through.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApproveGuard {
    pub guard_name: GuardName,
    pub approved_by: RoleId,
    pub approved_by_kind: AuditActorKind,
    pub now: OffsetDateTime,
}
