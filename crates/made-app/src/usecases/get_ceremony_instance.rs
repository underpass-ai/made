use std::fmt;
use std::sync::Arc;

use made_core::entities::CeremonyInstance;
use made_core::error::DomainError;
use made_core::value_objects::CeremonyId;

use crate::services::SessionStream;

/// Retrieves a ceremony instance as the fold of its stream.
pub struct GetCeremonyInstanceUseCase {
    stream: Arc<SessionStream>,
}

impl fmt::Debug for GetCeremonyInstanceUseCase {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("GetCeremonyInstanceUseCase")
            .finish()
    }
}

impl GetCeremonyInstanceUseCase {
    #[must_use]
    pub fn new(stream: Arc<SessionStream>) -> Self {
        Self { stream }
    }

    #[tracing::instrument(name = "get_ceremony_instance", skip_all, fields(ceremony_id = %id))]
    pub async fn execute(&self, id: &CeremonyId) -> Result<CeremonyInstance, DomainError> {
        self.stream.load(id).await.map(|session| session.instance)
    }
}
