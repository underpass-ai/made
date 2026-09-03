use made_core::entities::CeremonyInstance;
use made_core::value_objects::CeremonyRevision;
use serde::{Deserialize, Serialize};

/// A ceremony's stored state and the revision that guards it.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(in crate::redb) struct StoredCeremony {
    pub(super) revision: CeremonyRevision,
    pub(super) instance: CeremonyInstance,
}
