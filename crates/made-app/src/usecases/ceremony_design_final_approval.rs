use made_core::value_objects::{GuardName, RoleId, TransitionTrigger};

/// The explicit human gate an author may put after the final stage.
///
/// Its guard and trigger are optional because most authors want the
/// gate and not the naming of it; the use case supplies the names when
/// they are left out, so the two arms cannot name it differently.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CeremonyDesignFinalApproval {
    role_id: RoleId,
    guard_name: Option<GuardName>,
    trigger: Option<TransitionTrigger>,
}

impl CeremonyDesignFinalApproval {
    #[must_use]
    pub const fn new(
        role_id: RoleId,
        guard_name: Option<GuardName>,
        trigger: Option<TransitionTrigger>,
    ) -> Self {
        Self {
            role_id,
            guard_name,
            trigger,
        }
    }

    #[must_use]
    pub const fn role_id(&self) -> &RoleId {
        &self.role_id
    }

    #[must_use]
    pub const fn guard_name(&self) -> Option<&GuardName> {
        self.guard_name.as_ref()
    }

    #[must_use]
    pub const fn trigger(&self) -> Option<&TransitionTrigger> {
        self.trigger.as_ref()
    }
}
