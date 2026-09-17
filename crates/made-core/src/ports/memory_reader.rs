use async_trait::async_trait;

use crate::error::DomainError;
use crate::ports::MemoryRecollection;
use crate::value_objects::{MemoryCapabilities, MemoryEntryId, MemoryMoment, MemoryScope};

/// Reading what earlier sessions learned, and why.
///
/// Three ways of reading, because three different questions get asked:
/// what is known about this at all, what was known at a moment, and how
/// one thing here came from another. The second is not the first
/// filtered by date — it excludes what was learned later about earlier
/// events, which is the whole point of asking it.
///
/// Every method here has a consumer: `recall` is what a session reads
/// when it starts, and the other two are what the conformance suite
/// states as the contract (ADR-013). A method with neither is a shape
/// answering to the backend it was drawn from, which is why the one
/// that had neither is gone.
#[async_trait]
pub trait MemoryReaderPort: Send + Sync {
    /// Everything memory holds about `scope`.
    async fn recall(&self, scope: &MemoryScope) -> Result<MemoryRecollection, DomainError>;

    /// What was known about `scope` at `moment`.
    async fn as_known_at(
        &self,
        scope: &MemoryScope,
        moment: MemoryMoment,
    ) -> Result<MemoryRecollection, DomainError>;

    /// The chain of reasons leading from `from` back to `to`.
    ///
    /// The question the whole contract exists to answer, and the only
    /// one whose failure means the memory has stopped being worth
    /// keeping: everything else can be reconstructed by reading, and
    /// this cannot.
    ///
    /// It answers with the reasons and not with the prose — the edges
    /// on the path, in the order they connect. What each end says is
    /// what `recall` is for, and a backend that padded the chain with
    /// text would make two contracts out of one.
    ///
    /// An empty chain is a real answer: the two are not connected by
    /// anything anyone wrote down.
    async fn follow(
        &self,
        scope: &MemoryScope,
        from: &MemoryEntryId,
        to: &MemoryEntryId,
    ) -> Result<MemoryRecollection, DomainError>;

    fn capabilities(&self) -> MemoryCapabilities;
}
