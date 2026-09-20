use made_core::value_objects::{SourceRecordRef, StepId};
use serde::{Deserialize, Serialize};

/// One step a handoff could carry, and the sealed record it would
/// carry it from.
///
/// A proposal, not a decision: what is actually carried is whatever
/// the caller names when it starts the successor. Offering the
/// resolved source here is what lets that call be made without reading
/// the predecessor's journal by hand.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProposedCarriedStep {
    pub step_id: StepId,
    pub source: SourceRecordRef,
}
