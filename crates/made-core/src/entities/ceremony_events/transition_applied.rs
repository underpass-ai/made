use serde::{Deserialize, Serialize};

use crate::value_objects::CeremonyTransitionRecord;

/// The ceremony moved from one state to another.
///
/// The record exactly as the aggregate keeps it — trigger, both
/// states, who applied it and when — so a fold pushes it as is and
/// takes the current state from it. Whether the destination is
/// terminal is not repeated here: the [`super::CeremonyCompleted`]
/// event sealed in the same commit says so.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TransitionApplied {
    pub transition: CeremonyTransitionRecord,
}
