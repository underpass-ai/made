use std::fmt;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::error::DomainError;

const DIGEST_BYTES: usize = 32;

/// Domain separator, distinct from the ceremony definition scheme on
/// purpose: two different aggregates whose digests could collide would
/// let one identity be read as the other's.
const CANONICAL_SCHEME: &[u8] = b"underpass.made.agentic-system.v1";

/// SHA-256 identity of one revision of an agentic system design.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct AgenticSystemDigest([u8; DIGEST_BYTES]);

impl AgenticSystemDigest {
    #[must_use]
    pub const fn from_bytes(bytes: [u8; DIGEST_BYTES]) -> Self {
        Self(bytes)
    }

    pub fn parse_hex(value: &str) -> Result<Self, DomainError> {
        let value = value.trim();
        if value.len() != DIGEST_BYTES * 2 {
            return Err(DomainError::InvalidCharacters {
                field: "agentic_system_digest",
            });
        }
        let mut bytes = [0_u8; DIGEST_BYTES];
        for (index, byte) in bytes.iter_mut().enumerate() {
            let start = index * 2;
            *byte = u8::from_str_radix(&value[start..start + 2], 16).map_err(|_| {
                DomainError::InvalidCharacters {
                    field: "agentic_system_digest",
                }
            })?;
        }
        Ok(Self(bytes))
    }

    /// Seal a canonical encoding of the design into a digest.
    #[must_use]
    pub fn of_canonical_form(canonical: &[u8]) -> Self {
        let mut hasher = Sha256::new();
        hasher.update(CANONICAL_SCHEME);
        hasher.update(canonical);
        Self(hasher.finalize().into())
    }

    #[must_use]
    pub const fn as_bytes(&self) -> &[u8; DIGEST_BYTES] {
        &self.0
    }

    #[must_use]
    pub fn to_hex(self) -> String {
        const DIGITS: &[u8; 16] = b"0123456789abcdef";

        let mut hex = String::with_capacity(DIGEST_BYTES * 2);
        for byte in self.0 {
            hex.push(DIGITS[usize::from(byte >> 4)] as char);
            hex.push(DIGITS[usize::from(byte & 0x0f)] as char);
        }
        hex
    }
}

impl fmt::Display for AgenticSystemDigest {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.to_hex())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hex_round_trips() {
        let digest = AgenticSystemDigest::from_bytes([0x5a; DIGEST_BYTES]);

        assert_eq!(
            AgenticSystemDigest::parse_hex(&digest.to_hex()).unwrap(),
            digest
        );
        assert!(AgenticSystemDigest::parse_hex("beef").is_err());
    }

    #[test]
    fn the_scheme_separates_a_design_from_a_definition_of_the_same_bytes() {
        assert_ne!(
            AgenticSystemDigest::of_canonical_form(b"payload").as_bytes(),
            &<[u8; DIGEST_BYTES]>::from(Sha256::digest(b"payload"))
        );
    }
}
