use crate::error::DomainError;
use crate::value_objects::EvidenceSignature;

/// Checks an attestation against the public key it carries.
///
/// A malformed key or signature is an error; a well-formed signature
/// that does not verify is `Ok(false)`. The two are different news: the
/// first is a bundle that cannot be judged, the second a bundle that
/// was.
pub trait EvidenceVerifierPort: Send + Sync {
    fn verify(&self, signature: &EvidenceSignature, message: &[u8]) -> Result<bool, DomainError>;
}
