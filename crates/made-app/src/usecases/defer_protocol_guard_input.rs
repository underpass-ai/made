use made_core::value_objects::{
    AuditActorKind, GuardName, ProtocolGuardDeferralContent, ProtocolId, RoleId,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeferProtocolGuardInput {
    pub(crate) instance_id: ProtocolId,
    pub(crate) guard_name: GuardName,
    pub(crate) content: ProtocolGuardDeferralContent,
    /// The seat deferring. A deferral already records what was
    /// decided, why, and what would revisit it; who is the fourth, and
    /// the only one nobody can reconstruct afterwards.
    pub(crate) role_id: RoleId,
    /// What kind of party the caller says filled that seat.
    /// Declared, not deduced.
    pub(crate) role_kind: AuditActorKind,
}

impl DeferProtocolGuardInput {
    #[must_use]
    pub fn new(
        instance_id: ProtocolId,
        guard_name: GuardName,
        content: ProtocolGuardDeferralContent,
        role_id: RoleId,
        role_kind: AuditActorKind,
    ) -> Self {
        Self {
            instance_id,
            guard_name,
            content,
            role_id,
            role_kind,
        }
    }
    #[must_use]
    pub fn guard_name(&self) -> &GuardName {
        &self.guard_name
    }

    #[must_use]
    pub fn content(&self) -> &ProtocolGuardDeferralContent {
        &self.content
    }

    #[must_use]
    pub const fn instance_id(&self) -> &ProtocolId {
        &self.instance_id
    }
}
