use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

use crate::value_objects::{CeremonyId, EventId, EvidenceReference, StepId};

use super::{
    AttentionKind, AttentionReason, AttentionSummary, HostDeliveryId, HostDeliveryItem,
    IntegratorBindingId, IntegratorFence,
};

/// What an activated host is told, and the whole of it.
///
/// The envelope crosses into a process the engine does not control, so
/// it carries identity, cause and references — never a secret, never
/// the engine's reasoning, and never anything an adapter could mistake
/// for an instruction to run. What the host does next it does through
/// the engine's own commands, under their own authorization.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HostActivationEnvelope {
    delivery_id: HostDeliveryId,
    binding_id: IntegratorBindingId,
    fence: IntegratorFence,
    item: HostDeliveryItem,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    attention: Option<AttentionSummary>,
    ceremony_id: CeremonyId,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    step_id: Option<StepId>,
    kind: AttentionKind,
    reason: AttentionReason,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    evidence: Vec<EvidenceReference>,
    #[serde(with = "time::serde::rfc3339")]
    emitted_at: OffsetDateTime,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    correlation_id: Option<EventId>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    causation_id: Option<EventId>,
}

impl HostActivationEnvelope {
    #[must_use]
    pub const fn new(
        delivery_id: HostDeliveryId,
        binding_id: IntegratorBindingId,
        fence: IntegratorFence,
        item: HostDeliveryItem,
        ceremony_id: CeremonyId,
        kind: AttentionKind,
        reason: AttentionReason,
        emitted_at: OffsetDateTime,
    ) -> Self {
        Self {
            delivery_id,
            binding_id,
            fence,
            item,
            attention: None,
            ceremony_id,
            step_id: None,
            kind,
            reason,
            evidence: Vec::new(),
            emitted_at,
            correlation_id: None,
            causation_id: None,
        }
    }

    /// The projected attention behind this hand-off, summarised.
    #[must_use]
    pub fn about(mut self, attention: AttentionSummary) -> Self {
        self.attention = Some(attention);
        self
    }

    /// The step the hand-off is about, when it is about one.
    #[must_use]
    pub fn at_step(mut self, step_id: StepId) -> Self {
        self.step_id = Some(step_id);
        self
    }

    /// References a host may fetch under its own authorization.
    #[must_use]
    pub fn with_evidence(mut self, evidence: impl IntoIterator<Item = EvidenceReference>) -> Self {
        self.evidence = evidence.into_iter().collect();
        self
    }

    /// The trail this hand-off belongs to.
    #[must_use]
    pub fn caused_by(
        mut self,
        correlation_id: Option<EventId>,
        causation_id: Option<EventId>,
    ) -> Self {
        self.correlation_id = correlation_id;
        self.causation_id = causation_id;
        self
    }

    #[must_use]
    pub const fn delivery_id(&self) -> &HostDeliveryId {
        &self.delivery_id
    }

    #[must_use]
    pub const fn binding_id(&self) -> &IntegratorBindingId {
        &self.binding_id
    }

    #[must_use]
    pub const fn fence(&self) -> IntegratorFence {
        self.fence
    }

    #[must_use]
    pub const fn item(&self) -> &HostDeliveryItem {
        &self.item
    }

    #[must_use]
    pub const fn attention(&self) -> Option<&AttentionSummary> {
        self.attention.as_ref()
    }

    #[must_use]
    pub const fn ceremony_id(&self) -> &CeremonyId {
        &self.ceremony_id
    }

    #[must_use]
    pub const fn step_id(&self) -> Option<&StepId> {
        self.step_id.as_ref()
    }

    #[must_use]
    pub const fn kind(&self) -> AttentionKind {
        self.kind
    }

    #[must_use]
    pub const fn reason(&self) -> &AttentionReason {
        &self.reason
    }

    #[must_use]
    pub fn evidence(&self) -> &[EvidenceReference] {
        &self.evidence
    }

    #[must_use]
    pub const fn emitted_at(&self) -> OffsetDateTime {
        self.emitted_at
    }

    #[must_use]
    pub const fn correlation_id(&self) -> Option<&EventId> {
        self.correlation_id.as_ref()
    }

    #[must_use]
    pub const fn causation_id(&self) -> Option<&EventId> {
        self.causation_id.as_ref()
    }
}
