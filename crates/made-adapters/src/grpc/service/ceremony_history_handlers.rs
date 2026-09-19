//! Reading what a session left behind, reporting on it, and checking
//! the chain that seals it.
//!
//! Read-only operations. They resolve nothing about the session
//! beyond what the use cases resolve, because a stream and a
//! transcript are facts about a ceremony rather than views of its
//! current state.

use futures::StreamExt;
use made_app::usecases::{
    GenerateCeremonyReportInput, PullCeremonyEventsInput, ReadCeremonyEventsInput, ReportTitle,
    StreamCeremonyInput,
};
use made_core::value_objects::{
    CeremonyEventConsumer, CeremonyEventPageLimit, CeremonyProgressWait, GlobalPosition,
    StreamVersion,
};

use super::rpc::RpcResultStream;
use super::{
    domain_error_to_status, generate_ceremony_report_response_from,
    get_ceremony_transcript_response_from, link_span_to_metadata, pb,
    pull_ceremony_events_response_from, read_ceremony_events_response_from,
    stream_ceremony_response_from, verify_ceremony_journal_response_from, CeremonyId, GrpcResult,
    MadeGrpcService, Request, Response,
};

impl MadeGrpcService {
    #[tracing::instrument(name = "rpc.stream_ceremony", skip_all)]
    pub(super) async fn handle_stream_ceremony(
        &self,
        request: Request<pb::StreamCeremonyRequest>,
    ) -> GrpcResult<<Self as pb::made_service_server::MadeService>::StreamCeremonyStream> {
        link_span_to_metadata(&request);
        let request = request.into_inner();
        let ceremony_id = CeremonyId::new(request.ceremony_id).map_err(domain_error_to_status)?;
        let max_events = if request.max_events == 0 {
            CeremonyEventPageLimit::DEFAULT
        } else {
            CeremonyEventPageLimit::new(request.max_events as usize)
                .map_err(domain_error_to_status)?
        };
        let wait_timeout = CeremonyProgressWait::from_millis(
            request
                .wait_timeout_ms
                .unwrap_or(CeremonyProgressWait::DEFAULT.millis()),
        )
        .map_err(domain_error_to_status)?;
        let stream = self
            .stream_ceremony
            .execute(StreamCeremonyInput::new(
                ceremony_id,
                StreamVersion::new(request.after_sequence),
                max_events,
                wait_timeout,
            ))
            .await
            .map_err(domain_error_to_status)?;
        Ok(Response::new(RpcResultStream::new(
            stream.map(progress_frame_to_grpc),
        )))
    }

    #[tracing::instrument(name = "rpc.pull_ceremony_events", skip_all)]
    pub(super) async fn handle_pull_ceremony_events(
        &self,
        request: Request<pb::PullCeremonyEventsRequest>,
    ) -> GrpcResult<pb::PullCeremonyEventsResponse> {
        link_span_to_metadata(&request);
        let request = request.into_inner();
        let consumer =
            CeremonyEventConsumer::new(request.consumer).map_err(domain_error_to_status)?;
        let limit = if request.limit == 0 {
            CeremonyEventPageLimit::DEFAULT
        } else {
            CeremonyEventPageLimit::new(request.limit as usize).map_err(domain_error_to_status)?
        };
        let acknowledge_through = request
            .acknowledge_through
            .map(GlobalPosition::new)
            .transpose()
            .map_err(domain_error_to_status)?;
        let output = self
            .pull_ceremony_events
            .execute(PullCeremonyEventsInput::new(
                consumer,
                limit,
                acknowledge_through,
            ))
            .await
            .map_err(domain_error_to_status)?;
        Ok(Response::new(
            pull_ceremony_events_response_from(&output).map_err(domain_error_to_status)?,
        ))
    }

    #[tracing::instrument(name = "rpc.read_ceremony_events", skip_all)]
    pub(super) async fn handle_read_ceremony_events(
        &self,
        request: Request<pb::ReadCeremonyEventsRequest>,
    ) -> GrpcResult<pb::ReadCeremonyEventsResponse> {
        link_span_to_metadata(&request);
        let request = request.into_inner();
        let ceremony_id = CeremonyId::new(request.ceremony_id).map_err(domain_error_to_status)?;
        // Zero is "no limit asked for" on the wire, and the use case
        // turns that into the default. Proto3 has no other way to say
        // an absent scalar, and asking for no records is not a
        // question anyone means.
        let limit = if request.limit == 0 {
            CeremonyEventPageLimit::DEFAULT
        } else {
            CeremonyEventPageLimit::new(request.limit as usize).map_err(domain_error_to_status)?
        };
        let page = self
            .read_ceremony_events
            .execute(ReadCeremonyEventsInput::new(
                ceremony_id,
                StreamVersion::new(request.from_version),
                limit,
            ))
            .await
            .map_err(domain_error_to_status)?;
        Ok(Response::new(
            read_ceremony_events_response_from(&page).map_err(domain_error_to_status)?,
        ))
    }

    /// The verdict on one journal's chain.
    ///
    /// A broken chain is an answer, not an error: the caller asked
    /// whether the journal holds, and "it does not, from position
    /// seven, because the digest no longer matches" is that question
    /// answered. A `Status` would say the call failed.
    #[tracing::instrument(name = "rpc.verify_ceremony_journal", skip_all)]
    pub(super) async fn handle_verify_ceremony_journal(
        &self,
        request: Request<pb::VerifyCeremonyJournalRequest>,
    ) -> GrpcResult<pb::VerifyCeremonyJournalResponse> {
        link_span_to_metadata(&request);
        let ceremony_id =
            CeremonyId::new(request.into_inner().ceremony_id).map_err(domain_error_to_status)?;
        let verdict = self
            .verify_ceremony_journal
            .execute(&ceremony_id)
            .await
            .map_err(domain_error_to_status)?;
        Ok(Response::new(verify_ceremony_journal_response_from(
            &verdict,
        )))
    }

    #[tracing::instrument(name = "rpc.get_ceremony_transcript", skip_all)]
    pub(super) async fn handle_get_ceremony_transcript(
        &self,
        request: Request<pb::GetCeremonyTranscriptRequest>,
    ) -> GrpcResult<pb::GetCeremonyTranscriptResponse> {
        link_span_to_metadata(&request);
        let ceremony_id =
            CeremonyId::new(request.into_inner().ceremony_id).map_err(domain_error_to_status)?;
        let transcript = self
            .get_ceremony_transcript
            .execute(&ceremony_id)
            .await
            .map_err(domain_error_to_status)?;
        Ok(Response::new(get_ceremony_transcript_response_from(
            &transcript,
        )))
    }

    #[tracing::instrument(name = "rpc.generate_ceremony_report", skip_all)]
    pub(super) async fn handle_generate_ceremony_report(
        &self,
        request: Request<pb::GenerateCeremonyReportRequest>,
    ) -> GrpcResult<pb::GenerateCeremonyReportResponse> {
        link_span_to_metadata(&request);
        let request = request.into_inner();
        let ceremony_ids = request
            .ceremony_ids
            .into_iter()
            .map(CeremonyId::new)
            .collect::<Result<Vec<_>, _>>()
            .map_err(domain_error_to_status)?;
        // An unset title is the empty string: proto3 has no other way
        // to say it, and that is the only reading this handler gives
        // it. Everything else — what a heading is once it is present,
        // and that a heading of nothing but space is not one — is
        // `ReportTitle`'s, which is the rule the in-process arm applies
        // too. So a padded heading renders one document rather than
        // two, and a blank one is refused identically on both arms.
        let title = (!request.title.is_empty())
            .then(|| ReportTitle::new(request.title))
            .transpose()
            .map_err(domain_error_to_status)?;
        let report = self
            .generate_ceremony_report
            .execute(
                GenerateCeremonyReportInput::new(ceremony_ids, title)
                    .map_err(domain_error_to_status)?,
            )
            .await
            .map_err(domain_error_to_status)?;
        Ok(Response::new(generate_ceremony_report_response_from(
            &report,
        )))
    }
}

// Tonic fixes a server-stream item's error to `Status`; the generated trait
// leaves no smaller error representation at this boundary.
#[allow(clippy::result_large_err)]
fn progress_frame_to_grpc(
    item: Result<made_app::usecases::CeremonyProgressFrame, made_core::error::DomainError>,
) -> Result<pb::StreamCeremonyResponse, tonic::Status> {
    item.and_then(stream_ceremony_response_from)
        .map_err(domain_error_to_status)
}
