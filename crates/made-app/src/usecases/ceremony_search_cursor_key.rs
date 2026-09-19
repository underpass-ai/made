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

    pub(crate) const fn as_bytes(&self) -> &[u8; Self::BYTES] {
        &self.0
    }
}
