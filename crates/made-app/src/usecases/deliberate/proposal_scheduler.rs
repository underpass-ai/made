use std::sync::Arc;

use futures::future::join_all;
use made_core::entities::Task;
use made_core::error::DomainError;
use made_core::ports::{AgentPort, DraftRequest, Revision};
use made_core::value_objects::{DiversityPreference, MaxParallel};
use tokio::sync::Semaphore;

/// A call pool shared by every deliberation using one application service.
pub(super) struct ProposalScheduler {
    permits: Semaphore,
}

impl ProposalScheduler {
    pub(super) fn new(limit: MaxParallel) -> Self {
        Self {
            permits: Semaphore::new(usize::from(limit.get())),
        }
    }

    pub(super) async fn generate(
        &self,
        agents: &[Arc<dyn AgentPort>],
        task: &Task,
    ) -> Vec<Result<Revision, DomainError>> {
        // Drain every accepted future before returning the first failure in
        // agent order. Dropping siblings on an early error would hide provider
        // calls that may already have produced an external effect.
        join_all(agents.iter().map(|agent| async {
            let _permit =
                self.permits
                    .acquire()
                    .await
                    .map_err(|_| DomainError::InvariantViolated {
                        reason: "proposal permit pool closed",
                    })?;
            agent.generate(request(task)).await
        }))
        .await
    }

    #[cfg(test)]
    pub(super) async fn sequential(
        agents: &[Arc<dyn AgentPort>],
        task: &Task,
    ) -> Result<Vec<Revision>, DomainError> {
        let mut drafts = Vec::with_capacity(agents.len());
        for agent in agents {
            drafts.push(agent.generate(request(task)).await?);
        }
        Ok(drafts)
    }
}

pub(super) fn request(task: &Task) -> DraftRequest {
    DraftRequest {
        task: task.description().clone(),
        constraints: task.constraints().clone(),
        diversity: DiversityPreference::Diverse,
        external_context: task.external_context().cloned(),
    }
}

#[cfg(test)]
mod tests;
