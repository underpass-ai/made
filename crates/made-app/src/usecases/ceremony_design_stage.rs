use made_core::value_objects::{
    NumAgents, PriorContext, RoleId, Rounds, StepHandlerKind, StepId, StepInstructions,
};

use super::ceremony_design_repeat::CeremonyDesignRepeat;

/// One stage of the working session, in the author's terms.
///
/// Declaration order is execution order: the design is linear, so a
/// stage says what is done and who does it, and where it sits comes
/// from where it was written.
///
/// The optional fields are optional all the way down: what an omitted
/// handler, agent count or `see_prior` means is decided once, by
/// [`super::DesignCeremonyUseCase`], rather than by whichever adapter
/// took the call.
#[derive(Debug, Clone, PartialEq)]
pub struct CeremonyDesignStage {
    id: StepId,
    owner_role_id: RoleId,
    /// What the stage asks of whoever runs it, and how they know they
    /// are done. Free text: it reaches the step handler as its prompt
    /// and the engine never reads it.
    instructions: StepInstructions,
    handler: Option<StepHandlerKind>,
    prior_context: Option<PriorContext>,
    num_agents: Option<NumAgents>,
    review_rounds: Rounds,
    repeat: Option<CeremonyDesignRepeat>,
}

impl CeremonyDesignStage {
    #[must_use]
    pub fn new(
        id: StepId,
        owner_role_id: RoleId,
        instructions: StepInstructions,
        handler: Option<StepHandlerKind>,
        prior_context: Option<PriorContext>,
        num_agents: Option<NumAgents>,
        review_rounds: Rounds,
        repeat: Option<CeremonyDesignRepeat>,
    ) -> Self {
        Self {
            id,
            owner_role_id,
            instructions,
            handler,
            prior_context,
            num_agents,
            review_rounds,
            repeat,
        }
    }

    #[must_use]
    pub const fn id(&self) -> &StepId {
        &self.id
    }

    #[must_use]
    pub const fn owner_role_id(&self) -> &RoleId {
        &self.owner_role_id
    }

    #[must_use]
    pub fn instructions(&self) -> &str {
        self.instructions.as_str()
    }

    #[must_use]
    pub fn handler(&self) -> Option<&StepHandlerKind> {
        self.handler.as_ref()
    }

    #[must_use]
    pub const fn prior_context(&self) -> Option<PriorContext> {
        self.prior_context
    }

    #[must_use]
    pub const fn num_agents(&self) -> Option<NumAgents> {
        self.num_agents
    }

    #[must_use]
    pub const fn review_rounds(&self) -> Rounds {
        self.review_rounds
    }

    #[must_use]
    pub fn repeat(&self) -> Option<&CeremonyDesignRepeat> {
        self.repeat.as_ref()
    }
}
