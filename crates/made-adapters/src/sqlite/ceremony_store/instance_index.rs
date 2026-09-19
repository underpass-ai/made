use std::collections::BTreeSet;

use async_trait::async_trait;
use made_core::error::DomainError;
use made_core::ports::{CeremonyInstanceIdPage, CeremonyInstanceIndexPort};
use made_core::value_objects::{CeremonyId, CeremonyIdPrefix, CeremonyInstancePageLimit};

use crate::engine::{Key, Table};
use crate::sqlite::keys::ceremony_of;

use super::SqliteCeremonyStore;

const BACKFILL_MARKER: &str = "ceremony_stream_index_backfilled_v1";

impl SqliteCeremonyStore {
    /// Build the durable stream identity index once for stores created before
    /// the index existed. The transaction makes a crash either keep the old
    /// state or install the complete index and marker together.
    pub(super) fn backfill_stream_index(&self) -> Result<(), DomainError> {
        let mut tx = self.engine.begin_write()?;
        if tx.get(Table::Meta, Key::Str(BACKFILL_MARKER))?.is_some() {
            return Ok(());
        }
        let mut ids = BTreeSet::new();
        for (key, _) in tx.scan_bytes(Table::Events)? {
            let bytes = ceremony_of(&key).ok_or(DomainError::InvariantViolated {
                reason: "sqlite: an events-table key is too short to index its ceremony",
            })?;
            ids.insert(CeremonyId::new(String::from_utf8_lossy(bytes))?);
        }
        for id in ids {
            tx.insert(Table::StreamIndex, Key::Str(id.as_str()), &[])?;
        }
        tx.insert(Table::Meta, Key::Str(BACKFILL_MARKER), &[])?;
        tx.commit()
    }
}

#[async_trait]
impl CeremonyInstanceIndexPort for SqliteCeremonyStore {
    async fn ids_after(
        &self,
        after: Option<&CeremonyId>,
        id_prefix: Option<&CeremonyIdPrefix>,
        limit: CeremonyInstancePageLimit,
    ) -> Result<CeremonyInstanceIdPage, DomainError> {
        let after = after.cloned();
        let id_prefix = id_prefix.cloned();
        self.blocking("page ceremony stream index", move |engine| {
            let tx = engine.begin_read()?;
            let mut rows = tx.scan_str_page(
                Table::StreamIndex,
                after.as_ref().map(CeremonyId::as_str),
                id_prefix.as_ref().map(CeremonyIdPrefix::as_str),
                limit.value() + 1,
            )?;
            let has_more = rows.len() > limit.value();
            rows.truncate(limit.value());
            let ids = rows
                .into_iter()
                .map(|(id, _)| CeremonyId::new(id))
                .collect::<Result<_, _>>()?;
            Ok(CeremonyInstanceIdPage::new(ids, has_more))
        })
        .await
    }
}
