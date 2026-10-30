use made_core::entities::ProtocolInstance;
use made_core::value_objects::ProtocolRevision;
use serde::{Deserialize, Serialize};

/// A protocol's stored state and the revision that guards it.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(in crate::sqlite) struct StoredProtocol {
    pub(super) revision: ProtocolRevision,
    pub(super) instance: ProtocolInstance,
}
