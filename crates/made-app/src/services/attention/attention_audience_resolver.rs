//! Reading a binding's scope as the ceremonies it covers right now.

use std::collections::BTreeSet;
use std::fmt;
use std::sync::Arc;

use made_core::error::DomainError;
use made_core::ports::{AgenticSystemExecutionStorePort, AgenticSystemRepositoryPort};
use made_core::value_objects::{AttentionPolicy, IntegratorBinding, IntegratorScope};

use super::AttentionAudience;

/// Turns one binding into the audience a projection round serves.
///
/// A scope is not a list. An execution of a composed system owns
/// several ceremonies and opens more as it runs, so the set has to be
/// read again each round rather than fixed when the binding was made —
/// a projector that cached it would stop waking the integrator for the
/// second ceremony of its own run.
///
/// # What a ceremony-scoped binding cannot say yet
///
/// Nothing in a binding carries an attention policy, so a binding
/// outside a system gets the defaults. A composed system holds its own
/// policy and that one is honoured. Until a binding can carry one, an
/// integrator bound to a single ceremony cannot ask to hear about less
/// than everything.
pub struct AttentionAudienceResolver {
    executions: Arc<dyn AgenticSystemExecutionStorePort>,
    systems: Arc<dyn AgenticSystemRepositoryPort>,
}

impl fmt::Debug for AttentionAudienceResolver {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("AttentionAudienceResolver")
            .finish_non_exhaustive()
    }
}

impl AttentionAudienceResolver {
    #[must_use]
    pub fn new(
        executions: Arc<dyn AgenticSystemExecutionStorePort>,
        systems: Arc<dyn AgenticSystemRepositoryPort>,
    ) -> Self {
        Self {
            executions,
            systems,
        }
    }

    /// What this binding covers, and what it asked to hear about.
    pub async fn resolve(
        &self,
        binding: &IntegratorBinding,
    ) -> Result<AttentionAudience, DomainError> {
        match binding.scope() {
            IntegratorScope::Ceremony { ceremony_id } => Ok(AttentionAudience::new(
                binding.clone(),
                AttentionPolicy::default(),
                BTreeSet::from([ceremony_id.clone()]),
                None,
            )),
            IntegratorScope::SystemExecution {
                system_execution_id,
            } => {
                let execution = self.executions.get(system_execution_id).await?;
                // A binding whose run is gone covers nothing. Saying so
                // is not the same as failing: the cursor still advances
                // and the loop reads as having nothing to do, which is
                // what a host should be told about a run that ended
                // underneath it.
                let Some(execution) = execution else {
                    return Ok(AttentionAudience::new(
                        binding.clone(),
                        AttentionPolicy::default(),
                        BTreeSet::new(),
                        Some(system_execution_id.clone()),
                    ));
                };
                let covers = execution
                    .ceremonies()
                    .values()
                    .filter_map(|link| link.instance_id().cloned())
                    .collect();
                let policy = self
                    .systems
                    .get(execution.system().id(), Some(execution.system().revision()))
                    .await?
                    .map_or_else(AttentionPolicy::default, |system| {
                        system.attention().clone()
                    });
                Ok(AttentionAudience::new(
                    binding.clone(),
                    policy,
                    covers,
                    Some(system_execution_id.clone()),
                ))
            }
        }
    }
}

#[cfg(test)]
mod tests;
