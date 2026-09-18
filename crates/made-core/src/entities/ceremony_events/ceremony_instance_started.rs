use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

use crate::value_objects::{
    CeremonyContext, CeremonyDeadline, CeremonyDefinitionDigest, CeremonyId, CeremonyLineage,
    CeremonyName, CeremonyVersion, StateDeadline, StateId, StepId,
};

/// A ceremony was opened.
///
/// Carries what `CeremonyInstance::start` and `start_bound` take and
/// everything they derive from the definition — the initial state and
/// the steps that get a pending record — so a fold can open the same
/// instance without the definition in hand. The ids repeat the
/// envelope's on purpose: the payload is meant to stand alone.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CeremonyInstanceStarted {
    pub ceremony_id: CeremonyId,
    pub definition_name: CeremonyName,
    pub definition_version: CeremonyVersion,
    pub initial_state: StateId,
    /// Every step the definition declares, each opened as pending.
    pub step_ids: BTreeSet<StepId>,
    pub context: CeremonyContext,
    /// The published definition's digest when the ceremony was started
    /// from one; absent for a definition supplied for the run.
    pub bound_definition: Option<CeremonyDefinitionDigest>,
    /// Durable parent identity for a spawned ceremony. Never sourced from context.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lineage: Option<CeremonyLineage>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ceremony_deadline: Option<CeremonyDeadline>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub state_deadline: Option<StateDeadline>,
    #[serde(with = "time::serde::rfc3339")]
    pub created_at: OffsetDateTime,
}
