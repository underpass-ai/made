use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

use crate::error::DomainError;
use crate::value_objects::{ProtocolId, RoleId};

/// Who says so, and when they saw it.
///
/// Memory without provenance is hearsay: an agent reading it later
/// cannot weigh a claim it cannot attribute. The session is carried
/// too, so a memory found from somewhere else can be walked back to
/// the working session that produced it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MemoryProvenance {
    protocol_id: ProtocolId,
    role_id: Option<RoleId>,
    #[serde(with = "time::serde::rfc3339")]
    observed_at: OffsetDateTime,
}

impl MemoryProvenance {
    #[must_use]
    pub fn new(
        protocol_id: ProtocolId,
        role_id: Option<RoleId>,
        observed_at: OffsetDateTime,
    ) -> Self {
        Self {
            protocol_id,
            role_id,
            observed_at,
        }
    }

    #[must_use]
    pub fn protocol_id(&self) -> &ProtocolId {
        &self.protocol_id
    }

    /// The role that saw it, when a role did. Absent for what the
    /// engine itself observed — a transition firing has no author.
    #[must_use]
    pub fn role_id(&self) -> Option<&RoleId> {
        self.role_id.as_ref()
    }

    #[must_use]
    pub fn observed_at(&self) -> OffsetDateTime {
        self.observed_at
    }

    /// An idempotency key that names the write rather than the moment.
    ///
    /// Retrying a write must not double it, and the only durable way
    /// to say "this is that same write again" is to derive the key
    /// from what is being written about. Wall-clock time would make
    /// every retry a new memory.
    pub fn idempotency_key(&self, discriminator: &str) -> Result<String, DomainError> {
        if discriminator.trim().is_empty() {
            return Err(DomainError::EmptyField {
                field: "memory_provenance.discriminator",
            });
        }
        Ok(format!(
            "remember:{}:{}",
            self.protocol_id.as_str(),
            discriminator.trim()
        ))
    }
}
