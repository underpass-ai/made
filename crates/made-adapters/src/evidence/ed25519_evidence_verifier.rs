use ed25519_dalek::{Signature, VerifyingKey};
use made_core::error::DomainError;
use made_core::ports::EvidenceVerifierPort;
use made_core::value_objects::{EvidenceSignature, EvidenceSignatureAlgorithm};

/// Checks an Ed25519 attestation against the public key it carries.
/// Holds nothing: the key is in the signature, which is what lets a
/// bundle be judged on a machine that has never seen the signer.
#[derive(Debug, Default, Clone, Copy)]
pub struct Ed25519EvidenceVerifier;

impl EvidenceVerifierPort for Ed25519EvidenceVerifier {
    fn verify(&self, signature: &EvidenceSignature, message: &[u8]) -> Result<bool, DomainError> {
        if signature.algorithm() != EvidenceSignatureAlgorithm::Ed25519 {
            return Err(DomainError::InvariantViolated {
                reason: "evidence signature algorithm is not one this verifier judges",
            });
        }
        let key_bytes: [u8; 32] =
            signature
                .public_key()
                .try_into()
                .map_err(|_| DomainError::InvariantViolated {
                    reason: "evidence signature public key is not 32 bytes",
                })?;
        let key =
            VerifyingKey::from_bytes(&key_bytes).map_err(|_| DomainError::InvariantViolated {
                reason: "evidence signature public key is not a valid Ed25519 point",
            })?;
        let Ok(signature_bytes) = <[u8; 64]>::try_from(signature.value()) else {
            return Err(DomainError::InvariantViolated {
                reason: "evidence signature value is not 64 bytes",
            });
        };
        Ok(key
            .verify_strict(message, &Signature::from_bytes(&signature_bytes))
            .is_ok())
    }
}
