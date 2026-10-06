//! [`SignedEvidenceBundle`] — a bundle with its attestation, as it is
//! written to a file and read back by somebody with no store at all.
//!
//! Read back, nothing is trusted yet: the head the file claims and the
//! records it carries are kept apart so a verifier can recompute one
//! from the other and say which of the three questions failed.

use crate::error::DomainError;
use crate::value_objects::{CeremonyId, EvidenceHead, EvidenceSignature};

use super::{AuditRecord, EvidenceBundle};

mod head_wire;
mod signature_wire;
mod wire;

/// The file's own schema name, so a reader refuses a shape it does not
/// know rather than guessing at it.
pub const SIGNED_EVIDENCE_BUNDLE_SCHEMA: &str = "underpass.made.signed-evidence-bundle.v1";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SignedEvidenceBundle {
    ceremony_id: CeremonyId,
    head: EvidenceHead,
    records: Vec<AuditRecord>,
    signature: EvidenceSignature,
}

impl SignedEvidenceBundle {
    /// Attach a signature to a bundle built from verified records.
    #[must_use]
    pub fn new(bundle: EvidenceBundle, signature: EvidenceSignature) -> Self {
        let head = bundle.head();
        Self {
            ceremony_id: bundle.ceremony_id().clone(),
            head,
            records: bundle.into_records(),
            signature,
        }
    }

    /// What a file claimed, before anything about it is believed.
    #[must_use]
    pub const fn from_parts(
        ceremony_id: CeremonyId,
        head: EvidenceHead,
        records: Vec<AuditRecord>,
        signature: EvidenceSignature,
    ) -> Self {
        Self {
            ceremony_id,
            head,
            records,
            signature,
        }
    }

    #[must_use]
    pub fn ceremony_id(&self) -> &CeremonyId {
        &self.ceremony_id
    }

    /// The head the bundle claims; compare with what the records give.
    #[must_use]
    pub const fn head(&self) -> &EvidenceHead {
        &self.head
    }

    #[must_use]
    pub fn records(&self) -> &[AuditRecord] {
        &self.records
    }

    #[must_use]
    pub const fn signature(&self) -> &EvidenceSignature {
        &self.signature
    }

    /// The bytes the signature was made over, from the claimed head.
    #[must_use]
    pub fn attestation_message(&self) -> Vec<u8> {
        EvidenceBundle::attestation_message(&self.ceremony_id, &self.head)
    }

    pub fn to_json(&self) -> Result<String, DomainError> {
        serde_json::to_string_pretty(&wire::SignedEvidenceBundleWire::from(self)).map_err(|_| {
            DomainError::InvariantViolated {
                reason: "evidence bundle could not be rendered",
            }
        })
    }

    pub fn from_json(json: &str) -> Result<Self, DomainError> {
        let wire: wire::SignedEvidenceBundleWire =
            serde_json::from_str(json).map_err(|_| DomainError::InvariantViolated {
                reason: "evidence bundle file is not a signed evidence bundle",
            })?;
        Self::try_from(wire)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::value_objects::EvidenceSignatureAlgorithm;

    fn signed() -> SignedEvidenceBundle {
        let bundle =
            EvidenceBundle::from_records(crate::entities::evidence_bundle::tests::chain()).unwrap();
        let signature = EvidenceSignature::new(
            EvidenceSignatureAlgorithm::Ed25519,
            vec![1; 32],
            vec![2; 64],
        )
        .unwrap();
        SignedEvidenceBundle::new(bundle, signature)
    }

    #[test]
    fn a_bundle_round_trips_through_its_file_shape() {
        let bundle = signed();
        let json = bundle.to_json().unwrap();
        assert!(json.contains(SIGNED_EVIDENCE_BUNDLE_SCHEMA));
        assert!(json.contains(&bundle.head().hash().to_hex()));
        let read = SignedEvidenceBundle::from_json(&json).unwrap();
        assert_eq!(read, bundle);
        assert_eq!(read.attestation_message(), bundle.attestation_message());
    }

    #[test]
    fn a_foreign_schema_or_shape_is_refused() {
        let json = signed().to_json().unwrap().replace(
            SIGNED_EVIDENCE_BUNDLE_SCHEMA,
            "underpass.made.signed-evidence-bundle.v9",
        );
        assert!(SignedEvidenceBundle::from_json(&json).is_err());
        assert!(SignedEvidenceBundle::from_json("{\"records\":[]}").is_err());
        assert!(SignedEvidenceBundle::from_json("not json").is_err());
    }
}
