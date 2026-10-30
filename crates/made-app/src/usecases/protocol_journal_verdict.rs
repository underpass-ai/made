use made_core::value_objects::{
    AuditChainDefect, AuditChainVerdict, AuditSequence, ProtocolId, StreamVersion,
};

/// What verifying one session's journal found.
///
/// The answer names how far the stream goes, how many records were
/// checked and whether the chain holds — and, when it does not, the
/// first position that cannot be trusted and why. The first only: past
/// a break the verifier does not know what it is looking at any more,
/// and a list of further defects would suggest otherwise.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProtocolJournalVerdict {
    protocol_id: ProtocolId,
    head_version: StreamVersion,
    record_count: usize,
    verdict: AuditChainVerdict,
}

impl ProtocolJournalVerdict {
    #[must_use]
    pub fn new(
        protocol_id: ProtocolId,
        head_version: StreamVersion,
        record_count: usize,
        verdict: AuditChainVerdict,
    ) -> Self {
        Self {
            protocol_id,
            head_version,
            record_count,
            verdict,
        }
    }

    #[must_use]
    pub fn protocol_id(&self) -> &ProtocolId {
        &self.protocol_id
    }

    /// How far the stream goes.
    #[must_use]
    pub const fn head_version(&self) -> StreamVersion {
        self.head_version
    }

    /// How many records were verified.
    ///
    /// Beside the head version rather than derived from it: they agree
    /// on a whole journal and disagree on one that lost records, which
    /// is precisely the case worth telling apart.
    #[must_use]
    pub const fn record_count(&self) -> usize {
        self.record_count
    }

    #[must_use]
    pub fn is_intact(&self) -> bool {
        self.verdict.is_intact()
    }

    /// The first position that cannot be trusted, if any.
    #[must_use]
    pub fn first_broken_sequence(&self) -> Option<AuditSequence> {
        self.verdict.defect().map(AuditChainDefect::at)
    }

    /// Why it cannot be trusted, in words.
    #[must_use]
    pub fn reason(&self) -> Option<String> {
        self.verdict.defect().map(AuditChainDefect::explain)
    }
}
