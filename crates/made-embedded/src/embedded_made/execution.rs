use made_app::usecases::{
    ApplyCeremonyTransitionInput, ApplyCeremonyTransitionUseCase, BindCeremonyParticipantsInput,
    BindCeremonyParticipantsUseCase, CompleteCeremonyStepInput, CompleteCeremonyStepUseCase,
    RunCeremonyInput, RunCeremonyOutput, RunCeremonyStepInput, RunCeremonyStepOutput,
    RunCeremonyStepUseCase, RunCeremonyUseCase, StartCeremonyInput, StartCeremonyStepInput,
    StartCeremonyStepUseCase, StartCeremonyUseCase, StartPublishedCeremonyUseCase,
};
use made_core::entities::CeremonyInstance;
use made_core::error::DomainError;
use made_core::value_objects::StepAttempt;

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
    ) -> Result<StepAttempt, DomainError> {
        StartCeremonyStepUseCase::new(
            self.resolve_definition(),
            self.stream.clone(),
            self.clock.clone(),
        )
        .with_max_parallel_ceiling(self.max_parallel_ceiling)
        .execute(input)
        .await
    }

    pub async fn run_step(
        &self,
        input: RunCeremonyStepInput,
    ) -> Result<RunCeremonyStepOutput, DomainError> {
        RunCeremonyStepUseCase::new(
            self.resolve_definition(),
            self.stream.clone(),
            self.step_handler.clone(),
            self.clock.clone(),
        )
        .with_max_parallel_ceiling(self.max_parallel_ceiling)
        .execute(input)
        .await
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
}
