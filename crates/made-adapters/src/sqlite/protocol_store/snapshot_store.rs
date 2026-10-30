//! [`ProtocolSnapshotStorePort`] over the storage seam.
//!
//! One table, `protocol_snapshots`, keyed by `(protocol_id, version)`
//! so the latest snapshot of a stream is the last row of its scope
//! range and forgetting a stream is removing that range.

use async_trait::async_trait;
use made_core::error::DomainError;
use made_core::ports::{ProtocolSnapshot, ProtocolSnapshotStorePort};
use made_core::value_objects::ProtocolId;

use crate::engine::{Key, Table};
use crate::sqlite::keys::{scope_range, scoped};

use super::stored_snapshot::StoredSnapshot;
use super::{decode, encode, SqliteProtocolStore};

#[async_trait]
impl ProtocolSnapshotStorePort for SqliteProtocolStore {
    async fn save(&self, snapshot: ProtocolSnapshot) -> Result<(), DomainError> {
        self.blocking("save snapshot", move |engine| {
            let key = scoped(snapshot.instance.id(), snapshot.version.value());
            let stored = StoredSnapshot {
                version: snapshot.version,
                instance: snapshot.instance,
            };
            let mut tx = engine.begin_write()?;
            // An insert is an upsert on the seam, which is what makes
            // saving the same version twice a no-op.
            tx.insert(
                Table::Snapshots,
                Key::Bytes(&key),
                &encode(&stored, "encode snapshot")?,
            )?;
            tx.commit()
        })
        .await
    }

    async fn latest(&self, stream: &ProtocolId) -> Result<Option<ProtocolSnapshot>, DomainError> {
        let stream = stream.clone();
        self.blocking("latest snapshot", move |engine| {
            let tx = engine.begin_read()?;
            let (start, end) = scope_range(&stream);
            tx.scan_bytes_range(Table::Snapshots, &start, &end)?
                .pop()
                .map(|(_, value)| {
                    let stored: StoredSnapshot = decode(&value, "decode snapshot")?;
                    Ok(ProtocolSnapshot {
                        version: stored.version,
                        instance: stored.instance,
                    })
                })
                .transpose()
        })
        .await
    }

    async fn forget(&self, stream: &ProtocolId) -> Result<(), DomainError> {
        let stream = stream.clone();
        self.blocking("forget snapshots", move |engine| {
            let mut tx = engine.begin_write()?;
            let (start, end) = scope_range(&stream);
            for (key, _) in tx.scan_bytes_range(Table::Snapshots, &start, &end)? {
                tx.remove(Table::Snapshots, Key::Bytes(&key))?;
            }
            tx.commit()
        })
        .await
    }
}
