use serde::{Deserialize, Serialize};

use crate::entities::AuditRecord;
use crate::error::DomainError;
use crate::value_objects::{
    AuditRecordHash, CeremonyId, EvidenceHead, EvidenceSignature, StreamVersion,
};

use super::SignedEvidenceBundle;

/// The one file shape a bundle is written in and read from. Hashes and
/// keys travel as hex so a person can read them and a diff can show
/// them; every other field is the record's own serialization.
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct SignedEvidenceBundleWire {
    schema: String,
    ceremony_id: String,
    head: HeadWire,
    records: Vec<AuditRecord>,
    signature: SignatureWire,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct HeadWire {
    version: u64,
    hash: String,
    record_count: u64,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct SignatureWire {
    algorithm: String,
    public_key: String,
    value: String,
}

impl From<&SignedEvidenceBundle> for SignedEvidenceBundleWire {
    fn from(bundle: &SignedEvidenceBundle) -> Self {
        Self {
            schema: super::SIGNED_EVIDENCE_BUNDLE_SCHEMA.to_owned(),
            ceremony_id: bundle.ceremony_id().as_str().to_owned(),
            head: HeadWire {
                version: bundle.head().version().value(),
                hash: bundle.head().hash().to_hex(),
                record_count: bundle.head().record_count() as u64,
            },
            records: bundle.records().to_vec(),
            signature: SignatureWire {
                algorithm: bundle.signature().algorithm().as_str().to_owned(),
                public_key: bundle.signature().public_key_hex(),
                value: bundle.signature().value_hex(),
            },
        }
    }
}

impl TryFrom<SignedEvidenceBundleWire> for SignedEvidenceBundle {
    type Error = DomainError;

    fn try_from(wire: SignedEvidenceBundleWire) -> Result<Self, Self::Error> {
        if wire.schema != super::SIGNED_EVIDENCE_BUNDLE_SCHEMA {
            return Err(DomainError::InvariantViolated {
                reason: "evidence bundle schema is not one this build reads",
            });
        }
        let record_count = usize::try_from(wire.head.record_count).map_err(|_| {
            DomainError::InvariantViolated {
                reason: "evidence bundle head record count is out of range",
            }
        })?;
        Ok(Self::from_parts(
            CeremonyId::new(wire.ceremony_id)?,
            EvidenceHead::new(
                StreamVersion::new(wire.head.version),
                AuditRecordHash::parse_hex(&wire.head.hash)?,
                record_count,
            ),
            wire.records,
            EvidenceSignature::parse_hex(
                &wire.signature.algorithm,
                &wire.signature.public_key,
                &wire.signature.value,
            )?,
        ))
    }
}
