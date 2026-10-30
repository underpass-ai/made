use async_trait::async_trait;
use made_core::error::DomainError;
use made_core::ports::{ProtocolInstanceIdPage, ProtocolInstanceIndexPort};
use made_core::value_objects::{ProtocolId, ProtocolIdPrefix, ProtocolInstancePageLimit};

use crate::engine::{Key, Table};
use crate::sqlite::keys::protocol_of;

use super::SqliteProtocolStore;

const BACKFILL_MARKER: &str = "protocol_stream_index_backfilled_v1";
const BACKFILL_KEY_PAGE: usize = 512;

impl SqliteProtocolStore {
    /// Build the durable stream identity index once for stores created before
    /// the index existed. The transaction makes a crash either keep the old
    /// state or install the complete index and marker together.
    pub(super) fn backfill_stream_index(&self) -> Result<(), DomainError> {
        let mut tx = self.engine.begin_write()?;
        if tx.get(Table::Meta, Key::Str(BACKFILL_MARKER))?.is_some() {
            return Ok(());
        }
        let mut after = None;
        let mut previous_id = None;
        loop {
            let keys =
                tx.scan_byte_keys_page(Table::Events, after.as_deref(), BACKFILL_KEY_PAGE)?;
            if keys.is_empty() {
                break;
            }
            for key in &keys {
                let bytes = protocol_of(key).ok_or(DomainError::InvariantViolated {
                    reason: "sqlite: an events-table key is too short to index its protocol",
                })?;
                let raw =
                    std::str::from_utf8(bytes).map_err(|_| DomainError::InvalidCharacters {
                        field: "protocol_id",
                    })?;
                let id = ProtocolId::new(raw)?;
                if id.as_str().as_bytes() != bytes {
                    return Err(DomainError::InvalidCharacters {
                        field: "protocol_id",
                    });
                }
                if previous_id.as_ref() != Some(&id) {
                    tx.insert(Table::StreamIndex, Key::Str(id.as_str()), &[])?;
                    previous_id = Some(id);
                }
            }
            after = keys.last().cloned();
        }
        tx.insert(Table::Meta, Key::Str(BACKFILL_MARKER), &[])?;
        tx.commit()
    }
}

#[async_trait]
impl ProtocolInstanceIndexPort for SqliteProtocolStore {
    async fn ids_after(
        &self,
        after: Option<&ProtocolId>,
        id_prefix: Option<&ProtocolIdPrefix>,
        limit: ProtocolInstancePageLimit,
    ) -> Result<ProtocolInstanceIdPage, DomainError> {
        let after = after.cloned();
        let id_prefix = id_prefix.cloned();
        self.blocking("page protocol stream index", move |engine| {
            let tx = engine.begin_read()?;
            let mut rows = tx.scan_str_page(
                Table::StreamIndex,
                after.as_ref().map(ProtocolId::as_str),
                id_prefix.as_ref().map(ProtocolIdPrefix::as_str),
                limit.value() + 1,
            )?;
            let has_more = rows.len() > limit.value();
            rows.truncate(limit.value());
            let ids = rows
                .into_iter()
                .map(|(id, _)| ProtocolId::new(id))
                .collect::<Result<_, _>>()?;
            Ok(ProtocolInstanceIdPage::new(ids, has_more))
        })
        .await
    }
}
