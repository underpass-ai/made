use super::ceremony_store::{decode, encode};
use super::council_store::append;
use super::SqliteCouncilStore;
use crate::council_data_snapshot::CouncilDataSnapshot;
use crate::engine::{Key, ReadTx, Table, WriteTx};
use made_core::entities::{CouncilJournalEvent, CouncilJournalRecord, CouncilSnapshotProvenance};
use made_core::error::DomainError;
use made_core::value_objects::AuthorizationEvidence;
use serde::{de::DeserializeOwned, Serialize};

/// Offline migration boundary; an import commits all rows and one provenance
/// record atomically. It never synthesizes historical registry or phase events.
#[derive(Debug, Clone)]
pub struct SqliteCouncilSnapshot {
    store: SqliteCouncilStore,
}
impl SqliteCouncilSnapshot {
    #[must_use]
    pub fn new(store: SqliteCouncilStore) -> Self {
        Self { store }
    }
    pub async fn export(
        &self,
        provenance: CouncilSnapshotProvenance,
    ) -> Result<CouncilDataSnapshot, DomainError> {
        self.store
            .blocking(move |engine| {
                let tx = engine.begin_read()?;
                let snapshot = read(tx.as_ref(), provenance)?;
                snapshot.encode()?;
                Ok(snapshot)
            })
            .await
    }
    pub async fn import(
        &self,
        snapshot: CouncilDataSnapshot,
    ) -> Result<CouncilJournalRecord, DomainError> {
        self.import_authorized(snapshot, None).await
    }
    pub async fn import_authorized(
        &self,
        snapshot: CouncilDataSnapshot,
        authorization: Option<AuthorizationEvidence>,
    ) -> Result<CouncilJournalRecord, DomainError> {
        let snapshot = snapshot.canonicalized();
        let bytes = snapshot.encode()?;
        self.store
            .blocking(move |engine| {
                let mut tx = engine.begin_write()?;
                let key = format!(
                    "council_snapshot_import/{}",
                    snapshot.provenance.source().as_str()
                );
                if let Some(prior) = tx.get(Table::Meta, Key::Str(&key))? {
                    let (original, record): (CouncilDataSnapshot, CouncilJournalRecord) =
                        decode(&prior, "read council import")?;
                    if original != snapshot {
                        return Err(conflict());
                    }
                    return Ok(record);
                }
                let existing = read(tx.as_ref(), snapshot.provenance.clone())?;
                if !existing.is_empty() && existing != snapshot {
                    return Err(conflict());
                }
                write(tx.as_mut(), &snapshot)?;
                let record = append(
                    tx.as_mut(),
                    CouncilJournalEvent::SnapshotImported(snapshot.provenance.clone()),
                    authorization,
                )?;
                let original: CouncilDataSnapshot = decode(&bytes, "seal council import")?;
                tx.insert(
                    Table::Meta,
                    Key::Str(&key),
                    &encode(&(original, &record), "record council import")?,
                )?;
                tx.commit()?;
                Ok(record)
            })
            .await
    }
}
fn conflict() -> DomainError {
    DomainError::InvariantViolated {
        reason: "council snapshot conflicts with existing data or provenance",
    }
}
fn list<T: DeserializeOwned>(tx: &dyn ReadTx, table: Table) -> Result<Vec<T>, DomainError> {
    tx.scan_str(table)?
        .into_iter()
        .map(|(_, bytes)| decode(&bytes, "export council snapshot"))
        .collect()
}
fn read(
    tx: &dyn ReadTx,
    provenance: CouncilSnapshotProvenance,
) -> Result<CouncilDataSnapshot, DomainError> {
    Ok(CouncilDataSnapshot {
        schema_version: 1,
        provenance,
        councils: list(tx, Table::Councils)?,
        agents: list(tx, Table::CouncilAgents)?,
        contracts: list(tx, Table::CouncilContracts)?,
        deliberations: list(tx, Table::CouncilDeliberations)?,
        statistics: tx
            .get(Table::CouncilStatistics, Key::Str("totals"))?
            .map(|bytes| decode(&bytes, "export statistics"))
            .transpose()?
            .unwrap_or_default(),
    })
}
fn put(
    tx: &mut dyn WriteTx,
    table: Table,
    key: &str,
    value: &impl Serialize,
) -> Result<(), DomainError> {
    tx.insert(
        table,
        Key::Str(key),
        &encode(value, "import council snapshot")?,
    )
}
fn write(tx: &mut dyn WriteTx, snapshot: &CouncilDataSnapshot) -> Result<(), DomainError> {
    for council in &snapshot.councils {
        put(tx, Table::Councils, council.specialty().as_str(), council)?;
    }
    for agent in &snapshot.agents {
        put(tx, Table::CouncilAgents, agent.id.as_str(), agent)?;
    }
    for contract in &snapshot.contracts {
        put(
            tx,
            Table::CouncilContracts,
            contract.contract_id().as_str(),
            contract,
        )?;
    }
    for deliberation in &snapshot.deliberations {
        put(
            tx,
            Table::CouncilDeliberations,
            deliberation.task_id().as_str(),
            deliberation,
        )?;
    }
    put(tx, Table::CouncilStatistics, "totals", &snapshot.statistics)
}
