//! [`AwaitIntegratorAttentionUseCase`] — a bound host asks what it is
//! owed, and holds the line for a moment if the answer is nothing.

use std::sync::Arc;

use made_core::error::DomainError;
use made_core::ports::{
    CeremonyEventStorePort, CeremonyProgressNotifierPort, ClockPort, HostDeliveryFilter,
    HostDeliveryLedgerPort, HostDeliveryTargetFilter, IntegratorBindingPort,
};
use made_core::value_objects::{
    AttentionKind, CeremonyId, HostDeliveryItemKind, IntegratorBinding, IntegratorScope,
    MaxParallel,
};
use time::OffsetDateTime;

use crate::services::attention::{self, LoopProgress, LoopState};
use crate::services::SessionStream;

use super::{
    AttentionBatch, AttentionContext, AttentionDelivery, AttentionEndReason,
    AwaitIntegratorAttentionInput, CeremonyInstanceView, ResolveCeremonyDefinitionUseCase,
};

/// Hands a bound integrator its outstanding work.
///
/// The read path is where the loop recovers. Expiring what has run out
/// and leasing what is offerable happen here rather than in a sweeper,
/// so a deployment with no background worker still gets its work back
/// after a host dies holding a lease — which, for a loop whose whole
/// point is surviving restarts, is the ordinary case and not the
/// exception.
pub struct AwaitIntegratorAttentionUseCase {
    bindings: Arc<dyn IntegratorBindingPort>,
    deliveries: Arc<dyn HostDeliveryLedgerPort>,
    events: Arc<dyn CeremonyEventStorePort>,
    stream: Arc<SessionStream>,
    definitions: Arc<ResolveCeremonyDefinitionUseCase>,
    notifier: Arc<dyn CeremonyProgressNotifierPort>,
    clock: Arc<dyn ClockPort>,
}

impl std::fmt::Debug for AwaitIntegratorAttentionUseCase {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("AwaitIntegratorAttentionUseCase")
            .finish_non_exhaustive()
    }
}

impl AwaitIntegratorAttentionUseCase {
    #[must_use]
    pub fn new(
        bindings: Arc<dyn IntegratorBindingPort>,
        deliveries: Arc<dyn HostDeliveryLedgerPort>,
        events: Arc<dyn CeremonyEventStorePort>,
        stream: Arc<SessionStream>,
        definitions: Arc<ResolveCeremonyDefinitionUseCase>,
        notifier: Arc<dyn CeremonyProgressNotifierPort>,
        clock: Arc<dyn ClockPort>,
    ) -> Self {
        Self {
            bindings,
            deliveries,
            events,
            stream,
            definitions,
            notifier,
            clock,
        }
    }

    #[tracing::instrument(
        name = "await_integrator_attention",
        skip_all,
        fields(binding_id = %input.binding_id)
    )]
    pub async fn execute(
        &self,
        input: AwaitIntegratorAttentionInput,
    ) -> Result<AttentionBatch, DomainError> {
        let binding = self.admitted(&input).await?;
        // Subscribe before reading, so an append that lands during the
        // read cannot be missed by the wait that follows it.
        let mut progress = self.notifier.subscribe();

        let mut items = self.take(&binding, &input).await?;
        if items.is_empty() && input.bounded_wait().get() > 0 {
            let waited = tokio::time::timeout(
                std::time::Duration::from_millis(input.bounded_wait().get()),
                progress.wait(),
            )
            .await;
            if waited.is_ok() {
                items = self.take(&binding, &input).await?;
            }
        }

        let loop_state = self.loop_state(&binding, &items).await?;
        let end_reason = if !items.is_empty() {
            AttentionEndReason::Items
        } else if loop_state.is_halting() {
            AttentionEndReason::Terminal
        } else {
            AttentionEndReason::WaitElapsed
        };
        Ok(AttentionBatch::new(items, loop_state, end_reason))
    }

    /// The binding in force, or a refusal with nothing written.
    ///
    /// Checked against the scope rather than looked up by its own
    /// identifier: what a stale host needs to be told is that somebody
    /// else holds this scope now, and asking the scope is how that
    /// answer comes out right even when the old binding still exists.
    async fn admitted(
        &self,
        input: &AwaitIntegratorAttentionInput,
    ) -> Result<IntegratorBinding, DomainError> {
        let Some(binding) = self.bindings.current(&input.scope).await? else {
            return Err(DomainError::NotFound {
                what: "integrator_binding",
            });
        };
        if binding.id() != &input.binding_id
            || !binding.admits(&input.incarnation, input.fence)
            || !binding.is_live()
        {
            return Err(DomainError::Conflict {
                what: "integrator_fence",
            });
        }
        Ok(binding)
    }

    /// Everything offerable to this binding, as the host can read it.
    async fn take(
        &self,
        binding: &IntegratorBinding,
        input: &AwaitIntegratorAttentionInput,
    ) -> Result<Vec<AttentionDelivery>, DomainError> {
        let now = self.clock.now();
        self.deliveries.expire(now).await?;
        let filter = HostDeliveryFilter::to(HostDeliveryTargetFilter::any_of([
            binding.delivery_target()
        ])?)
        .of_kind(HostDeliveryItemKind::Attention);
        let leased = self
            .deliveries
            .lease(
                &filter,
                &input.incarnation,
                now,
                input.lease_duration,
                input.limit,
            )
            .await?;

        let mut items = Vec::with_capacity(leased.len());
        for delivery in leased {
            let (lease, record) = delivery.into_parts();
            let attention =
                attention::replay(self.events.as_ref(), record.item(), binding.role_id()).await?;
            let Some(attention) = attention else {
                // The offer stands for news this build can no longer
                // derive. Hand the lease back rather than hand a host
                // something invented to fill the gap.
                self.deliveries.release(&lease, now).await?;
                continue;
            };
            let context = self.context_of(attention.ceremony_id(), now).await?;
            let leased_until = lease.leased_until();
            items.push(AttentionDelivery::new(
                lease,
                leased_until,
                attention,
                context,
            ));
        }
        Ok(items)
    }

    /// What the ceremony looks like right now, in five fields.
    async fn context_of(
        &self,
        ceremony_id: &CeremonyId,
        now: OffsetDateTime,
    ) -> Result<AttentionContext, DomainError> {
        let session = self.stream.load(ceremony_id).await?;
        let definition = self.definitions.execute(&session.instance).await?;
        let view = CeremonyInstanceView::project_at(
            &session.instance,
            &definition,
            now,
            MaxParallel::SERVER_MAX,
        )?;
        Ok(AttentionContext::of(ceremony_id.clone(), &view))
    }

    /// Where the loop stands, read from the ceremony and the ledger.
    ///
    /// A batch that carries items reports on the ceremony they came
    /// from; an empty one on the scope's own ceremony when it has a
    /// single one. A scope with several and nothing to hand over has no
    /// one ceremony to speak for, so it reports as awaiting results —
    /// which is what it is.
    async fn loop_state(
        &self,
        binding: &IntegratorBinding,
        items: &[AttentionDelivery],
    ) -> Result<LoopState, DomainError> {
        let Some(ceremony_id) = items
            .first()
            .map(|item| item.attention().ceremony_id().clone())
            .or_else(|| match binding.scope() {
                IntegratorScope::Ceremony { ceremony_id } => Some(ceremony_id.clone()),
                IntegratorScope::SystemExecution { .. } => None,
            })
        else {
            return Ok(LoopState::AwaitingResults);
        };
        let now = self.clock.now();
        let session = self.stream.load(&ceremony_id).await?;
        let definition = self.definitions.execute(&session.instance).await?;
        let view = CeremonyInstanceView::project_at(
            &session.instance,
            &definition,
            now,
            MaxParallel::SERVER_MAX,
        )?;
        let progress = if items.iter().any(is_blocking) {
            LoopProgress::Blocked
        } else if items.is_empty() {
            LoopProgress::Idle
        } else {
            LoopProgress::AwaitingResults
        };
        Ok(LoopState::derive(&view, progress, now))
    }
}

fn is_blocking(item: &AttentionDelivery) -> bool {
    item.attention().kind() == AttentionKind::Blocked
}

#[cfg(test)]
mod tests;
