use made_core::value_objects::{GuardCondition, GuardName, RoleId, StepId, TransitionTrigger};

/// Internal route emitted while a high-level stage pattern is materialized.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CeremonyDesignRoute {
    from: StepId,
    to: StepId,
    trigger: TransitionTrigger,
    owner: RoleId,
    guards: Vec<(GuardName, GuardCondition)>,
}

impl CeremonyDesignRoute {
    pub(crate) fn new(
        from: StepId,
        to: StepId,
        trigger: TransitionTrigger,
        owner: RoleId,
        guards: Vec<(GuardName, GuardCondition)>,
    ) -> Self {
        Self {
            from,
            to,
            trigger,
            owner,
            guards,
        }
    }

    pub(crate) fn from(&self) -> &StepId {
        &self.from
    }
    pub(crate) fn to(&self) -> &StepId {
        &self.to
    }
    pub(crate) fn trigger(&self) -> &TransitionTrigger {
        &self.trigger
    }
    pub(crate) fn owner(&self) -> &RoleId {
        &self.owner
    }
    pub(crate) fn guards(&self) -> &[(GuardName, GuardCondition)] {
        &self.guards
    }
}
