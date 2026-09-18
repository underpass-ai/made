use serde::{Deserialize, Serialize};

use crate::error::DomainError;

/// Operator-supplied provenance label for an imported council snapshot; never a credential URL.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct CouncilSnapshotSource(String);

impl CouncilSnapshotSource {
    pub const MAX_LEN: usize = 128;

    pub fn new(value: impl Into<String>) -> Result<Self, DomainError> {
        let value = value.into();
        let value = value.trim();
        if value.is_empty() {
            return Err(DomainError::EmptyField {
                field: "council_snapshot_source",
            });
        }
        if value.len() > Self::MAX_LEN {
            return Err(DomainError::FieldTooLong {
                field: "council_snapshot_source",
                actual: value.len(),
                max: Self::MAX_LEN,
            });
        }
        if !value
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '.' | ':' | '/'))
        {
            return Err(DomainError::InvalidCharacters {
                field: "council_snapshot_source",
            });
        }
        Ok(Self(value.to_owned()))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for CouncilSnapshotSource {
    type Error = DomainError;
    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}
impl From<CouncilSnapshotSource> for String {
    fn from(value: CouncilSnapshotSource) -> Self {
        value.0
    }
}
