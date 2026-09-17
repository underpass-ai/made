//! Reading what a session left behind, reporting on it, and checking
//! the chain that seals it.
//!
//! Four reads, no writes. They resolve nothing about the session
//! beyond what the use cases resolve, because a stream and a
//! transcript are facts about a ceremony rather than views of its
//! current state.

use made_app::usecases::{GenerateCeremonyReportInput, ReadCeremonyEventsInput, ReportTitle};
use made_core::value_objects::StreamVersion;

use super::{
    domain_error_to_status, generate_ceremony_report_response_from,
    get_ceremony_transcript_response_from, link_span_to_metadata, pb,
    read_ceremony_events_response_from, verify_ceremony_journal_response_from, CeremonyId,
    GrpcResult, MadeGrpcService, Request, Response,
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
        let limit = (request.limit > 0).then_some(request.limit as usize);
        let page = self
            .read_ceremony_events
            .execute(
                ReadCeremonyEventsInput::new(
                    ceremony_id,
                    StreamVersion::new(request.from_version),
                    limit,
                )
                .map_err(domain_error_to_status)?,
            )
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
            .execute(GenerateCeremonyReportInput::new(ceremony_ids, title))
            .await
            .map_err(domain_error_to_status)?;
        Ok(Response::new(generate_ceremony_report_response_from(
            &report,
        )))
    }
}
