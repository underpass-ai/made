use async_trait::async_trait;

use crate::error::DomainError;
use crate::ports::CeremonyInstanceIdPage;
use crate::value_objects::{CeremonyId, CeremonyIdPrefix, CeremonyInstancePageLimit};

/// Keyset access to ceremony stream identities without loading their journals.
#[async_trait]
pub trait CeremonyInstanceIndexPort: Send + Sync {
    /// Return ids strictly above `after`, ordered by id and optionally restricted
    /// to a literal prefix. Implementations fetch at most `limit + 1` rows.
    async fn ids_after(
        &self,
        after: Option<&CeremonyId>,
        id_prefix: Option<&CeremonyIdPrefix>,
        limit: CeremonyInstancePageLimit,
    ) -> Result<CeremonyInstanceIdPage, DomainError>;
}
