use crate::entities::protocol_commands::{ApproveGuard, DeferGuard};
use crate::entities::ProtocolCommand;

use super::{
    AuditActorKind, DomainError, GuardName, OffsetDateTime, ProtocolDefinition,
    ProtocolGuardDeferralContent, ProtocolInstance, RoleId,
};

impl ProtocolInstance {
    /// Approving is checked the way deferring is. It used to take no
    /// definition at all, so any name at all could be "approved" —
    /// which wrote that name into the session context, told the caller
    /// it had succeeded, and left a session that would never move.
    pub fn approve_guard(
        &mut self,
        definition: &ProtocolDefinition,
        guard_name: &GuardName,
        approved_by: RoleId,
        approved_by_kind: AuditActorKind,
        now: OffsetDateTime,
    ) -> Result<(), DomainError> {
        let command = ProtocolCommand::ApproveGuard(ApproveGuard {
            guard_name: guard_name.clone(),
            approved_by,
            approved_by_kind,
            now,
        });
        let events = self.decide(&command, definition)?;
        self.apply_all(&events);
        Ok(())
    }

    pub fn defer_guard(
        &mut self,
        definition: &ProtocolDefinition,
        guard_name: GuardName,
        content: ProtocolGuardDeferralContent,
        deferred_by: RoleId,
        deferred_by_kind: AuditActorKind,
        now: OffsetDateTime,
    ) -> Result<(), DomainError> {
        let command = ProtocolCommand::DeferGuard(DeferGuard {
            guard_name,
            content,
            deferred_by,
            deferred_by_kind,
            now,
        });
        let events = self.decide(&command, definition)?;
        self.apply_all(&events);
        Ok(())
    }
}
