//! The pre-stream tables, read-only.
//!
//! `protocol_instances` and `audit_journal` are no longer written by
//! anything (A7). They stay because they are the provenance of every
//! session the import brings forward: the snapshot the import carries
//! and the journal head it names both come from here, and an operator
//! can still open the file and read what the old engine recorded.

use async_trait::async_trait;
use made_core::entities::AuditRecord;
use made_core::error::DomainError;
use made_core::ports::{LegacyProtocolSnapshot, LegacyProtocolSnapshotSourcePort};
use made_core::value_objects::ProtocolId;

use crate::engine::{Key, ReadTx, Table};
use crate::sqlite::keys::{scope_range, scoped};
use crate::sqlite::StoredProtocol;

use super::{decode, SqliteProtocolStore};

#[async_trait]
impl LegacyProtocolSnapshotSourcePort for SqliteProtocolStore {
    async fn instances_without_a_stream(&self) -> Result<Vec<LegacyProtocolSnapshot>, DomainError> {
        self.blocking("read legacy instances", move |engine| {
            let tx = engine.begin_read()?;
            let mut stranded = Vec::new();
            // `scan_str` walks the primary key, which is the id, so the
            // order the port promises is the storage order rather than
            // a sort this adapter performs.
            for (id, value) in tx.scan_str(Table::Protocols)? {
                let protocol_id = ProtocolId::new(id)?;
                if tx
                    .get(Table::Events, Key::Bytes(&scoped(&protocol_id, 1)))?
                    .is_some()
                {
                    continue;
                }
                let stored: StoredProtocol = decode(&value, "decode protocol")?;
                stranded.push(LegacyProtocolSnapshot {
                    instance: stored.instance,
                    revision: stored.revision,
                    journal_head_hash: legacy_journal_head(tx.as_ref(), &protocol_id)?
                        .map(|record| record.record_hash()),
                });
            }
            Ok(stranded)
        })
        .await
    }
}

/// The last record the legacy journal holds for one session.
///
/// The whole scan rather than a reverse seek: the seam hands back a
/// range in key order and has no `last`, and a journal is a handful of
/// rows per session.
fn legacy_journal_head(
    tx: &dyn ReadTx,
    protocol_id: &ProtocolId,
) -> Result<Option<AuditRecord>, DomainError> {
    let (start, end) = scope_range(protocol_id);
    let Some((_, value)) = tx.scan_bytes_range(Table::Journal, &start, &end)?.pop() else {
        return Ok(None);
    };
    decode(&value, "decode audit record").map(Some)
}
