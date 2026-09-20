use serde::{Deserialize, Serialize};

use crate::value_objects::SuccessionPlan;

/// A handoff was sealed in the ceremony being handed off.
///
/// The first of the two appends a succession takes, and deliberately
/// the one in the predecessor: a crash after this leaves a ceremony
/// that says exactly what it intended, so the opening can be resumed
/// or verified. Sealing the successor first would leave a stream with
/// no recorded provenance, indistinguishable from an unrelated
/// instance that happens to hold that id.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SuccessorPlanned {
    pub plan: SuccessionPlan,
}
