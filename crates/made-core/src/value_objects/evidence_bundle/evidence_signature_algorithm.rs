use crate::error::DomainError;

/// The signature scheme an attestation was made with. One today; named
/// so a bundle signed tomorrow under another can say so instead of
/// being verified under the wrong one.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EvidenceSignatureAlgorithm {
    Ed25519,
}

impl EvidenceSignatureAlgorithm {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Ed25519 => "ed25519",
        }
    }

    pub fn parse(value: &str) -> Result<Self, DomainError> {
        match value.trim() {
            "ed25519" => Ok(Self::Ed25519),
            _ => Err(DomainError::InvariantViolated {
                reason: "evidence signature algorithm is not one this build verifies",
            }),
        }
    }
}
