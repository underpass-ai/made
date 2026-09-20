//! [`BindCeremonyIntegratorUseCase`] — who drives this ceremony now.

use std::sync::Arc;

use made_core::error::DomainError;
use made_core::ports::{BindOutcome, ClockPort, HostDeliveryLedgerPort, IntegratorBindingPort};
use made_core::value_objects::IntegratorBinding;

use super::BindCeremonyIntegratorInput;

/// Puts one host in charge of a scope, and moves the unanswered work.
///
/// Binding is not only a row. A scope has one integrator, so binding a
/// new one displaces whoever held it, and whatever that host was
/// offered and never answered has to be dealt with in the same breath
/// — either re-addressed to the replacement or left behind with the
/// reason it stopped. Work silently addressed to a destination nobody
/// is listening to is the failure this use case exists to prevent.
pub struct BindCeremonyIntegratorUseCase {
    bindings: Arc<dyn IntegratorBindingPort>,
    deliveries: Arc<dyn HostDeliveryLedgerPort>,
    clock: Arc<dyn ClockPort>,
}

impl std::fmt::Debug for BindCeremonyIntegratorUseCase {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("BindCeremonyIntegratorUseCase")
            .finish_non_exhaustive()
    }
}

impl BindCeremonyIntegratorUseCase {
    #[must_use]
    pub fn new(
        bindings: Arc<dyn IntegratorBindingPort>,
        deliveries: Arc<dyn HostDeliveryLedgerPort>,
        clock: Arc<dyn ClockPort>,
    ) -> Self {
        Self {
            bindings,
            deliveries,
            clock,
        }
    }

    #[tracing::instrument(
        name = "bind_ceremony_integrator",
        skip_all,
        fields(binding_id = %input.binding_id, role_id = %input.role_id)
    )]
    pub async fn execute(
        &self,
        input: BindCeremonyIntegratorInput,
    ) -> Result<BindOutcome, DomainError> {
        let binding = IntegratorBinding::new(
            input.binding_id,
            input.scope,
            input.role_id,
            input.destination,
            input.incarnation,
            self.clock.now(),
        );
        let outcome = self.bindings.bind(binding, input.replacement).await?;
        if let BindOutcome::Replaced { previous, current } = &outcome {
            self.move_the_work(previous, current, input.follow).await?;
        }
        Ok(outcome)
    }

    /// Deal with what the displaced host was offered and never answered.
    ///
    /// A failure here is logged rather than raised: the binding is
    /// already in force and the fence already up, so returning an error
    /// would tell the caller its host is not bound when it is. The
    /// deliveries stay where they are, addressed to a destination that
    /// can no longer take them, which is visible in the ledger and
    /// recoverable by binding again.
    async fn move_the_work(
        &self,
        previous: &IntegratorBinding,
        current: &IntegratorBinding,
        follow: made_core::value_objects::FollowReplacement,
    ) -> Result<(), DomainError> {
        let moved = self
            .deliveries
            .supersede(
                &previous.delivery_target(),
                &current.delivery_target(),
                follow,
                self.clock.now(),
            )
            .await;
        match moved {
            Ok(outcome) => {
                tracing::info!(
                    binding_id = %current.id(),
                    superseded = outcome.superseded().len(),
                    "the outgoing integrator's unanswered work was dealt with"
                );
                Ok(())
            }
            Err(error) => {
                tracing::warn!(
                    binding_id = %current.id(),
                    %error,
                    "the binding stands, but the outgoing host's work could not be moved"
                );
                Ok(())
            }
        }
    }
}

#[cfg(test)]
mod tests;
