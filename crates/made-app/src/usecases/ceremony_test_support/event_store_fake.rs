use std::collections::BTreeMap;

use made_core::entities::{AuditFact, AuditRecord, CeremonyInstance};
use made_core::value_objects::{AuditSequence, CeremonyId, StreamVersion};
use tokio::sync::RwLock;

/// An event store and snapshot store in one, under one lock.
///
/// Refuses what the real ports refuse — a stale expectation, a
/// duplicate event id, an empty batch — so a use case cannot pass here
/// and fail against SQLite. `facts` keeps every fact appended through
/// the port, in order, so a test can read what a use case sealed
/// without decoding records.
#[derive(Debug, Default)]
pub(in crate::usecases) struct EventStoreFake {
    pub(super) streams: RwLock<BTreeMap<CeremonyId, Vec<AuditRecord>>>,
    pub(super) log: RwLock<Vec<(CeremonyId, AuditSequence)>>,
    pub(super) snapshots: RwLock<BTreeMap<CeremonyId, BTreeMap<StreamVersion, CeremonyInstance>>>,
    pub(super) facts: RwLock<Vec<AuditFact>>,
}
