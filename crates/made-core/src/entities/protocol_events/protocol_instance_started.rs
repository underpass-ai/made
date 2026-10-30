use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

use crate::value_objects::{
    BudgetAccountId, ProtocolContext, ProtocolDeadline, ProtocolDefinitionDigest, ProtocolId,
    ProtocolLineage, ProtocolName, ProtocolSuccession, ProtocolVersion, StateDeadline, StateId,
    StepId,
};

/// A protocol was opened.
///
/// Carries what `ProtocolInstance::start` and `start_bound` take and
/// everything they derive from the definition — the initial state and
/// the steps that get a pending record — so a fold can open the same
/// instance without the definition in hand. The ids repeat the
/// envelope's on purpose: the payload is meant to stand alone.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProtocolInstanceStarted {
    pub protocol_id: ProtocolId,
    pub definition_name: ProtocolName,
    pub definition_version: ProtocolVersion,
    pub initial_state: StateId,
    /// Every step the definition declares, each opened as pending.
    pub step_ids: BTreeSet<StepId>,
    pub context: ProtocolContext,
    /// The published definition's digest when the protocol was started
    /// from one; absent for a definition supplied for the run.
    pub bound_definition: Option<ProtocolDefinitionDigest>,
    /// Durable parent identity for a spawned protocol. Never sourced from context.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lineage: Option<ProtocolLineage>,
    /// Which protocol this one succeeds, sealed in the predecessor
    /// before this opening was appended. Treated exactly as `lineage`:
    /// durable provenance, never read out of context, and skipped when
    /// absent so openings that predate successions keep their bytes.
    ///
    /// Boxed because an opening is already the widest fact a stream
    /// holds and every other event of the enum pays for its size.
    /// Nothing on the wire changes: a box serializes as what it holds.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub succession: Option<Box<ProtocolSuccession>>,
    /// Shared root ledger for this protocol tree. Children inherit this exact account.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub budget_account_id: Option<BudgetAccountId>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub protocol_deadline: Option<ProtocolDeadline>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub state_deadline: Option<StateDeadline>,
    #[serde(with = "time::serde::rfc3339")]
    pub created_at: OffsetDateTime,
}
