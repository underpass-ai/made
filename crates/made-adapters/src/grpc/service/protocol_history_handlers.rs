//! Reading what a session left behind, reporting on it, and checking
//! the chain that seals it.
//!
//! Read-only operations. They resolve nothing about the session
//! beyond what the use cases resolve, because a stream and a
//! transcript are facts about a protocol rather than views of its
//! current state.

use futures::StreamExt;
use made_app::usecases::{
    GenerateProtocolReportInput, PullProtocolEventsInput, ReadProtocolEventsInput, ReportTitle,
    StreamProtocolInput,
};
use made_core::value_objects::{
    GlobalPosition, ProtocolAgentExecutionId, ProtocolEventConsumer, ProtocolEventPageLimit,
    ProtocolProgressWait, RoleId, StepId, StreamVersion,
};

use super::rpc::RpcResultStream;
use super::{
    domain_error_to_status, generate_protocol_report_response_from,
    get_protocol_transcript_response_from, link_span_to_metadata, pb,
    pull_protocol_events_response_from, read_protocol_events_response_from,
    stream_protocol_response_from, verify_protocol_journal_response_from, GrpcResult,
    MadeGrpcService, ProtocolId, Request, Response,
};

impl MadeGrpcService {
    #[tracing::instrument(name = "rpc.stream_protocol", skip_all)]
    pub(super) async fn handle_stream_protocol(
        &self,
        request: Request<pb::StreamProtocolRequest>,
    ) -> GrpcResult<<Self as pb::made_service_server::MadeService>::StreamProtocolStream> {
        link_span_to_metadata(&request);
        let request = request.into_inner();
        let protocol_id = ProtocolId::new(request.protocol_id).map_err(domain_error_to_status)?;
        let max_events = if request.max_events == 0 {
            ProtocolEventPageLimit::DEFAULT
        } else {
            ProtocolEventPageLimit::new(request.max_events as usize)
                .map_err(domain_error_to_status)?
        };
        let wait_timeout = ProtocolProgressWait::from_millis(
            request
                .wait_timeout_ms
                .unwrap_or(ProtocolProgressWait::DEFAULT.millis()),
        )
        .map_err(domain_error_to_status)?;
        let input = StreamProtocolInput::new(
            protocol_id,
            StreamVersion::new(request.after_sequence),
            max_events,
            wait_timeout,
        );
        let input = if request.include_agent_activity {
            input.with_agent_activity(
                request.after_activity_sequence,
                request
                    .role_id
                    .map(RoleId::new)
                    .transpose()
                    .map_err(domain_error_to_status)?,
                request
                    .step_id
                    .map(StepId::new)
                    .transpose()
                    .map_err(domain_error_to_status)?,
                request
                    .agent_execution_id
                    .map(ProtocolAgentExecutionId::new)
                    .transpose()
                    .map_err(domain_error_to_status)?,
            )
        } else {
            input
        };
        let stream = self
            .stream_protocol
            .execute(input)
            .await
            .map_err(domain_error_to_status)?;
        Ok(Response::new(RpcResultStream::new(
            stream.map(progress_frame_to_grpc),
        )))
    }

    #[tracing::instrument(name = "rpc.pull_protocol_events", skip_all)]
    pub(super) async fn handle_pull_protocol_events(
        &self,
        request: Request<pb::PullProtocolEventsRequest>,
    ) -> GrpcResult<pb::PullProtocolEventsResponse> {
        link_span_to_metadata(&request);
        let request = request.into_inner();
        let consumer =
            ProtocolEventConsumer::new(request.consumer).map_err(domain_error_to_status)?;
        let limit = if request.limit == 0 {
            ProtocolEventPageLimit::DEFAULT
        } else {
            ProtocolEventPageLimit::new(request.limit as usize).map_err(domain_error_to_status)?
        };
        let acknowledge_through = request
            .acknowledge_through
            .map(GlobalPosition::new)
            .transpose()
            .map_err(domain_error_to_status)?;
        let output = self
            .pull_protocol_events
            .execute(PullProtocolEventsInput::new(
                consumer,
                limit,
                acknowledge_through,
            ))
            .await
            .map_err(domain_error_to_status)?;
        Ok(Response::new(
            pull_protocol_events_response_from(&output).map_err(domain_error_to_status)?,
        ))
    }

    #[tracing::instrument(name = "rpc.read_protocol_events", skip_all)]
    pub(super) async fn handle_read_protocol_events(
        &self,
        request: Request<pb::ReadProtocolEventsRequest>,
    ) -> GrpcResult<pb::ReadProtocolEventsResponse> {
        link_span_to_metadata(&request);
        let request = request.into_inner();
        let protocol_id = ProtocolId::new(request.protocol_id).map_err(domain_error_to_status)?;
        // Zero is "no limit asked for" on the wire, and the use case
        // turns that into the default. Proto3 has no other way to say
        // an absent scalar, and asking for no records is not a
        // question anyone means.
        let limit = if request.limit == 0 {
            ProtocolEventPageLimit::DEFAULT
        } else {
            ProtocolEventPageLimit::new(request.limit as usize).map_err(domain_error_to_status)?
        };
        let page = self
            .read_protocol_events
            .execute(ReadProtocolEventsInput::new(
                protocol_id,
                StreamVersion::new(request.from_version),
                limit,
            ))
            .await
            .map_err(domain_error_to_status)?;
        Ok(Response::new(
            read_protocol_events_response_from(&page).map_err(domain_error_to_status)?,
        ))
    }

    /// The verdict on one journal's chain.
    ///
    /// A broken chain is an answer, not an error: the caller asked
    /// whether the journal holds, and "it does not, from position
    /// seven, because the digest no longer matches" is that question
    /// answered. A `Status` would say the call failed.
    #[tracing::instrument(name = "rpc.verify_protocol_journal", skip_all)]
    pub(super) async fn handle_verify_protocol_journal(
        &self,
        request: Request<pb::VerifyProtocolJournalRequest>,
    ) -> GrpcResult<pb::VerifyProtocolJournalResponse> {
        link_span_to_metadata(&request);
        let protocol_id =
            ProtocolId::new(request.into_inner().protocol_id).map_err(domain_error_to_status)?;
        let verdict = self
            .verify_protocol_journal
            .execute(&protocol_id)
            .await
            .map_err(domain_error_to_status)?;
        Ok(Response::new(verify_protocol_journal_response_from(
            &verdict,
        )))
    }

    #[tracing::instrument(name = "rpc.get_protocol_transcript", skip_all)]
    pub(super) async fn handle_get_protocol_transcript(
        &self,
        request: Request<pb::GetProtocolTranscriptRequest>,
    ) -> GrpcResult<pb::GetProtocolTranscriptResponse> {
        link_span_to_metadata(&request);
        let protocol_id =
            ProtocolId::new(request.into_inner().protocol_id).map_err(domain_error_to_status)?;
        let transcript = self
            .get_protocol_transcript
            .execute(&protocol_id)
            .await
            .map_err(domain_error_to_status)?;
        Ok(Response::new(get_protocol_transcript_response_from(
            &transcript,
        )))
    }

    #[tracing::instrument(name = "rpc.generate_protocol_report", skip_all)]
    pub(super) async fn handle_generate_protocol_report(
        &self,
        request: Request<pb::GenerateProtocolReportRequest>,
    ) -> GrpcResult<pb::GenerateProtocolReportResponse> {
        link_span_to_metadata(&request);
        let request = request.into_inner();
        let protocol_ids = request
            .protocol_ids
            .into_iter()
            .map(ProtocolId::new)
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
            .generate_protocol_report
            .execute(
                GenerateProtocolReportInput::new(protocol_ids, title)
                    .map_err(domain_error_to_status)?,
            )
            .await
            .map_err(domain_error_to_status)?;
        Ok(Response::new(generate_protocol_report_response_from(
            &report,
        )))
    }
}

// Tonic fixes a server-stream item's error to `Status`; the generated trait
// leaves no smaller error representation at this boundary.
#[allow(clippy::result_large_err)]
fn progress_frame_to_grpc(
    item: Result<made_app::usecases::ProtocolProgressFrame, made_core::error::DomainError>,
) -> Result<pb::StreamProtocolResponse, tonic::Status> {
    item.and_then(stream_protocol_response_from)
        .map_err(domain_error_to_status)
}
