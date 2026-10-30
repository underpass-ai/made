use made_core::value_objects::{
    ContextWrites, DynamicRoleBinding, NumAgents, PriorContext, ProtocolChildSpawn,
    ProtocolStepAggregation, RoleId, Rounds, StepHandlerKind, StepId, StepInstructions,
    StepTimeout,
};

use super::protocol_design_repeat::ProtocolDesignRepeat;
use super::ProtocolDesignExitGuard;

/// One stage of the working session, in the author's terms.
///
/// Declaration order is execution order: the design is linear, so a
/// stage says what is done and who does it, and where it sits comes
/// from where it was written.
///
/// The optional fields are optional all the way down: what an omitted
/// handler, agent count or `see_prior` means is decided once, by
/// [`super::DesignProtocolUseCase`], rather than by whichever adapter
/// took the call.
#[derive(Debug, Clone, PartialEq)]
pub struct ProtocolDesignStage {
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
    repeat: Option<ProtocolDesignRepeat>,
    exit_guards: Vec<ProtocolDesignExitGuard>,
    dynamic_role_binding: Option<DynamicRoleBinding>,
    context_writes: ContextWrites,
    aggregation: Option<ProtocolStepAggregation>,
    spawn: Option<ProtocolChildSpawn>,
    /// This stage's own technical timeout. Absent means the protocol's
    /// `step_timeout_seconds`, resolved by the designer like every
    /// other omission.
    timeout: Option<StepTimeout>,
}

impl ProtocolDesignStage {
    #[must_use]
    pub fn new(
        id: StepId,
        owner_role_id: RoleId,
        instructions: StepInstructions,
        handler: Option<StepHandlerKind>,
        prior_context: Option<PriorContext>,
        num_agents: Option<NumAgents>,
        review_rounds: Rounds,
        repeat: Option<ProtocolDesignRepeat>,
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
            exit_guards: Vec::new(),
            dynamic_role_binding: None,
            context_writes: ContextWrites::default(),
            aggregation: None,
            spawn: None,
            timeout: None,
        }
    }

    #[must_use]
    pub fn with_dynamic_role_binding(mut self, binding: DynamicRoleBinding) -> Self {
        self.dynamic_role_binding = Some(binding);
        self
    }

    #[must_use]
    pub fn with_context_writes(mut self, writes: ContextWrites) -> Self {
        self.context_writes = writes;
        self
    }

    #[must_use]
    pub fn with_exit_guards(mut self, exit_guards: Vec<ProtocolDesignExitGuard>) -> Self {
        self.exit_guards = exit_guards;
        self
    }

    #[must_use]
    pub fn with_aggregation(mut self, aggregation: ProtocolStepAggregation) -> Self {
        self.aggregation = Some(aggregation);
        self
    }

    #[must_use]
    pub fn with_spawn(mut self, spawn: ProtocolChildSpawn) -> Self {
        self.spawn = Some(spawn);
        self
    }

    /// Give this stage its own technical timeout, overriding the
    /// protocol-wide default for this stage alone.
    #[must_use]
    pub const fn with_timeout(mut self, timeout: StepTimeout) -> Self {
        self.timeout = Some(timeout);
        self
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
    pub fn repeat(&self) -> Option<&ProtocolDesignRepeat> {
        self.repeat.as_ref()
    }

    #[must_use]
    pub fn exit_guards(&self) -> &[ProtocolDesignExitGuard] {
        &self.exit_guards
    }

    #[must_use]
    pub fn dynamic_role_binding(&self) -> Option<&DynamicRoleBinding> {
        self.dynamic_role_binding.as_ref()
    }

    #[must_use]
    pub fn context_writes(&self) -> &ContextWrites {
        &self.context_writes
    }

    #[must_use]
    pub fn aggregation(&self) -> Option<&ProtocolStepAggregation> {
        self.aggregation.as_ref()
    }

    #[must_use]
    pub fn spawn(&self) -> Option<&ProtocolChildSpawn> {
        self.spawn.as_ref()
    }

    #[must_use]
    pub const fn timeout(&self) -> Option<StepTimeout> {
        self.timeout
    }
}
