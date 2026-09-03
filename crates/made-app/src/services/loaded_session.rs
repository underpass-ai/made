use made_core::entities::CeremonyInstance;
use made_core::value_objects::ExpectedRevision;

/// A ceremony instance paired with the revision it was read at.
#[derive(Debug)]
pub struct LoadedSession {
    pub instance: CeremonyInstance,
    pub expected: ExpectedRevision,
}
