use std::fmt;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::error::DomainError;

use super::{CeremonyId, ChildSpawnCoordinates};

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ChildGroupId(String);

impl ChildGroupId {
    pub fn new(value: impl Into<String>) -> Result<Self, DomainError> {
        let value = value.into();
        if value.len() != 64
            || !value
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        {
            return Err(DomainError::InvalidCharacters {
                field: "child_group_id",
            });
        }
        Ok(Self(value))
    }

    #[must_use]
    pub fn derive(parent_id: &CeremonyId, coordinates: &ChildSpawnCoordinates) -> Self {
        let mut digest = Sha256::new();
        digest.update(b"made.child-group.v1\0");
        for part in [
            parent_id.as_str().to_owned(),
            coordinates.step_id().as_str().to_owned(),
            coordinates.state_visit().get().to_string(),
            coordinates.state_iteration().get().to_string(),
            coordinates.step_iteration().get().to_string(),
        ] {
            digest.update((part.len() as u64).to_be_bytes());
            digest.update(part.as_bytes());
        }
        Self(format!("{:x}", digest.finalize()))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for ChildGroupId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}
