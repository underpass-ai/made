//! [`AttentionRecovery`] — running the projection for whoever needs it
//! to have run.
//!
//! The projector walks the feed for one audience, and an audience is a
//! binding resolved against the stores. Three callers need that pair
//! and none of them holds both: the subscriber knows a ceremony, the
//! await path knows a binding it has already fenced, and the operator
//! read knows a binding identifier or nothing at all. Resolving in one
//! place is what keeps the three from drifting into three answers to
//! "which bindings does this concern".
//!
//! # Why the read path projects at all
//!
//! Being told after an append is a wake-up, and a wake-up can be
//! missed: the process dies between the append and the reaction, or it
//! was not running when the append happened. The journal and the
//! cursor are the authority, so every read that is about to look at
//! the ledger walks the feed first. A restart then recovers by being
//! asked an ordinary question rather than by a background sweeper
//! nobody deployed.

use std::sync::Arc;

use made_core::error::DomainError;
use made_core::ports::IntegratorBindingPort;
use made_core::value_objects::{
    CeremonyEventPageLimit, CeremonyId, IntegratorBinding, IntegratorBindingId, LoopLimits,
};

use super::{AttentionAudienceResolver, AttentionProjector, ProjectionRound};

/// Projects attention for the bindings a caller is about to read for.
pub struct AttentionRecovery {
    bindings: Arc<dyn IntegratorBindingPort>,
    resolver: Arc<AttentionAudienceResolver>,
    projector: Arc<AttentionProjector>,
    limit: CeremonyEventPageLimit,
}

impl std::fmt::Debug for AttentionRecovery {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("AttentionRecovery")
            .field("limit", &self.limit)
            .finish_non_exhaustive()
    }
}

impl AttentionRecovery {
    #[must_use]
    pub fn new(
        bindings: Arc<dyn IntegratorBindingPort>,
        resolver: Arc<AttentionAudienceResolver>,
        projector: Arc<AttentionProjector>,
    ) -> Self {
        Self {
            bindings,
            resolver,
            projector,
            limit: CeremonyEventPageLimit::DEFAULT,
        }
    }

    /// Walk the feed for one binding the caller already holds.
    ///
    /// The caller has checked the fence, so nothing here re-reads the
    /// binding: a round projected for a binding that has since been
    /// replaced still advances that binding's own cursor, which is the
    /// only thing it can affect.
    pub async fn for_binding(
        &self,
        binding: &IntegratorBinding,
    ) -> Result<ProjectionRound, DomainError> {
        let audience = self.resolver.resolve(binding).await?;
        self.projector.project(&audience, self.limit).await
    }

    /// What this binding's policy allows its loop.
    ///
    /// Asked of the resolver rather than of the binding, because a
    /// binding on a system execution inherits the composed system's
    /// policy and a ceremony-scoped one takes the defaults. The read
    /// path needs the same answer the projection round uses, and two
    /// places reading it separately is how the two come to disagree.
    pub async fn limits_for(&self, binding: &IntegratorBinding) -> Result<LoopLimits, DomainError> {
        Ok(self.resolver.resolve(binding).await?.policy().limits())
    }

    /// Walk the feed for every live binding this ceremony concerns.
    ///
    /// The scope is asked rather than matched: a binding on a system
    /// execution covers ceremonies that were opened after it was made,
    /// so "does this binding care about this ceremony" is a question
    /// only the resolved audience can answer.
    pub async fn for_ceremony(&self, ceremony_id: &CeremonyId) -> Result<(), DomainError> {
        for binding in self.live().await? {
            let audience = self.resolver.resolve(&binding).await?;
            if !audience.covers(ceremony_id) {
                continue;
            }
            self.projector.project(&audience, self.limit).await?;
        }
        Ok(())
    }

    /// Walk the feed for one named binding, or for every live one.
    ///
    /// An operator asking what the loop has been offered without
    /// naming a binding is asking about the deployment, and answering
    /// from a ledger that no round had filled would show an empty
    /// queue on a host that is behind.
    pub async fn for_bindings(
        &self,
        binding_id: Option<&IntegratorBindingId>,
    ) -> Result<(), DomainError> {
        for binding in self.live().await? {
            if binding_id.is_some_and(|wanted| wanted != binding.id()) {
                continue;
            }
            self.for_binding(&binding).await?;
        }
        Ok(())
    }

    /// The bindings still in force. A revoked one drives nothing, and
    /// projecting for it would offer work to a host that was replaced.
    async fn live(&self) -> Result<Vec<IntegratorBinding>, DomainError> {
        Ok(self
            .bindings
            .list(None)
            .await?
            .into_iter()
            .filter(IntegratorBinding::is_live)
            .collect())
    }
}
