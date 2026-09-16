//! [`CeremonySnapshotStorePort`] — the cache beside the stream.

use async_trait::async_trait;

use crate::error::DomainError;
use crate::ports::CeremonySnapshot;
use crate::value_objects::CeremonyId;

#[async_trait]
pub trait CeremonySnapshotStorePort: Send + Sync {
    /// Keep a snapshot, keyed by its stream and version. Saving the
    /// same version twice is a no-op: the fold is deterministic, so
    /// the second copy can only be the first one again.
    async fn save(&self, snapshot: CeremonySnapshot) -> Result<(), DomainError>;

    /// The snapshot at the highest version this stream has one for.
    async fn latest(&self, stream: &CeremonyId) -> Result<Option<CeremonySnapshot>, DomainError>;

    /// Drop every snapshot of a stream. The stream itself is untouched,
    /// and the next load folds it from the beginning.
    async fn forget(&self, stream: &CeremonyId) -> Result<(), DomainError>;
}
