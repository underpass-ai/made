use std::sync::atomic::AtomicBool;
use std::sync::Arc;

use made_core::entities::AuditFact;
use made_core::value_objects::AuditEventType;

use super::EventStoreFake;

/// An event store whose first matching append is overtaken by another writer.
///
/// The optional event type selects the append to refuse. The supplied facts
/// land before the conflict is returned; every later append goes through.
/// Tests can therefore pin a retry against changed durable state without sleeps.
#[derive(Debug)]
pub(in crate::usecases) struct StoreThatConflictsOnce {
    pub(super) inner: Arc<EventStoreFake>,
    pub(super) conflicted: AtomicBool,
    pub(super) overtaking_facts: Vec<AuditFact>,
    pub(super) conflict_on: Option<AuditEventType>,
}
