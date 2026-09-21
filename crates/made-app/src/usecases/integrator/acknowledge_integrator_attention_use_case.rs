//! [`AcknowledgeIntegratorAttentionUseCase`] — the host comes back and
//! says what it is doing, and later what it did.

use std::sync::Arc;

use made_core::error::DomainError;
use made_core::ports::{ClockPort, HostDeliveryLedgerPort, IntegratorBindingPort};
use made_core::value_objects::{HostDeliveryObservation, HostDeliveryObservationKind};

use super::{
    AcknowledgeIntegratorAttentionInput, IntegratorAcknowledgement, IntegratorAttentionAcknowledged,
};

/// Records intent before effect, and effect after it.
///
/// An attention event confers no authority, and neither does this. The
/// host does its work through the commands it already has, authorized
/// as they already are; what happens here is only the bookkeeping that
/// lets a restart tell "about to act" from "acted" — and the fence
/// that keeps a replaced host from closing its successor's work.
pub struct AcknowledgeIntegratorAttentionUseCase {
    bindings: Arc<dyn IntegratorBindingPort>,
    deliveries: Arc<dyn HostDeliveryLedgerPort>,
    clock: Arc<dyn ClockPort>,
}

impl std::fmt::Debug for AcknowledgeIntegratorAttentionUseCase {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("AcknowledgeIntegratorAttentionUseCase")
            .finish_non_exhaustive()
    }
}

impl AcknowledgeIntegratorAttentionUseCase {
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
        name = "acknowledge_integrator_attention",
        skip_all,
        fields(delivery_id = %input.delivery_id)
    )]
    pub async fn execute(
        &self,
        input: AcknowledgeIntegratorAttentionInput,
    ) -> Result<IntegratorAttentionAcknowledged, DomainError> {
        self.still_this_host(&input).await?;
        let now = self.clock.now();
        match input.outcome {
            IntegratorAcknowledgement::Intent {
                action,
                note,
                evidence,
            } => {
                // Received, not processed: the host has the item and
                // has said what it means to do with it. The lease stays
                // with it, because it is still the one holding the work.
                let observation = HostDeliveryObservation::new(
                    HostDeliveryObservationKind::Received,
                    now,
                    evidence,
                    note,
                );
                let outcome = self
                    .deliveries
                    .acknowledge(&input.lease, &observation, now)
                    .await?;
                Ok(IntegratorAttentionAcknowledged::Intent {
                    outcome,
                    action: Box::new(action),
                })
            }
            IntegratorAcknowledgement::Processed { action } => {
                let outcome = self
                    .deliveries
                    .mark_processed(
                        &input.delivery_id,
                        &input.incarnation,
                        Some(input.fence),
                        &action,
                        now,
                    )
                    .await?;
                Ok(IntegratorAttentionAcknowledged::Processed(outcome))
            }
            IntegratorAcknowledgement::Failed { reason } => {
                let outcome = self
                    .deliveries
                    .mark_failed(&input.lease, &reason, now)
                    .await?;
                Ok(IntegratorAttentionAcknowledged::Failed(outcome))
            }
        }
    }

    /// Refuse a host that is no longer the one bound to this scope.
    ///
    /// Checked here as well as in the ledger, and on purpose: the
    /// ledger's fence guards the write, and this guards the call, so a
    /// stale host is told what happened to it rather than finding out
    /// through a lease it does not own.
    async fn still_this_host(
        &self,
        input: &AcknowledgeIntegratorAttentionInput,
    ) -> Result<(), DomainError> {
        let bindings = self.bindings.list(None).await?;
        let Some(binding) = bindings
            .into_iter()
            .find(|binding| binding.id() == &input.binding_id)
        else {
            return Err(DomainError::NotFound {
                what: "integrator_binding",
            });
        };
        if !binding.is_live() || !binding.admits(&input.incarnation, input.fence) {
            return Err(DomainError::Conflict {
                what: "integrator_fence",
            });
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests;
