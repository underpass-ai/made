use super::{
    AuditActorKind, CeremonyDefinition, CeremonyGuardApproval, CeremonyGuardDeferral,
    CeremonyGuardDeferralContent, CeremonyInstance, DomainError, GuardCondition, GuardName,
    OffsetDateTime, RoleId,
};

impl CeremonyInstance {
    /// Approving is checked the way deferring is. It used to take no
    /// definition at all, so any name at all could be "approved" —
    /// which wrote that name into the session context, told the caller
    /// it had succeeded, and left a session that would never move.
    ///
    /// Approving ahead of time is still allowed: unlike a deferral,
    /// which answers a decision being asked for now, a person may
    /// settle a guard before the work leading up to it is finished.
    pub fn approve_guard(
        &mut self,
        definition: &CeremonyDefinition,
        guard_name: &GuardName,
        approved_by: RoleId,
        approved_by_kind: AuditActorKind,
        now: OffsetDateTime,
    ) -> Result<(), DomainError> {
        self.require_active(
            definition,
            "terminal ceremony instances cannot approve guards",
        )?;
        let guard = definition
            .guards()
            .get(guard_name)
            .ok_or(DomainError::NotFound {
                what: "ceremony_guard",
            })?;
        if !matches!(guard.condition(), GuardCondition::HumanApproval) {
            return Err(DomainError::InvariantViolated {
                reason: "only human approval guards can be approved",
            });
        }
        self.require_declared_role(definition, &approved_by)?;
        self.context = self.context.clone().with_guard_approval(guard_name)?;
        self.guard_approvals.push(CeremonyGuardApproval::record(
            guard_name.clone(),
            approved_by,
            approved_by_kind,
            now,
        ));
        self.updated_at = now;
        Ok(())
    }

    pub fn defer_guard(
        &mut self,
        definition: &CeremonyDefinition,
        guard_name: GuardName,
        content: CeremonyGuardDeferralContent,
        deferred_by: RoleId,
        deferred_by_kind: AuditActorKind,
        now: OffsetDateTime,
    ) -> Result<(), DomainError> {
        self.require_active(
            definition,
            "terminal ceremony instances cannot defer guard decisions",
        )?;
        let guard = definition
            .guards()
            .get(&guard_name)
            .ok_or(DomainError::NotFound {
                what: "ceremony_guard",
            })?;
        if !matches!(guard.condition(), GuardCondition::HumanApproval) {
            return Err(DomainError::InvariantViolated {
                reason: "only human approval guards can be deferred",
            });
        }
        if self.context.is_guard_approved(&guard_name) {
            return Err(DomainError::InvariantViolated {
                reason: "approved human guards cannot be deferred",
            });
        }
        let is_currently_required = definition
            .available_transitions(&self.current_state)
            .any(|transition| transition.required_guards().contains(&guard_name));
        if !is_currently_required {
            return Err(DomainError::InvariantViolated {
                reason: "human guard is not required from the current state",
            });
        }

        self.require_declared_role(definition, &deferred_by)?;
        self.guard_deferrals.push(CeremonyGuardDeferral::record(
            guard_name,
            deferred_by,
            deferred_by_kind,
            content,
            now,
        ));
        self.updated_at = now;
        Ok(())
    }
}
