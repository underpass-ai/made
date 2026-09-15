use serde::{Deserialize, Serialize};

use crate::value_objects::CeremonyParticipantBinding;

/// Roles were seated for this ceremony.
///
/// One binding per role seated by the call, exactly as the aggregate
/// keeps them, ordered by role. A role seated again replaces its
/// earlier binding when folded, which is what `bind_participant` does.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ParticipantsBound {
    pub bindings: Vec<CeremonyParticipantBinding>,
}
