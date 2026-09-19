/// Secret key used to authenticate public ceremony search cursors.
///
/// Configuration must provide the same key to every replica and after a
/// restart. Deliberately does not implement `Debug` or expose its bytes.
#[derive(Clone)]
pub struct CeremonySearchCursorKey([u8; Self::BYTES]);

impl std::fmt::Debug for CeremonySearchCursorKey {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("CeremonySearchCursorKey([REDACTED])")
    }
}

impl CeremonySearchCursorKey {
    pub const BYTES: usize = 32;

    #[must_use]
    pub const fn new(bytes: [u8; Self::BYTES]) -> Self {
        Self(bytes)
    }

    pub fn from_hex(value: &str) -> Result<Self, made_core::error::DomainError> {
        if value.len() != Self::BYTES * 2 {
            return Err(made_core::error::DomainError::InvalidCharacters {
                field: "ceremony_search_cursor_hmac_key",
            });
        }
        let mut bytes = [0_u8; Self::BYTES];
        for (index, pair) in value.as_bytes().chunks_exact(2).enumerate() {
            bytes[index] = (hex_nibble(pair[0])? << 4) | hex_nibble(pair[1])?;
        }
        Ok(Self(bytes))
    }

    pub(crate) const fn as_bytes(&self) -> &[u8; Self::BYTES] {
        &self.0
    }
}

fn hex_nibble(value: u8) -> Result<u8, made_core::error::DomainError> {
    match value {
        b'0'..=b'9' => Ok(value - b'0'),
        b'a'..=b'f' => Ok(value - b'a' + 10),
        b'A'..=b'F' => Ok(value - b'A' + 10),
        _ => Err(made_core::error::DomainError::InvalidCharacters {
            field: "ceremony_search_cursor_hmac_key",
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_exactly_one_256_bit_hex_key_without_exposing_it() {
        let key = CeremonySearchCursorKey::from_hex(&"a5".repeat(32)).unwrap();
        assert_eq!(format!("{key:?}"), "CeremonySearchCursorKey([REDACTED])");
        assert!(CeremonySearchCursorKey::from_hex(&"a5".repeat(31)).is_err());
        assert!(CeremonySearchCursorKey::from_hex(&"zz".repeat(32)).is_err());
    }
}
