use std::sync::atomic::AtomicBool;
use std::sync::Arc;

use made_core::entities::AuditFact;

use super::EventStoreFake;

/// An event store whose first append is overtaken by another writer.
///
/// The first append, whatever it expects, is refused as a conflict
/// with nothing written; every later one goes through to the store
/// underneath. What a retrying use case does with that one refusal —
/// reload, decide again, land on the second attempt — is what the
/// tests over this fake pin.
#[derive(Debug)]
pub(in crate::usecases) struct StoreThatConflictsOnce {
    pub(super) inner: Arc<EventStoreFake>,
    pub(super) conflicted: AtomicBool,
    pub(super) overtaking_fact: Option<AuditFact>,
}
