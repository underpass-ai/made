use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

use crate::value_objects::SessionRecollection;

/// A session read what earlier sessions in its scope decided.
///
/// Sealed in the same batch as the opening, immediately after it, and
/// only when something came back. A session with nothing to recall
/// appends nothing, so its stream is byte for byte the stream it would
/// have had before memory could be read at all — which is what lets
/// every store written until now still fold.
///
/// It is an event rather than a lookup the reader repeats because the
/// recollection is **what this session was told**, not what the scope
/// says now. A later reader asking memory again would get today's
/// answer to a question this session asked yesterday, and would judge
/// what the session did by knowledge it did not have.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MemoryRecalled {
    pub recollection: SessionRecollection,
    #[serde(with = "time::serde::rfc3339")]
    pub recalled_at: OffsetDateTime,
}
