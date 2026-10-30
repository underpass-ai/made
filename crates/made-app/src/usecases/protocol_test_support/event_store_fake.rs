use std::collections::BTreeMap;

use made_core::entities::{AuditFact, AuditRecord, ProtocolInstance};
use made_core::value_objects::{AuditSequence, ProtocolId, StreamVersion};
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
    pub(super) streams: RwLock<BTreeMap<ProtocolId, Vec<AuditRecord>>>,
    pub(super) log: RwLock<Vec<(ProtocolId, AuditSequence)>>,
    pub(super) snapshots: RwLock<BTreeMap<ProtocolId, BTreeMap<StreamVersion, ProtocolInstance>>>,
    pub(super) facts: RwLock<Vec<AuditFact>>,
}
