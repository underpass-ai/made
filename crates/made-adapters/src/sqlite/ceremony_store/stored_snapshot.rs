use made_core::entities::CeremonyInstance;
use made_core::value_objects::StreamVersion;
use serde::{Deserialize, Serialize};

use super::stored_snapshot_wire::StoredSnapshotWire;

/// A ceremony's folded state at one version of its stream.
///
/// New snapshots use an envelope that pre-lifecycle readers cannot decode.
/// Otherwise an old reader could ignore new aggregate fields and load only a
/// familiar tail event, bypassing a pause or another earlier control event.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(try_from = "StoredSnapshotWire", into = "StoredSnapshotWire")]
pub(super) struct StoredSnapshot {
    pub(super) version: StreamVersion,
    pub(super) instance: CeremonyInstance,
}

impl TryFrom<StoredSnapshotWire> for StoredSnapshot {
    type Error = &'static str;

    fn try_from(wire: StoredSnapshotWire) -> Result<Self, Self::Error> {
        match wire {
            StoredSnapshotWire::Legacy { version, instance }
            | StoredSnapshotWire::Current {
                snapshot_schema_version: 2,
                version,
                folded_instance: instance,
            } => Ok(Self { version, instance }),
            StoredSnapshotWire::Current { .. } => {
                Err("unsupported ceremony snapshot schema version")
            }
        }
    }
}

impl From<StoredSnapshot> for StoredSnapshotWire {
    fn from(snapshot: StoredSnapshot) -> Self {
        Self::Current {
            snapshot_schema_version: 2,
            version: snapshot.version,
            folded_instance: snapshot.instance,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn legacy() -> serde_json::Value {
        use made_core::value_objects::{CeremonyContext, CeremonyId};
        let events = CeremonyInstance::decide_start(
            CeremonyId::new("snapshot-compat").unwrap(),
            &super::super::legacy_store_fixture::definition(),
            CeremonyContext::empty(),
            None,
            time::OffsetDateTime::UNIX_EPOCH,
        )
        .unwrap();
        let instance = CeremonyInstance::rehydrate(&events).unwrap();
        serde_json::json!({"version": 1, "instance": instance})
    }

    #[test]
    fn reads_legacy_but_writes_only_the_versioned_envelope() {
        let snapshot: StoredSnapshot = serde_json::from_value(legacy()).unwrap();
        let current = serde_json::to_value(&snapshot).unwrap();
        assert_eq!(current["snapshot_schema_version"], 2);
        assert!(current.get("instance").is_none());
        assert!(current.get("folded_instance").is_some());
        let reopened: StoredSnapshot = serde_json::from_value(current).unwrap();
        assert_eq!(reopened.instance, snapshot.instance);
        assert_eq!(reopened.version, snapshot.version);
    }

    #[test]
    fn unknown_versions_and_mixed_envelopes_are_refused() {
        let snapshot: StoredSnapshot = serde_json::from_value(legacy()).unwrap();
        let mut current = serde_json::to_value(snapshot).unwrap();
        current["snapshot_schema_version"] = 3.into();
        assert!(serde_json::from_value::<StoredSnapshot>(current.clone()).is_err());
        current["instance"] = current["folded_instance"].clone();
        assert!(serde_json::from_value::<StoredSnapshot>(current).is_err());
    }
}
