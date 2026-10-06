use crate::value_objects::{AuditRecordHash, StreamVersion};

/// Where a journal ends: the version of its last record, that record's
/// digest, and how many records lead to it.
///
/// The digest already commits to every record before it, so a signature
/// over the head is a signature over the whole journal; the count is
/// kept beside the version because the two agree on a whole journal and
/// disagree on one that lost records.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EvidenceHead {
    version: StreamVersion,
    hash: AuditRecordHash,
    record_count: usize,
}

impl EvidenceHead {
    #[must_use]
    pub const fn new(version: StreamVersion, hash: AuditRecordHash, record_count: usize) -> Self {
        Self {
            version,
            hash,
            record_count,
        }
    }

    #[must_use]
    pub const fn version(&self) -> StreamVersion {
        self.version
    }

    #[must_use]
    pub const fn hash(&self) -> AuditRecordHash {
        self.hash
    }

    #[must_use]
    pub const fn record_count(&self) -> usize {
        self.record_count
    }
}
