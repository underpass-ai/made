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
    ExportEvidenceBundleUseCase, GenerateProtocolReportInput, GenerateProtocolReportUseCase,
    GetProtocolTranscriptUseCase, ProtocolEventPage, ProtocolJournalVerdict,
    ProtocolProgressStream, ProtocolReport, PullProtocolEventsInput, PullProtocolEventsOutput,
    PullProtocolEventsUseCase, ReadProtocolEventsInput, ReadProtocolEventsUseCase,
    ReadWholeProtocolEventsUseCase, StreamProtocolInput, VerifyProtocolJournalUseCase,
};
use made_core::entities::{AuditRecord, SignedEvidenceBundle};
use made_core::error::DomainError;
use made_core::ports::EvidenceSignerPort;
use made_core::value_objects::{
    AuthorizationAction, GlobalPosition, ProtocolEventConsumer, ProtocolEventPageLimit, ProtocolId,
    ProtocolTranscript, StreamVersion,
};

use super::EmbeddedMade;

impl EmbeddedMade {
    /// Replay and follow one protocol's sealed records within caller bounds.
    pub async fn stream_protocol(
        &self,
        input: StreamProtocolInput,
    ) -> Result<ProtocolProgressStream, DomainError> {
        self.require_authorized_protocol_action(
            AuthorizationAction::StreamProtocol,
            input.protocol_id(),
        )?;
        self.progress_stream.execute(input).await
    }

    /// Every sealed record of one session's stream, in stream order.
    ///
    /// Kept beside the paged read below because a chain is verified
    /// from its first record: a caller that wants to check what it was
    /// given asks for the whole stream, and a caller that is following
    /// one asks for a page.
    pub async fn audit_records(&self, id: &ProtocolId) -> Result<Vec<AuditRecord>, DomainError> {
        self.require_authorized_protocol_records(id)?;
        ReadWholeProtocolEventsUseCase::new(self.events.clone())
            .execute(id)
            .await
    }

    /// One page of a session's stream: the records after `from`, at
    /// most `limit` of them, and where the reader now stands.
    ///
    /// `limit` absent takes [`ReadProtocolEventsInput::DEFAULT_LIMIT`]
    /// and anything above `MAX_LIMIT` is refused rather than cut down
    /// to it; a protocol with no stream is
    /// `NotFound`, where the whole-stream read above answers with
    /// nothing.
    pub async fn audit_records_from(
        &self,
        id: &ProtocolId,
        from: StreamVersion,
        limit: Option<usize>,
    ) -> Result<ProtocolEventPage, DomainError> {
        self.require_authorized_protocol_records(id)?;
        let limit = match limit {
            Some(limit) => ProtocolEventPageLimit::new(limit)?,
            None => ProtocolEventPageLimit::DEFAULT,
        };
        ReadProtocolEventsUseCase::new(self.events.clone())
            .execute(ReadProtocolEventsInput::new(id.clone(), from, limit))
            .await
    }

    /// Read a named global feed, optionally acknowledging prior delivery first.
    pub async fn pull_events(
        &self,
        consumer: ProtocolEventConsumer,
        limit: ProtocolEventPageLimit,
        acknowledge_through: Option<GlobalPosition>,
    ) -> Result<PullProtocolEventsOutput, DomainError> {
        self.require_authorized_global_action(AuthorizationAction::PullProtocolEvents)?;
        PullProtocolEventsUseCase::new(self.events.clone(), self.cursors.clone())
            .execute(PullProtocolEventsInput::new(
                consumer,
                limit,
                acknowledge_through,
            ))
            .await
    }

    pub async fn transcript(&self, id: &ProtocolId) -> Result<ProtocolTranscript, DomainError> {
        self.require_authorized_protocol_action(AuthorizationAction::GetProtocolTranscript, id)?;
        GetProtocolTranscriptUseCase::new(self.events.clone())
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
        input: GenerateProtocolReportInput,
    ) -> Result<ProtocolReport, DomainError> {
        self.require_authorized_global_action(AuthorizationAction::GenerateProtocolReport)?;
        GenerateProtocolReportUseCase::new(self.resolve_definition(), self.events.clone())
            .execute(input)
            .await
    }

    /// Whether one session's journal is sealed, positioned and linked
    /// as it was written.
    ///
    /// The engine's own answer to a question the caller can also
    /// answer for itself: [`Self::audit_records`] hands out the
    /// records, and `AuditChain::verify` in `made-core` is the same
    /// verifier this runs. That is what makes the answer evidence
    /// rather than a reassurance.
    pub async fn verify_journal(
        &self,
        id: &ProtocolId,
    ) -> Result<ProtocolJournalVerdict, DomainError> {
        self.require_authorized_protocol_action(AuthorizationAction::VerifyProtocolJournal, id)?;
        VerifyProtocolJournalUseCase::new(self.events.clone())
            .execute(id)
            .await
    }

    /// One session's journal, verified and signed, as a bundle that can
    /// be read and judged where this store is not.
    ///
    /// The same records [`Self::audit_records`] hands out, under the
    /// same authorization; the signer is the caller's because which
    /// key vouches for an export is the operator's decision, not the
    /// engine's.
    pub async fn export_evidence_bundle(
        &self,
        id: &ProtocolId,
        signer: Arc<dyn EvidenceSignerPort>,
    ) -> Result<SignedEvidenceBundle, DomainError> {
        self.require_authorized_protocol_records(id)?;
        ExportEvidenceBundleUseCase::new(self.events.clone(), signer)
            .execute(id)
            .await
    }
}
