use std::fmt;

use serde::{Deserialize, Serialize};

use crate::error::DomainError;

const TRACE_ID_HEX_LEN: usize = 32;

/// Validated W3C trace identity, without span-local context.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct TraceId(String);

impl TraceId {
    /// Validate a 16-byte, non-zero hexadecimal trace identity.
    pub fn new(raw: impl Into<String>) -> Result<Self, DomainError> {
        let raw = raw.into().to_ascii_lowercase();
        if raw.len() != TRACE_ID_HEX_LEN {
            return Err(DomainError::FieldTooLong {
                field: "trace_id",
                actual: raw.len(),
                max: TRACE_ID_HEX_LEN,
            });
        }
        if !raw.bytes().all(|byte| byte.is_ascii_hexdigit()) {
            return Err(DomainError::InvalidCharacters { field: "trace_id" });
        }
        if raw.bytes().all(|byte| byte == b'0') {
            return Err(DomainError::InvariantViolated {
                reason: "trace_id must not be all zeros",
            });
        }
        Ok(Self(raw))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for TraceId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_and_canonicalises_a_w3c_trace_id() {
        let id = TraceId::new("0AF7651916CD43DD8448EB211C80319C").unwrap();
        assert_eq!(id.as_str(), "0af7651916cd43dd8448eb211c80319c");
    }

    #[test]
    fn rejects_invalid_and_zero_identities() {
        assert!(TraceId::new("short").is_err());
        assert!(TraceId::new("0af7651916cd43dd8448eb211c80319z").is_err());
        assert!(TraceId::new("00000000000000000000000000000000").is_err());
    }
}
