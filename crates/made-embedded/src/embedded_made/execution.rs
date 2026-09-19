use made_app::budgets::{
    BudgetedStepClaimInput, BudgetedStepClaimOutput, StartBudgetedCeremonyInput,
    StartBudgetedCeremonyUseCase,
};
use made_app::usecases::StartCeremonyStepOutput;
use made_app::usecases::{
    AcceptChildCompletionInput, AcceptChildCompletionOutput, AcceptChildCompletionUseCase,
    ApplyCeremonyTransitionInput, ApplyCeremonyTransitionUseCase, BindCeremonyParticipantsInput,
    BindCeremonyParticipantsUseCase, CancelCeremonyInput, CancelCeremonyUseCase,
    CompleteCeremonyStepInput, CompleteCeremonyStepUseCase, EnforceCeremonyDeadlinesInput,
    EnforceCeremonyDeadlinesUseCase, PauseCeremonyInput, PauseCeremonyUseCase,
    PrepareCeremonyChildrenInput, PrepareCeremonyChildrenOutput, PrepareCeremonyChildrenUseCase,
    RecoverCeremonyChildrenRound, RecoverCeremonyChildrenUseCase, ResumeCeremonyInput,
    ResumeCeremonyUseCase, RunCeremonyInput, RunCeremonyOutput, RunCeremonyStepInput,
    RunCeremonyStepOutput, RunCeremonyStepUseCase, RunCeremonyUseCase, StartCeremonyInput,
    StartCeremonyStepInput, StartCeremonyStepUseCase, StartCeremonyUseCase,
    StartPublishedCeremonyUseCase,
};
use made_core::entities::CeremonyInstance;
use made_core::error::DomainError;
use made_core::value_objects::{CeremonyEventConsumer, CeremonyEventPageLimit};
use std::sync::Arc;

use super::EmbeddedMade;

impl EmbeddedMade {
    pub async fn run(&self, input: RunCeremonyInput) -> Result<RunCeremonyOutput, DomainError> {
        RunCeremonyUseCase::new(
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
        input: BindCeremonyParticipantsInput,
    ) -> Result<CeremonyInstance, DomainError> {
        BindCeremonyParticipantsUseCase::new(
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
        input: StartCeremonyInput,
    ) -> Result<CeremonyInstance, DomainError> {
        StartPublishedCeremonyUseCase::new(
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
        input: StartBudgetedCeremonyInput,
    ) -> Result<CeremonyInstance, made_core::BudgetError> {
        StartBudgetedCeremonyUseCase::new(
            self.publications.clone(),
            self.stream.clone(),
            self.clock.clone(),
            self.memory_reader.clone(),
            self.budgets.clone(),
        )
        .execute(input)
        .await
    }

    pub async fn start(&self, input: StartCeremonyInput) -> Result<CeremonyInstance, DomainError> {
        StartCeremonyUseCase::new(
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
        input: StartCeremonyStepInput,
    ) -> Result<StartCeremonyStepOutput, DomainError> {
        StartCeremonyStepUseCase::new(
            self.resolve_definition(),
            self.stream.clone(),
            self.clock.clone(),
        )
        .with_max_parallel_ceiling(self.max_parallel_ceiling)
        .execute(input)
        .await
    }

    pub async fn start_budgeted_step(
        &self,
        input: BudgetedStepClaimInput,
    ) -> Result<BudgetedStepClaimOutput, made_core::BudgetError> {
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
        input: RunCeremonyStepInput,
    ) -> Result<RunCeremonyStepOutput, DomainError> {
        Box::pin(
            RunCeremonyStepUseCase::new(
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
        input: PrepareCeremonyChildrenInput,
    ) -> Result<PrepareCeremonyChildrenOutput, DomainError> {
        self.child_orchestrator().execute(input).await
    }

    pub async fn accept_child_completion(
        &self,
        input: AcceptChildCompletionInput,
    ) -> Result<AcceptChildCompletionOutput, DomainError> {
        self.child_completion_acceptor().execute(input).await
    }

    pub async fn recover_children(
        &self,
        limit: CeremonyEventPageLimit,
    ) -> Result<RecoverCeremonyChildrenRound, DomainError> {
        RecoverCeremonyChildrenUseCase::new(
            self.events.clone(),
            self.cursors.clone(),
            self.stream.clone(),
            self.child_orchestrator(),
            self.child_completion_acceptor(),
            self.clock.clone(),
            CeremonyEventConsumer::new("made.children.recovery.v1")?,
        )
        .execute(limit)
        .await
    }

    fn child_orchestrator(&self) -> Arc<PrepareCeremonyChildrenUseCase> {
        Arc::new(PrepareCeremonyChildrenUseCase::new(
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
        input: CompleteCeremonyStepInput,
    ) -> Result<CeremonyInstance, DomainError> {
        CompleteCeremonyStepUseCase::new(
            self.resolve_definition(),
            self.stream.clone(),
            self.clock.clone(),
        )
        .execute(input)
        .await
    }

    pub async fn apply_transition(
        &self,
        input: ApplyCeremonyTransitionInput,
    ) -> Result<CeremonyInstance, DomainError> {
        ApplyCeremonyTransitionUseCase::new(
            self.resolve_definition(),
            self.stream.clone(),
            self.clock.clone(),
        )
        .execute(input)
        .await
    }

    pub async fn pause_ceremony(
        &self,
        input: PauseCeremonyInput,
    ) -> Result<CeremonyInstance, DomainError> {
        PauseCeremonyUseCase::new(
            self.resolve_definition(),
            self.stream.clone(),
            self.clock.clone(),
        )
        .execute(input)
        .await
    }

    pub async fn resume_ceremony(
        &self,
        input: ResumeCeremonyInput,
    ) -> Result<CeremonyInstance, DomainError> {
        ResumeCeremonyUseCase::new(
            self.resolve_definition(),
            self.stream.clone(),
            self.clock.clone(),
        )
        .execute(input)
        .await
    }

    pub async fn cancel_ceremony(
        &self,
        input: CancelCeremonyInput,
    ) -> Result<CeremonyInstance, DomainError> {
        CancelCeremonyUseCase::new(
            self.resolve_definition(),
            self.stream.clone(),
            self.clock.clone(),
        )
        .execute(input)
        .await
    }

    pub async fn enforce_ceremony_deadlines(
        &self,
        input: EnforceCeremonyDeadlinesInput,
    ) -> Result<CeremonyInstance, DomainError> {
        EnforceCeremonyDeadlinesUseCase::new(
            self.resolve_definition(),
            self.stream.clone(),
            self.clock.clone(),
        )
        .execute(input)
        .await
    }
}
