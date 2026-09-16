use serde::{Deserialize, Serialize};

use crate::value_objects::CeremonyGuardApproval;

/// A human guard was let through.
///
/// The approval as the aggregate keeps it. The context entry marking
/// the guard approved is derived from the guard's name when folded.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HumanApprovalRecorded {
    pub approval: CeremonyGuardApproval,
}
