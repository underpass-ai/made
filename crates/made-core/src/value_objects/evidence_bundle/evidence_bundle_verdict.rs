use crate::value_objects::AuditChainVerdict;

/// What verifying a signed bundle found, question by question: do the
/// records chain, do they end where the bundle says they end, and did
/// the named key sign that ending.
///
/// Three answers rather than one because they fail for different
/// reasons and call for different remedies — a broken chain is a
/// tampered record, a head that does not match is a bundle rewritten
/// after signing, a bad signature is the wrong key or a forged one.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EvidenceBundleVerdict {
    chain: AuditChainVerdict,
    head_matches: bool,
    signature_valid: bool,
}

impl EvidenceBundleVerdict {
    #[must_use]
    pub const fn new(chain: AuditChainVerdict, head_matches: bool, signature_valid: bool) -> Self {
        Self {
            chain,
            head_matches,
            signature_valid,
        }
    }

    #[must_use]
    pub const fn chain(&self) -> AuditChainVerdict {
        self.chain
    }

    #[must_use]
    pub const fn head_matches(&self) -> bool {
        self.head_matches
    }

    #[must_use]
    pub const fn signature_valid(&self) -> bool {
        self.signature_valid
    }

    /// Every question answered yes.
    #[must_use]
    pub fn is_sound(&self) -> bool {
        self.chain.is_intact() && self.head_matches && self.signature_valid
    }
}
