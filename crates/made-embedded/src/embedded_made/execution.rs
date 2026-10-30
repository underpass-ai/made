use made_app::budgets::{
    BudgetedStepClaimInput, BudgetedStepClaimOutput, StartBudgetedProtocolInput,
    StartBudgetedProtocolUseCase,
};
use made_app::usecases::StartProtocolStepOutput;
use made_app::usecases::{
    AcceptChildCompletionInput, AcceptChildCompletionOutput, AcceptChildCompletionUseCase,
    ApplyProtocolTransitionInput, ApplyProtocolTransitionUseCase, BindProtocolParticipantsInput,
    BindProtocolParticipantsUseCase, CancelProtocolInput, CancelProtocolUseCase,
    CompleteProtocolStepInput, CompleteProtocolStepUseCase, EnforceProtocolDeadlinesInput,
    EnforceProtocolDeadlinesUseCase, PauseProtocolInput, PauseProtocolUseCase,
    PrepareProtocolChildrenInput, PrepareProtocolChildrenOutput, PrepareProtocolChildrenUseCase,
    RecoverProtocolChildrenRound, RecoverProtocolChildrenUseCase, ResumeProtocolInput,
    ResumeProtocolUseCase, RunProtocolInput, RunProtocolOutput, RunProtocolStepInput,
    RunProtocolStepOutput, RunProtocolStepUseCase, RunProtocolUseCase, StartProtocolInput,
    StartProtocolStepInput, StartProtocolStepUseCase, StartProtocolUseCase,
    StartPublishedProtocolUseCase,
};
use made_core::entities::ProtocolInstance;
use made_core::error::DomainError;
use made_core::ports::AuthorizationScopeResolverPort;
use made_core::value_objects::{
    AuthorizationAction, ProtocolEventConsumer, ProtocolEventPageLimit,
};
use std::sync::Arc;

use super::EmbeddedMade;

impl EmbeddedMade {
    pub async fn run(&self, input: RunProtocolInput) -> Result<RunProtocolOutput, DomainError> {
        self.require_authorized_protocol_action(AuthorizationAction::RunProtocol, input.id())?;
        RunProtocolUseCase::new(
            self.definitions.clone(),
            self.stream.clone(),
            self.step_handler.clone(),
            self.clock.clone(),
        )
        .with_metrics(self.metrics_recorder.clone())
        .with_max_parallel_ceiling(self.max_parallel_ceiling)
        .with_child_orchestrator(self.child_orchestrator())
        .execute(input)
        .await
    }

    /// Seat this session's roles.
    pub async fn bind_participants(
        &self,
        input: BindProtocolParticipantsInput,
    ) -> Result<ProtocolInstance, DomainError> {
        self.require_authorized_protocol_action(
            AuthorizationAction::BindProtocolParticipants,
            input.instance_id(),
        )?;
        BindProtocolParticipantsUseCase::new(
            self.resolve_definition(),
            self.stream.clone(),
            self.clock.clone(),
        )
        .execute(input)
        .await
    }

    /// Start an instance bound to a published definition's digest.
    pub async fn start_published(
        &self,
        input: StartProtocolInput,
    ) -> Result<ProtocolInstance, DomainError> {
        self.require_authorized_protocol_action(
            AuthorizationAction::StartPublishedProtocol,
            input.id(),
        )?;
        StartPublishedProtocolUseCase::new(
            self.publications.clone(),
            self.stream.clone(),
            self.clock.clone(),
            self.memory_reader.clone(),
        )
        .execute(input)
        .await
    }

    pub async fn start_budgeted_published(
        &self,
        input: StartBudgetedProtocolInput,
    ) -> Result<ProtocolInstance, made_core::BudgetError> {
        self.require_authorized_protocol_action(
            AuthorizationAction::StartPublishedProtocol,
            input.protocol_id(),
        )?;
        StartBudgetedProtocolUseCase::new(
            self.publications.clone(),
            self.stream.clone(),
            self.clock.clone(),
            self.memory_reader.clone(),
            self.budgets.clone(),
        )
        .execute(input)
        .await
    }

    pub async fn start(&self, input: StartProtocolInput) -> Result<ProtocolInstance, DomainError> {
        self.require_authorized_protocol_action(AuthorizationAction::StartProtocol, input.id())?;
        StartProtocolUseCase::new(
            self.definitions.clone(),
            self.stream.clone(),
            self.clock.clone(),
            self.memory_reader.clone(),
        )
        .execute(input)
        .await
    }

    pub async fn start_step(
        &self,
        input: StartProtocolStepInput,
    ) -> Result<StartProtocolStepOutput, DomainError> {
        self.require_authorized_protocol_action(
            AuthorizationAction::ClaimProtocolStep,
            input.instance_id(),
        )?;
        StartProtocolStepUseCase::new(
            self.resolve_definition(),
            self.stream.clone(),
            self.clock.clone(),
        )
        .with_max_parallel_ceiling(self.max_parallel_ceiling)
        .execute(input)
        .await
    }

    /// Renew a delegated producer; replay returns its original durable receipt.
    pub async fn renew_step_lease(
        &self,
        input: made_app::workers::RenewProtocolStepLeaseInput,
    ) -> Result<made_core::entities::protocol_events::StepLeaseRenewed, DomainError> {
        self.require_authorized_protocol_action(
            AuthorizationAction::RenewProtocolStepLease,
            &input.protocol_id,
        )?;
        let mut renewal = made_app::workers::RenewProtocolStepLeaseUseCase::new(
            self.stream.clone(),
            self.resolve_definition(),
            self.clock.clone(),
        );
        if let Some(authorization) = &self.authorization {
            renewal = renewal.with_reauthorization(authorization.reauthorize.clone());
        }
        Box::pin(renewal.execute_request(input)).await
    }

    pub async fn start_budgeted_step(
        &self,
        input: BudgetedStepClaimInput,
    ) -> Result<BudgetedStepClaimOutput, made_core::BudgetError> {
        self.require_authorized_protocol_action(
            AuthorizationAction::ClaimProtocolStep,
            input.protocol_id(),
        )?;
        made_app::budgets::BudgetedStepClaimUseCase::new(
            self.resolve_definition(),
            self.stream.clone(),
            self.clock.clone(),
            self.budgets.clone(),
        )
        .with_max_parallel_ceiling(self.max_parallel_ceiling)
        .execute(input)
        .await
    }

    pub async fn run_step(
        &self,
        input: RunProtocolStepInput,
    ) -> Result<RunProtocolStepOutput, DomainError> {
        self.require_authorized_protocol_action(
            AuthorizationAction::RunProtocolStep,
            input.instance_id(),
        )?;
        Box::pin(
            RunProtocolStepUseCase::new(
                self.resolve_definition(),
                self.stream.clone(),
                self.step_handler.clone(),
                self.clock.clone(),
            )
            .with_max_parallel_ceiling(self.max_parallel_ceiling)
            .with_child_orchestrator(self.child_orchestrator())
            .execute(input),
        )
        .await
    }

    pub async fn prepare_children(
        &self,
        input: PrepareProtocolChildrenInput,
    ) -> Result<PrepareProtocolChildrenOutput, DomainError> {
        self.require_authorized_protocol_action(
            AuthorizationAction::PrepareProtocolChildren,
            &input.instance_id,
        )?;
        self.child_orchestrator().execute(input).await
    }

    pub async fn accept_child_completion(
        &self,
        input: AcceptChildCompletionInput,
    ) -> Result<AcceptChildCompletionOutput, DomainError> {
        self.require_authorized_action(AuthorizationAction::AcceptChildCompletion)?;
        if self.authorization.is_some() {
            let parent_scope = self.child_parent_scope(&input.child_id).await?;
            self.require_authorized_scope_action(
                AuthorizationAction::AcceptChildCompletion,
                &parent_scope,
            )?;
        }
        self.child_completion_acceptor().execute(input).await
    }

    pub async fn recover_children(
        &self,
        limit: ProtocolEventPageLimit,
    ) -> Result<RecoverProtocolChildrenRound, DomainError> {
        let recovery = RecoverProtocolChildrenUseCase::new(
            self.events.clone(),
            self.cursors.clone(),
            self.stream.clone(),
            self.child_orchestrator(),
            self.child_completion_acceptor(),
            self.clock.clone(),
            ProtocolEventConsumer::new("made.children.recovery.v1")?,
        );
        let recovery = if let Some(authorization) = &self.authorization {
            recovery.with_authorization_continuation(authorization.continuation())
        } else {
            recovery
        };
        recovery.execute(limit).await
    }

    pub(super) fn child_orchestrator(&self) -> Arc<PrepareProtocolChildrenUseCase> {
        Arc::new(PrepareProtocolChildrenUseCase::new(
            self.resolve_definition(),
            self.publications.clone(),
            self.stream.clone(),
            self.clock.clone(),
            self.memory_reader.clone(),
        ))
    }

    fn child_completion_acceptor(&self) -> Arc<AcceptChildCompletionUseCase> {
        Arc::new(AcceptChildCompletionUseCase::new(
            self.resolve_definition(),
            self.publications.clone(),
            self.stream.clone(),
            self.clock.clone(),
        ))
    }

    pub async fn complete_step(
        &self,
        input: CompleteProtocolStepInput,
    ) -> Result<ProtocolInstance, DomainError> {
        self.require_authorized_protocol_action(
            AuthorizationAction::CompleteProtocolStep,
            input.instance_id(),
        )?;
        CompleteProtocolStepUseCase::new(
            self.resolve_definition(),
            self.stream.clone(),
            self.clock.clone(),
        )
        .execute(input)
        .await
    }

    pub async fn apply_transition(
        &self,
        input: ApplyProtocolTransitionInput,
    ) -> Result<ProtocolInstance, DomainError> {
        self.require_authorized_protocol_action(
            AuthorizationAction::ApplyProtocolTransition,
            input.instance_id(),
        )?;
        ApplyProtocolTransitionUseCase::new(
            self.resolve_definition(),
            self.stream.clone(),
            self.clock.clone(),
        )
        .execute(input)
        .await
    }

    pub async fn pause_protocol(
        &self,
        input: PauseProtocolInput,
    ) -> Result<ProtocolInstance, DomainError> {
        self.require_authorized_protocol_action(
            AuthorizationAction::PauseProtocol,
            input.instance_id(),
        )?;
        PauseProtocolUseCase::new(
            self.resolve_definition(),
            self.stream.clone(),
            self.clock.clone(),
        )
        .execute(input)
        .await
    }

    pub async fn resume_protocol(
        &self,
        input: ResumeProtocolInput,
    ) -> Result<ProtocolInstance, DomainError> {
        self.require_authorized_protocol_action(
            AuthorizationAction::ResumeProtocol,
            input.instance_id(),
        )?;
        ResumeProtocolUseCase::new(
            self.resolve_definition(),
            self.stream.clone(),
            self.clock.clone(),
        )
        .execute(input)
        .await
    }

    pub async fn cancel_protocol(
        &self,
        input: CancelProtocolInput,
    ) -> Result<ProtocolInstance, DomainError> {
        self.require_authorized_protocol_action(
            AuthorizationAction::CancelProtocol,
            input.instance_id(),
        )?;
        CancelProtocolUseCase::new(
            self.resolve_definition(),
            self.stream.clone(),
            self.clock.clone(),
        )
        .execute(input)
        .await
    }

    pub async fn enforce_protocol_deadlines(
        &self,
        input: EnforceProtocolDeadlinesInput,
    ) -> Result<ProtocolInstance, DomainError> {
        self.require_authorized_protocol_action(
            AuthorizationAction::EnforceProtocolDeadlines,
            input.instance_id(),
        )?;
        EnforceProtocolDeadlinesUseCase::new(
            self.resolve_definition(),
            self.stream.clone(),
            self.clock.clone(),
        )
        .execute(input)
        .await
    }
}
