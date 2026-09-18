use made_core::value_objects::{RoleId, StateIteration, StepId, StepInstructions};

use super::{CeremonyDesignJoin, CeremonyStagePatternKind};

/// Typed parameters for one reusable coordination fragment in a design.
#[derive(Debug, Clone, PartialEq)]
pub struct CeremonyDesignPatternStage {
    id: StepId,
    kind: CeremonyStagePatternKind,
    roles: Vec<RoleId>,
    instructions: StepInstructions,
    manager_role_id: Option<RoleId>,
    max_iterations: Option<StateIteration>,
    fallback_role_id: Option<RoleId>,
    join: CeremonyDesignJoin,
}

impl CeremonyDesignPatternStage {
    #[must_use]
    pub fn new(
        id: StepId,
        kind: CeremonyStagePatternKind,
        roles: Vec<RoleId>,
        instructions: StepInstructions,
    ) -> Self {
        Self {
            id,
            kind,
            roles,
            instructions,
            manager_role_id: None,
            max_iterations: None,
            fallback_role_id: None,
            join: CeremonyDesignJoin::AllStepsCompleted,
        }
    }

    #[must_use]
    pub fn with_manager_role(mut self, role_id: RoleId) -> Self {
        self.manager_role_id = Some(role_id);
        self
    }

    #[must_use]
    pub const fn with_max_iterations(mut self, max_iterations: StateIteration) -> Self {
        self.max_iterations = Some(max_iterations);
        self
    }

    #[must_use]
    pub fn with_fallback_role(mut self, role_id: RoleId) -> Self {
        self.fallback_role_id = Some(role_id);
        self
    }

    #[must_use]
    pub const fn with_join(mut self, join: CeremonyDesignJoin) -> Self {
        self.join = join;
        self
    }

    #[must_use]
    pub const fn id(&self) -> &StepId {
        &self.id
    }

    #[must_use]
    pub const fn kind(&self) -> CeremonyStagePatternKind {
        self.kind
    }

    #[must_use]
    pub fn roles(&self) -> &[RoleId] {
        &self.roles
    }

    #[must_use]
    pub fn instructions(&self) -> &str {
        self.instructions.as_str()
    }

    #[must_use]
    pub fn manager_role_id(&self) -> Option<&RoleId> {
        self.manager_role_id.as_ref()
    }

    #[must_use]
    pub const fn max_iterations(&self) -> Option<StateIteration> {
        self.max_iterations
    }

    #[must_use]
    pub fn fallback_role_id(&self) -> Option<&RoleId> {
        self.fallback_role_id.as_ref()
    }

    #[must_use]
    pub const fn join(&self) -> CeremonyDesignJoin {
        self.join
    }
}
