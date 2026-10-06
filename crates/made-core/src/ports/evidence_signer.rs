use crate::error::DomainError;
use crate::value_objects::EvidenceSignature;

/// Signs an evidence bundle's attestation message with a key the
/// domain never sees.
///
/// Synchronous: a signature is arithmetic over bytes already in hand,
/// and a port that awaited would suggest a service behind it.
pub trait EvidenceSignerPort: Send + Sync {
    fn sign(&self, message: &[u8]) -> Result<EvidenceSignature, DomainError>;
}
