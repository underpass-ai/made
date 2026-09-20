//! One thing an integrator is woken for.

use made_core::value_objects::{
    AgenticSystemExecutionId, AttentionEventId, AttentionKind, AttentionReason, AttentionSummary,
    CeremonyId, EventId, EvidenceReference, GlobalPosition, StepId,
};
use serde::Serialize;
use time::OffsetDateTime;

use super::{EventRef, ResultAcceptance};

/// A typed projection of one record of the global feed, shaped as
/// something a host can act on.
///
/// This is not a journal event and never becomes one. The ceremony
/// decided what it decided; attention is a reading of those decisions
/// for one integrator, and two integrators bound to the same ceremony
/// derive the same events independently without either of them
/// writing anything. That is what lets the loop be rebuilt from the
/// stream after a restart instead of being remembered.
///
/// The identity is derived from the ceremony, the source record and
/// the kind, so re-deriving the same record yields the same id and
/// the delivery ledger deduplicates it. Replaying the feed cannot
/// wake a host twice for the same news.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct AttentionEvent {
    id: AttentionEventId,
    kind: AttentionKind,
    ceremony_id: CeremonyId,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    system_execution_id: Option<AgenticSystemExecutionId>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    step_id: Option<StepId>,
    source: EventRef,
    position: GlobalPosition,
    #[serde(with = "time::serde::rfc3339")]
    occurred_at: OffsetDateTime,
    reason: AttentionReason,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    evidence: Vec<EvidenceReference>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    correlation_id: Option<EventId>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    causation_id: Option<EventId>,
    acceptance: ResultAcceptance,
}

impl AttentionEvent {
    #[must_use]
    pub const fn new(
        id: AttentionEventId,
        kind: AttentionKind,
        ceremony_id: CeremonyId,
        source: EventRef,
        position: GlobalPosition,
        occurred_at: OffsetDateTime,
        reason: AttentionReason,
        acceptance: ResultAcceptance,
    ) -> Self {
        Self {
            id,
            kind,
            ceremony_id,
            system_execution_id: None,
            step_id: None,
            source,
            position,
            occurred_at,
            reason,
            evidence: Vec::new(),
            correlation_id: None,
            causation_id: None,
            acceptance,
        }
    }

    /// Name the run this ceremony belongs to, when it belongs to one.
    #[must_use]
    pub fn within_execution(mut self, execution: Option<AgenticSystemExecutionId>) -> Self {
        self.system_execution_id = execution;
        self
    }

    /// Name the step the news is about, when it is about one.
    #[must_use]
    pub fn about_step(mut self, step: Option<StepId>) -> Self {
        self.step_id = step;
        self
    }

    /// Attach what a host can read to decide what to do.
    #[must_use]
    pub fn with_evidence(mut self, evidence: Vec<EvidenceReference>) -> Self {
        self.evidence = evidence;
        self
    }

    /// Carry the causal chain of the record this was derived from, so a
    /// host's own work stays correlated with the ceremony's.
    #[must_use]
    pub fn caused_by(mut self, correlation: Option<EventId>, causation: Option<EventId>) -> Self {
        self.correlation_id = correlation;
        self.causation_id = causation;
        self
    }

    #[must_use]
    pub const fn id(&self) -> &AttentionEventId {
        &self.id
    }

    #[must_use]
    pub const fn kind(&self) -> AttentionKind {
        self.kind
    }

    #[must_use]
    pub const fn ceremony_id(&self) -> &CeremonyId {
        &self.ceremony_id
    }

    #[must_use]
    pub const fn system_execution_id(&self) -> Option<&AgenticSystemExecutionId> {
        self.system_execution_id.as_ref()
    }

    #[must_use]
    pub const fn step_id(&self) -> Option<&StepId> {
        self.step_id.as_ref()
    }

    #[must_use]
    pub const fn source(&self) -> &EventRef {
        &self.source
    }

    #[must_use]
    pub const fn position(&self) -> GlobalPosition {
        self.position
    }

    #[must_use]
    pub const fn occurred_at(&self) -> OffsetDateTime {
        self.occurred_at
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
    pub const fn correlation_id(&self) -> Option<&EventId> {
        self.correlation_id.as_ref()
    }

    #[must_use]
    pub const fn causation_id(&self) -> Option<&EventId> {
        self.causation_id.as_ref()
    }

    #[must_use]
    pub const fn acceptance(&self) -> ResultAcceptance {
        self.acceptance
    }

    /// What travels in an activation envelope: enough to know why the
    /// host was woken, and nothing a host has no business reading.
    #[must_use]
    pub fn summary(&self) -> AttentionSummary {
        AttentionSummary::new(
            self.id.clone(),
            self.kind,
            self.occurred_at,
            self.reason.clone(),
        )
    }
}
