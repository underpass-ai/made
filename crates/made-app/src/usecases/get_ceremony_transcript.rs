use std::fmt;
use std::sync::Arc;

use made_core::error::DomainError;
use made_core::ports::CeremonyTranscriptStorePort;
use made_core::value_objects::{CeremonyId, CeremonyTranscript};

/// Retrieves the ordered contributions accumulated by one ceremony instance.
pub struct GetCeremonyTranscriptUseCase {
    transcript_store: Arc<dyn CeremonyTranscriptStorePort>,
}

impl fmt::Debug for GetCeremonyTranscriptUseCase {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("GetCeremonyTranscriptUseCase")
            .finish()
    }
}

impl GetCeremonyTranscriptUseCase {
    #[must_use]
    pub fn new(transcript_store: Arc<dyn CeremonyTranscriptStorePort>) -> Self {
        Self { transcript_store }
    }

    #[tracing::instrument(name = "get_ceremony_transcript", skip_all, fields(ceremony_id = %id))]
    pub async fn execute(&self, id: &CeremonyId) -> Result<CeremonyTranscript, DomainError> {
        self.transcript_store.transcript(id).await
    }
}
