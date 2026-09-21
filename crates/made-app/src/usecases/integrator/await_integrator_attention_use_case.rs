//! [`AwaitIntegratorAttentionUseCase`] — a bound host asks what it is
//! owed, and holds the line for a moment if the answer is nothing.

use std::sync::Arc;

use made_core::entities::CeremonyDefinition;
use made_core::error::DomainError;
use made_core::ports::{
    CeremonyEventStorePort, CeremonyProgressNotifierPort, ClockPort, HostDeliveryFilter,
    HostDeliveryLedgerPort, HostDeliveryTargetFilter, IntegratorBindingPort,
};
use made_core::value_objects::{
    AttentionKind, CeremonyId, GlobalPosition, HostDeliveryItemKind, HostDeliveryStateKind,
    IntegratorBinding, IntegratorScope, LoopLimits, MaxParallel,
};
use time::OffsetDateTime;

use crate::services::attention::{
    self, AttentionRecovery, BindingDeliveries, LoopProgress, LoopRoundTally, LoopStall, LoopState,
    NoProgressDetector,
};
use crate::services::SessionStream;

use crate::usecases::{CeremonyInstanceView, ResolveCeremonyDefinitionUseCase};

use super::{
    AttentionBatch, AttentionContext, AttentionDelivery, AttentionEndReason,
    AwaitIntegratorAttentionInput,
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
    recovery: Option<Arc<AttentionRecovery>>,
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
            recovery: None,
        }
    }

    /// Project the feed before every read of the ledger.
    ///
    /// Optional because a host that composed the engine without the
    /// loop has nothing to project for, and the use case would then
    /// pay for a walk of the global feed on every call. Every
    /// composition that has a loop attaches one: without it the read
    /// only sees what a wake-up managed to enqueue, and a wake-up that
    /// arrived while the process was down never arrived at all.
    #[must_use]
    pub fn with_recovery(mut self, recovery: Arc<AttentionRecovery>) -> Self {
        self.recovery = Some(recovery);
        self
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

        let head = self.journal_head(&binding).await;
        let (_, stall) = self.observe(&binding, head).await?;
        let loop_state = self.loop_state(&binding, &items, stall).await?;
        let end_reason = if !items.is_empty() {
            AttentionEndReason::Items
        } else if loop_state.is_halting() {
            AttentionEndReason::Terminal
        } else {
            AttentionEndReason::WaitElapsed
        };
        Ok(AttentionBatch::new(items, loop_state, end_reason).at(head))
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
        self.recover(binding).await;
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
            // Loaded once and used twice: the derivation that needs
            // the definition, and the context the batch carries. Two
            // loads would also be two answers the moment something
            // was appended between them.
            let session = self.session_of(record.item().ceremony_id(), now).await?;
            let definition = session.as_ref().map(|session| &session.1);
            let attention = attention::replay(
                self.events.as_ref(),
                record.item(),
                binding.role_id(),
                definition,
            )
            .await?;
            let Some(attention) = attention else {
                // The offer stands for news this build can no longer
                // derive. Hand the lease back rather than hand a host
                // something invented to fill the gap.
                self.deliveries.release(&lease, now).await?;
                continue;
            };
            let Some((context, _)) = session else {
                self.deliveries.release(&lease, now).await?;
                continue;
            };
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

    /// Walk the feed for this binding before reading the ledger.
    ///
    /// A projection that cannot run is not a reason to refuse the
    /// read: the ledger still holds whatever earlier rounds offered,
    /// and answering with that is better than answering with an error
    /// about a walk the host never asked for.
    async fn recover(&self, binding: &IntegratorBinding) {
        let Some(recovery) = &self.recovery else {
            return;
        };
        if let Err(error) = recovery.for_binding(binding).await {
            tracing::warn!(
                binding_id = %binding.id(),
                %error,
                "attention projection failed before the read; the ledger answers as it stands"
            );
        }
    }

    /// What the ceremony looks like right now, and what it runs.
    ///
    /// `None` for a session this build cannot read: the caller hands
    /// the lease back rather than describing a ceremony it could not
    /// load.
    async fn session_of(
        &self,
        ceremony_id: &CeremonyId,
        now: OffsetDateTime,
    ) -> Result<Option<(AttentionContext, CeremonyDefinition)>, DomainError> {
        let Ok(session) = self.stream.load(ceremony_id).await else {
            return Ok(None);
        };
        let Ok(definition) = self.definitions.execute(&session.instance).await else {
            return Ok(None);
        };
        let view = CeremonyInstanceView::project_at(
            &session.instance,
            &definition,
            now,
            MaxParallel::SERVER_MAX,
        )?;
        Ok(Some((
            AttentionContext::of(ceremony_id.clone(), &view),
            definition,
        )))
    }

    /// Count this ask, and say whether the loop has stopped getting
    /// anywhere.
    ///
    /// The round is written down before it is judged, on the binding
    /// itself, so a process that restarts mid-loop comes back knowing
    /// how many times it has already been round instead of starting
    /// again from nothing. What counts as movement is the feed having
    /// been projected further for this binding, or one more of its
    /// deliveries having been closed — never a lease running out,
    /// which is about exclusion and not about progress.
    async fn observe(
        &self,
        binding: &IntegratorBinding,
        head: Option<GlobalPosition>,
    ) -> Result<(LoopRoundTally, Option<LoopStall>), DomainError> {
        let ledger = BindingDeliveries::new(self.deliveries.as_ref())
            .all(&binding.delivery_target())
            .await?;
        let closed = u32::try_from(
            ledger
                .iter()
                .filter(|record| record.state().kind() == HostDeliveryStateKind::Processed)
                .count(),
        )
        .unwrap_or(u32::MAX);
        let mark = binding.progress().observing(head, closed);
        self.bindings.record_progress(binding.id(), mark).await?;
        let rounds = LoopRoundTally::of(mark);
        let stall = NoProgressDetector::new(self.limits(binding).await).detect(rounds);
        if let Some(stall) = stall {
            tracing::info!(
                binding_id = %binding.id(),
                stall = stall.as_str(),
                rounds = rounds.rounds(),
                stuck = rounds.stuck(),
                journal_head = ?head,
                "the integrator loop stopped itself"
            );
        }
        Ok((rounds, stall))
    }

    /// How far this binding's projection has walked the feed.
    async fn journal_head(&self, binding: &IntegratorBinding) -> Option<GlobalPosition> {
        let recovery = self.recovery.as_ref()?;
        recovery.journal_head_for(binding).await.ok().flatten()
    }

    /// What this binding's policy allows the loop, or the defaults.
    async fn limits(&self, binding: &IntegratorBinding) -> LoopLimits {
        let Some(recovery) = &self.recovery else {
            return LoopLimits::default();
        };
        recovery
            .limits_for(binding)
            .await
            .unwrap_or_else(|_| LoopLimits::default())
    }

    /// Where the loop stands, read from the ceremony and the ledger.
    ///
    /// A batch that carries items reports on the ceremony they came
    /// from; an empty one on the scope's own ceremony when it has a
    /// single one. A scope with several and nothing to hand over has no
    /// one ceremony to speak for, so it reports as awaiting results —
    /// which is what it is, unless the loop has stopped itself.
    async fn loop_state(
        &self,
        binding: &IntegratorBinding,
        items: &[AttentionDelivery],
        stall: Option<LoopStall>,
    ) -> Result<LoopState, DomainError> {
        let Some(ceremony_id) = items
            .first()
            .map(|item| item.attention().ceremony_id().clone())
            .or_else(|| match binding.scope() {
                IntegratorScope::Ceremony { ceremony_id } => Some(ceremony_id.clone()),
                IntegratorScope::SystemExecution { .. } => None,
            })
        else {
            return Ok(if stall.is_some() {
                LoopState::Blocked
            } else {
                LoopState::AwaitingResults
            });
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
        let progress = if stall.is_some() || items.iter().any(is_blocking) {
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
