use std::path::Path;

use ed25519_dalek::{Signer, SigningKey};
use made_core::error::DomainError;
use made_core::ports::EvidenceSignerPort;
use made_core::value_objects::{encode_hex, EvidenceSignature, EvidenceSignatureAlgorithm};

use super::evidence_signing_key_file::EvidenceSigningKeyFile;

/// Signs attestation messages with an Ed25519 key held in memory.
///
/// The key comes from a seed file setup wrote owner-only, or from a
/// fresh seed drawn from the operating system. It never leaves this
/// type: callers get signatures and the public key, and nothing else.
pub struct Ed25519EvidenceSigner {
    key: SigningKey,
}

impl std::fmt::Debug for Ed25519EvidenceSigner {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Ed25519EvidenceSigner")
            .field("public_key", &self.public_key_hex())
            .finish()
    }
}

impl Ed25519EvidenceSigner {
    /// A new key from the operating system's randomness.
    pub fn generate() -> Result<Self, DomainError> {
        let mut seed = [0_u8; 32];
        getrandom::getrandom(&mut seed).map_err(|_| DomainError::InvariantViolated {
            reason: "the operating system did not supply randomness for a signing key",
        })?;
        Ok(Self {
            key: SigningKey::from_bytes(&seed),
        })
    }

    #[must_use]
    pub fn from_seed(seed: [u8; 32]) -> Self {
        Self {
            key: SigningKey::from_bytes(&seed),
        }
    }

    /// Read the seed setup wrote. Refuses a file anybody but its owner
    /// can read, on the systems that can tell.
    pub fn from_seed_file(path: &Path) -> Result<Self, DomainError> {
        Ok(Self::from_seed(EvidenceSigningKeyFile::read(path)?))
    }

    /// Write this key's seed where only the owner can read it. Refuses
    /// to replace a file that exists: a key is rotated on purpose.
    pub fn write_seed_file(&self, path: &Path) -> Result<(), DomainError> {
        EvidenceSigningKeyFile::write(path, &self.key.to_bytes())
    }

    #[must_use]
    pub fn public_key(&self) -> [u8; 32] {
        self.key.verifying_key().to_bytes()
    }

    #[must_use]
    pub fn public_key_hex(&self) -> String {
        encode_hex(&self.public_key())
    }
}

impl EvidenceSignerPort for Ed25519EvidenceSigner {
    fn sign(&self, message: &[u8]) -> Result<EvidenceSignature, DomainError> {
        EvidenceSignature::new(
            EvidenceSignatureAlgorithm::Ed25519,
            self.public_key().to_vec(),
            self.key.sign(message).to_bytes().to_vec(),
        )
    }
}
