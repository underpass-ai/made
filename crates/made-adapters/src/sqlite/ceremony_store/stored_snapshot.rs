use made_core::entities::CeremonyInstance;
use made_core::value_objects::StreamVersion;
use serde::{Deserialize, Serialize};

/// A ceremony's folded state at one version of its stream.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(super) struct StoredSnapshot {
    pub(super) version: StreamVersion,
    pub(super) instance: CeremonyInstance,
}
