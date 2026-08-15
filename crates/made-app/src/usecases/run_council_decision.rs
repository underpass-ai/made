//! [`RunCouncilDecisionUseCase`] — Epic 9's consumer-facing surface.
//!
//! Composes [`DeliberateUseCase`] with the [`ContractRegistryPort`] so
//! a consumer can ask:
//!
//!   _"Run the council registered under specialty X (or council id Y)
//!    against contract Z, with this external context, and tell me
//!    whether the winning proposal satisfies the contract."_
//!
//! Crucially, this use case **does not** call any executor — the
//! result is the validated council decision itself. Consumers that
//! want to execute the winning proposal call the runtime separately.
//!
//! Two modes:
//!
//! * [`ValidationMode::Strict`] propagates `NoValidProposal` to the
//!   caller when no candidate satisfies the contract (mirrors
//!   today's `pick_winner` semantics).
//! * [`ValidationMode::Warn`] always returns the top-ranked
//!   candidate, even when none passed validation, with `passed=false`
//!   so the caller can reason about the failure without losing the
//!   proposal.
//!
//! Both modes return per-candidate validator reports so the caller
//! can route, retry, or audit on a richer signal than "winner /
//! no-winner".

use std::sync::Arc;

use made_core::entities::{
    Council, Deliberation, ExternalContextBundle, RankedOutcome, Task, TaskConstraints,
    TaskMetadata,
};
use made_core::error::DomainError;
use made_core::ports::{ContractRegistryPort, CouncilRegistryPort, DeliberationRepositoryPort};
use made_core::value_objects::{
    Attributes, CouncilSelector, DurationMs, Specialty, TaskDescription, TaskId, ValidationMode,
};
use tracing::info;
use uuid::Uuid;

use crate::usecases::DeliberateUseCase;

/// Input to [`RunCouncilDecisionUseCase::execute`].
#[derive(Debug, Clone)]
pub struct RunCouncilDecisionInput {
    pub council_selector: CouncilSelector,
    pub contract_id: String,
    pub task_description: TaskDescription,
    pub external_context: Option<ExternalContextBundle>,
    pub validation_mode: ValidationMode,
    pub metadata: TaskMetadata,
}

/// Validated council decision plus the candidate set the caller
/// needs to reason about the validation outcome.
#[derive(Debug, Clone)]
pub struct RunCouncilDecisionOutput {
    pub task_id: TaskId,
    pub winner: RankedOutcome,
    pub candidates: Vec<RankedOutcome>,
    pub validation_mode: ValidationMode,
    /// `true` iff at least the winning proposal passed every
    /// validator. In `Strict` mode this is always `true` (the use
    /// case wouldn't return `Ok` otherwise); in `Warn` mode it can
    /// be `false` when no candidate satisfied the contract.
    pub passed: bool,
    pub duration_ms: DurationMs,
}

pub struct RunCouncilDecisionUseCase {
    contracts: Arc<dyn ContractRegistryPort>,
    councils: Arc<dyn CouncilRegistryPort>,
    deliberate: Arc<DeliberateUseCase>,
    repository: Arc<dyn DeliberationRepositoryPort>,
}

impl std::fmt::Debug for RunCouncilDecisionUseCase {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RunCouncilDecisionUseCase").finish()
    }
}

impl RunCouncilDecisionUseCase {
    #[must_use]
    pub fn new(
        contracts: Arc<dyn ContractRegistryPort>,
        councils: Arc<dyn CouncilRegistryPort>,
        deliberate: Arc<DeliberateUseCase>,
        repository: Arc<dyn DeliberationRepositoryPort>,
    ) -> Self {
        Self {
            contracts,
            councils,
            deliberate,
            repository,
        }
    }

    pub async fn execute(
        &self,
        input: RunCouncilDecisionInput,
    ) -> Result<RunCouncilDecisionOutput, DomainError> {
        let specialty = self.resolve_specialty(&input.council_selector).await?;
        let contract = self.contracts.get(&input.contract_id).await?;

        let task_id = TaskId::new(Uuid::new_v4().to_string()).map_err(|_| {
            DomainError::InvariantViolated {
                reason: "generated task id failed validation",
            }
        })?;

        let constraints = TaskConstraints::default().with_output_contract(contract);
        let task = Task::new_with_metadata(
            task_id.clone(),
            specialty,
            input.task_description,
            constraints,
            Attributes::default(),
            input.external_context,
            input.metadata,
        );

        let started = std::time::Instant::now();

        match self.deliberate.execute(task).await {
            Ok(output) => {
                let duration_ms = elapsed_ms(started);
                let candidates = output.deliberation.ranked_outcomes()?;
                let winner = candidates
                    .iter()
                    .find(|c| c.proposal().id() == &output.winner_proposal_id)
                    .cloned()
                    .ok_or(DomainError::InvariantViolated {
                        reason: "winner proposal id not found in ranked outcomes",
                    })?;
                info!(
                    task_id = task_id.as_str(),
                    contract_id = input.contract_id.as_str(),
                    validation_mode = ?input.validation_mode,
                    duration_ms = duration_ms.get(),
                    "council decision succeeded"
                );
                Ok(RunCouncilDecisionOutput {
                    task_id,
                    winner,
                    candidates,
                    validation_mode: input.validation_mode,
                    passed: true,
                    duration_ms,
                })
            }
            Err(DomainError::NoValidProposal { contract_id })
                if input.validation_mode == ValidationMode::Warn =>
            {
                let deliberation = self.repository.get(&task_id).await?;
                let candidates = deliberation.ranked_outcomes()?;
                let winner = candidates
                    .first()
                    .cloned()
                    .ok_or(DomainError::EmptyCollection {
                        field: "deliberation.ranked",
                    })?;
                let duration_ms = elapsed_ms(started);
                info!(
                    task_id = task_id.as_str(),
                    contract_id = contract_id.as_str(),
                    validation_mode = ?input.validation_mode,
                    duration_ms = duration_ms.get(),
                    "council decision returned top-ranked despite failing contract"
                );
                Ok(RunCouncilDecisionOutput {
                    task_id,
                    winner,
                    candidates,
                    validation_mode: input.validation_mode,
                    passed: false,
                    duration_ms,
                })
            }
            Err(err) => Err(err),
        }
    }

    async fn resolve_specialty(
        &self,
        selector: &CouncilSelector,
    ) -> Result<Specialty, DomainError> {
        match selector {
            CouncilSelector::BySpecialty(specialty) => Ok(specialty.clone()),
            CouncilSelector::ById(council_id) => self
                .councils
                .list()
                .await?
                .into_iter()
                .find(|c: &Council| c.id() == council_id)
                .map(|c| c.specialty().clone())
                .ok_or(DomainError::NotFound { what: "council" }),
        }
    }
}

fn elapsed_ms(started: std::time::Instant) -> DurationMs {
    let millis = u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX);
    DurationMs::from_millis(millis)
}

// Hint for the deliberation repository accessor — it lives in
// `Deliberation::ranked_outcomes` (see `crates/made-core/src/entities/deliberation.rs`).
// Imported as a member of `Deliberation` here; explicit re-export below
// keeps `cargo doc` happy.
#[allow(unused_imports)]
use Deliberation as _Deliberation;

#[cfg(test)]
mod tests {
    // Unit tests live next to the in-process fixture in the
    // integration-tests crate (see
    // `crates/made-tests-integration/tests/run_council_decision_rpc.rs`).
    // The full deliberation harness is heavy to stub here and would
    // duplicate the existing `deliberate.rs` test scaffolding; instead
    // we exercise the use case end-to-end through the gRPC fixture so
    // the same code path the RPC carries is what we assert against.
}
