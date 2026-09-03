use async_trait::async_trait;

use crate::error::DomainError;
use crate::ports::MemoryWriteOutcome;
use crate::value_objects::{MemoryCapabilities, MemoryScope, MemoryWrite};

/// Writing what a working session decided into memory that outlives it.
///
/// The engine already keeps an audit journal, and this is not that. A
/// journal proves what happened in one session; memory is what a later
/// session can navigate. One is evidence, the other is experience.
///
/// A write carries entries **and the reasons between them**, because
/// the reasons are not decoration on the entries — they are the part a
/// later session follows. What was decided can be listed; how one
/// thing led to another can only be walked.
#[async_trait]
pub trait MemoryWriterPort: Send + Sync {
    /// Record a write about `scope`.
    ///
    /// `idempotency_key` names the write, not the moment: the same key
    /// twice is the same write twice, whatever the clock says.
    ///
    /// A backend that does not keep reasons still accepts a write that
    /// carries them, and keeps what it can. Refusing would make a
    /// caller choose between explaining itself and being stored, and
    /// the honest place to learn what survives is the capabilities.
    async fn remember(
        &self,
        scope: &MemoryScope,
        write: MemoryWrite,
        idempotency_key: &str,
    ) -> Result<MemoryWriteOutcome, DomainError>;

    /// What this backend can do. A caller may ask before it acts, and
    /// the conformance suite checks the answer against behaviour: a
    /// backend that claims to remember and then does not is worse than
    /// one that claims nothing.
    fn capabilities(&self) -> MemoryCapabilities;
}
