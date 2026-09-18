use made_core::entities::CeremonyInstance;
use made_core::value_objects::StreamVersion;
use serde::{Deserialize, Serialize};

/// Storage boundary only. The v2 field name deliberately differs from v1 so
/// an older binary rejects it instead of ignoring newly added domain state.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged, deny_unknown_fields)]
pub(super) enum StoredSnapshotWire {
    Current {
        snapshot_schema_version: u32,
        version: StreamVersion,
        folded_instance: CeremonyInstance,
    },
    Legacy {
        version: StreamVersion,
        instance: CeremonyInstance,
    },
}
