//! Signing and verifying portable evidence bundles with Ed25519.
//!
//! The domain decides what is signed (the attestation message of a
//! bundle's head) and what a verdict means; this module only does the
//! arithmetic and keeps the seed on disk where only its owner reads it.

mod ed25519_evidence_signer;
mod ed25519_evidence_verifier;
mod evidence_signing_key_file;

pub use ed25519_evidence_signer::Ed25519EvidenceSigner;
pub use ed25519_evidence_verifier::Ed25519EvidenceVerifier;

#[cfg(test)]
mod tests {
    use made_core::ports::{EvidenceSignerPort, EvidenceVerifierPort};
    use made_core::value_objects::{EvidenceSignature, EvidenceSignatureAlgorithm};

    use super::*;

    #[test]
    fn a_signature_verifies_under_its_own_key_and_under_no_other() {
        let signer = Ed25519EvidenceSigner::generate().unwrap();
        let message = b"underpass.made.evidence-bundle.v1 test";
        let signature = signer.sign(message).unwrap();
        assert_eq!(signature.public_key(), signer.public_key());
        assert_eq!(signature.value().len(), 64);

        let verifier = Ed25519EvidenceVerifier;
        assert!(verifier.verify(&signature, message).unwrap());
        assert!(!verifier.verify(&signature, b"another message").unwrap());

        let other = Ed25519EvidenceSigner::generate().unwrap();
        let forged = EvidenceSignature::new(
            EvidenceSignatureAlgorithm::Ed25519,
            other.public_key().to_vec(),
            signature.value().to_vec(),
        )
        .unwrap();
        assert!(!verifier.verify(&forged, message).unwrap());

        let malformed = EvidenceSignature::new(
            EvidenceSignatureAlgorithm::Ed25519,
            vec![1; 5],
            signature.value().to_vec(),
        )
        .unwrap();
        assert!(verifier.verify(&malformed, message).is_err());
    }

    #[test]
    fn the_seed_file_round_trips_and_is_never_overwritten() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("keys").join("evidence.key");
        let signer = Ed25519EvidenceSigner::generate().unwrap();
        signer.write_seed_file(&path).unwrap();

        let reloaded = Ed25519EvidenceSigner::from_seed_file(&path).unwrap();
        assert_eq!(reloaded.public_key(), signer.public_key());
        assert!(signer.write_seed_file(&path).is_err());

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt as _;
            let mode = std::fs::metadata(&path).unwrap().permissions().mode() & 0o777;
            assert_eq!(mode, 0o600);
            std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o644)).unwrap();
            assert!(Ed25519EvidenceSigner::from_seed_file(&path).is_err());
        }
    }
}
