//! What a session left behind, as the in-process facade answers it.
//!
//! A second `impl EmbeddedMade` rather than a second type: these are
//! the facade's own verbs, moved out of `embedded_made.rs` unchanged
//! so that file stays under the size the architecture gate holds it
//! to. The stream, the transcript and the report are one group — they
//! all answer "what happened" from state the engine already holds, and
//! none of them writes.

use std::sync::Arc;

use made_app::usecases::{
    CeremonyEventPage, CeremonyReport, GenerateCeremonyReportInput, GenerateCeremonyReportUseCase,
    GetCeremonyInstanceUseCase, GetCeremonyTranscriptUseCase, ReadCeremonyEventsInput,
    ReadCeremonyEventsUseCase,
};
use made_core::entities::AuditRecord;
use made_core::error::DomainError;
use made_core::value_objects::{CeremonyId, CeremonyTranscript, StreamVersion};

use super::EmbeddedMade;

impl EmbeddedMade {
    /// Every sealed record of one session's stream, in stream order.
    ///
    /// Kept beside the paged read below because a chain is verified
    /// from its first record: a caller that wants to check what it was
    /// given asks for the whole stream, and a caller that is following
    /// one asks for a page.
    pub async fn audit_records(&self, id: &CeremonyId) -> Result<Vec<AuditRecord>, DomainError> {
        self.events.read(id, StreamVersion::EMPTY).await
    }

    /// One page of a session's stream: the records after `from`, at
    /// most `limit` of them, and where the reader now stands.
    ///
    /// `limit` absent takes [`ReadCeremonyEventsInput::DEFAULT_LIMIT`]
    /// and is capped at `MAX_LIMIT`; a ceremony with no stream is
    /// `NotFound`, where the whole-stream read above answers with
    /// nothing.
    pub async fn audit_records_from(
        &self,
        id: &CeremonyId,
        from: StreamVersion,
        limit: Option<usize>,
    ) -> Result<CeremonyEventPage, DomainError> {
        ReadCeremonyEventsUseCase::new(self.events.clone())
            .execute(ReadCeremonyEventsInput::new(id.clone(), from, limit))
            .await
    }

    pub async fn transcript(&self, id: &CeremonyId) -> Result<CeremonyTranscript, DomainError> {
        GetCeremonyTranscriptUseCase::new(self.transcript_store.clone())
            .execute(id)
            .await
    }

    /// The Markdown report over one or more sessions (ADR-006).
    ///
    /// A projection, not a document: nothing is written, and the same
    /// state reports the same bytes. The deployable edition renders it
    /// through the same use case, which is what makes the two answers
    /// identical.
    pub async fn report(
        &self,
        input: GenerateCeremonyReportInput,
    ) -> Result<CeremonyReport, DomainError> {
        GenerateCeremonyReportUseCase::new(
            Arc::new(GetCeremonyInstanceUseCase::new(self.stream.clone())),
            self.resolve_definition(),
            self.events.clone(),
        )
        .execute(input)
        .await
    }
}
