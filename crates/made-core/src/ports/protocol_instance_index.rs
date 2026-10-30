use async_trait::async_trait;

use crate::error::DomainError;
use crate::ports::ProtocolInstanceIdPage;
use crate::value_objects::{ProtocolId, ProtocolIdPrefix, ProtocolInstancePageLimit};

/// Keyset access to protocol stream identities without loading their journals.
#[async_trait]
pub trait ProtocolInstanceIndexPort: Send + Sync {
    /// Return ids strictly above `after`, ordered by id and optionally restricted
    /// to a literal prefix. Implementations fetch at most `limit + 1` rows.
    async fn ids_after(
        &self,
        after: Option<&ProtocolId>,
        id_prefix: Option<&ProtocolIdPrefix>,
        limit: ProtocolInstancePageLimit,
    ) -> Result<ProtocolInstanceIdPage, DomainError>;
}
