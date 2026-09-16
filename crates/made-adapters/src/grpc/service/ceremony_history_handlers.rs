//! Reading what a session left behind, and reporting on it.
//!
//! Three reads, no writes. They resolve nothing about the session
//! beyond what the use cases resolve, because a stream and a
//! transcript are facts about a ceremony rather than views of its
//! current state.

use made_app::usecases::{GenerateCeremonyReportInput, ReadCeremonyEventsInput};
use made_core::value_objects::StreamVersion;

use super::{
    domain_error_to_status, generate_ceremony_report_response_from,
    get_ceremony_transcript_response_from, link_span_to_metadata, pb,
    read_ceremony_events_response_from, CeremonyId, GrpcResult, MadeGrpcService, Request, Response,
};

impl MadeGrpcService {
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
        let limit = (request.limit > 0).then(|| request.limit as usize);
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
        // An empty title is an absent one: proto3 has no other way to
        // say it, and the schema that fronts the tool refuses a blank
        // one anyway.
        let title = (!request.title.is_empty()).then_some(request.title);
        let report = self
            .generate_ceremony_report
            .execute(GenerateCeremonyReportInput::new(ceremony_ids, title))
            .await
            .map_err(domain_error_to_status)?;
        Ok(Response::new(generate_ceremony_report_response_from(
            &report,
        )))
    }
}
