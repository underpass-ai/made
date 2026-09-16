use crate::entities::CeremonyInstance;
use crate::value_objects::StreamVersion;

/// A ceremony's state as folded from its stream up to `version`.
///
/// A snapshot is a cache of the fold, never the truth: loading a
/// ceremony is the latest snapshot plus the events after its version,
/// and dropping every snapshot changes nothing but speed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CeremonySnapshot {
    pub version: StreamVersion,
    pub instance: CeremonyInstance,
}
