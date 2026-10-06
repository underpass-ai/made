use crate::error::DomainError;

use super::EvidenceSignatureAlgorithm;

/// A signature over an evidence bundle's attestation message, with the
/// public key that verifies it.
///
/// The key travels with the signature so a bundle can be verified by
/// someone who never saw the signer's machine; whether that key is one
/// they trust is their question, and `verify-evidence --public-key`
/// is where they ask it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EvidenceSignature {
    algorithm: EvidenceSignatureAlgorithm,
    public_key: Vec<u8>,
    value: Vec<u8>,
}

impl EvidenceSignature {
    pub fn new(
        algorithm: EvidenceSignatureAlgorithm,
        public_key: Vec<u8>,
        value: Vec<u8>,
    ) -> Result<Self, DomainError> {
        if public_key.is_empty() || value.is_empty() {
            return Err(DomainError::InvariantViolated {
                reason: "evidence signature needs a public key and a value",
            });
        }
        Ok(Self {
            algorithm,
            public_key,
            value,
        })
    }

    /// Build from the hex spellings a bundle file carries.
    pub fn parse_hex(algorithm: &str, public_key: &str, value: &str) -> Result<Self, DomainError> {
        Self::new(
            EvidenceSignatureAlgorithm::parse(algorithm)?,
            decode_hex(public_key, "evidence_signature_public_key")?,
            decode_hex(value, "evidence_signature_value")?,
        )
    }

    #[must_use]
    pub const fn algorithm(&self) -> EvidenceSignatureAlgorithm {
        self.algorithm
    }

    #[must_use]
    pub fn public_key(&self) -> &[u8] {
        &self.public_key
    }

    #[must_use]
    pub fn value(&self) -> &[u8] {
        &self.value
    }

    #[must_use]
    pub fn public_key_hex(&self) -> String {
        encode_hex(&self.public_key)
    }

    #[must_use]
    pub fn value_hex(&self) -> String {
        encode_hex(&self.value)
    }
}

#[must_use]
pub fn encode_hex(bytes: &[u8]) -> String {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    let mut hex = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        hex.push(DIGITS[usize::from(byte >> 4)] as char);
        hex.push(DIGITS[usize::from(byte & 0x0f)] as char);
    }
    hex
}

pub fn decode_hex(value: &str, field: &'static str) -> Result<Vec<u8>, DomainError> {
    let value = value.trim();
    if value.is_empty() || !value.len().is_multiple_of(2) {
        return Err(DomainError::InvalidCharacters { field });
    }
    (0..value.len())
        .step_by(2)
        .map(|start| {
            u8::from_str_radix(&value[start..start + 2], 16)
                .map_err(|_| DomainError::InvalidCharacters { field })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hex_round_trips_and_refuses_odd_or_foreign_input() {
        let bytes = vec![0_u8, 1, 0xab, 0xff];
        let hex = encode_hex(&bytes);
        assert_eq!(hex, "0001abff");
        assert_eq!(decode_hex(&hex, "f").unwrap(), bytes);
        assert!(decode_hex("abc", "f").is_err());
        assert!(decode_hex("zz", "f").is_err());
        assert!(decode_hex("", "f").is_err());
    }

    #[test]
    fn a_signature_needs_both_parts_and_a_known_algorithm() {
        assert!(EvidenceSignature::parse_hex("ed25519", "00", "").is_err());
        assert!(EvidenceSignature::parse_hex("rsa", "00", "00").is_err());
        let signature = EvidenceSignature::parse_hex("ed25519", "0a0b", "0c").unwrap();
        assert_eq!(signature.public_key_hex(), "0a0b");
        assert_eq!(signature.value_hex(), "0c");
        assert_eq!(signature.algorithm(), EvidenceSignatureAlgorithm::Ed25519);
    }
}
