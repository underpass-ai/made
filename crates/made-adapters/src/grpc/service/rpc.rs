use std::pin::Pin;
use std::task::{Context, Poll};

use async_trait::async_trait;
use made_proto::v1 as pb;
use tonic::{Request, Status};

use super::{
    run_with_ceremony_trace, trace_context_from_metadata, GrpcResult, MadeGrpcService, MadeService,
};

/// Boxed server stream used by the generated gRPC associated type.
pub struct RpcResultStream<T>(Pin<Box<dyn futures::Stream<Item = Result<T, Status>> + Send>>);

impl<T> std::fmt::Debug for RpcResultStream<T> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.debug_struct("RpcResultStream").finish()
    }
}

impl<T> RpcResultStream<T> {
    pub(super) fn new(
        stream: impl futures::Stream<Item = Result<T, Status>> + Send + 'static,
    ) -> Self {
        Self(Box::pin(stream))
    }
}

impl<T> futures::Stream for RpcResultStream<T> {
    type Item = Result<T, Status>;

    fn poll_next(self: Pin<&mut Self>, context: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        self.get_mut().0.as_mut().poll_next(context)
    }
}

#[async_trait]
impl MadeService for MadeGrpcService {
    async fn get_execution_receipt(
        &self,
        request: Request<pb::GetExecutionReceiptRequest>,
    ) -> GrpcResult<pb::GetExecutionReceiptResponse> {
        self.handle_get_execution_receipt(request).await
    }

    async fn inspect_execution_recovery(
        &self,
        request: Request<pb::InspectExecutionRecoveryRequest>,
    ) -> GrpcResult<pb::InspectExecutionRecoveryResponse> {
        self.handle_inspect_execution_recovery(request).await
    }

    async fn complete_execution_receipt(
        &self,
        request: Request<pb::ApplyExecutionReceiptRequest>,
    ) -> GrpcResult<pb::ApplyExecutionReceiptResponse> {
        self.handle_complete_execution_receipt(request).await
    }

    async fn adopt_execution_receipt(
        &self,
        request: Request<pb::ApplyExecutionReceiptRequest>,
    ) -> GrpcResult<pb::ApplyExecutionReceiptResponse> {
        self.handle_adopt_execution_receipt(request).await
    }

    async fn read_council_events(
        &self,
        request: Request<pb::ReadCouncilEventsRequest>,
    ) -> GrpcResult<pb::ReadCouncilEventsResponse> {
        self.handle_read_council_events(request).await
    }
    async fn get_council_event_cursor(
        &self,
        request: Request<pb::GetCouncilEventCursorRequest>,
    ) -> GrpcResult<pb::GetCouncilEventCursorResponse> {
        self.handle_get_council_event_cursor(request).await
    }
    async fn lease_council_events(
        &self,
        request: Request<pb::LeaseCouncilEventsRequest>,
    ) -> GrpcResult<pb::LeaseCouncilEventsResponse> {
        self.handle_lease_council_events(request).await
    }
    async fn acknowledge_council_events(
        &self,
        request: Request<pb::AcknowledgeCouncilEventsRequest>,
    ) -> GrpcResult<pb::AcknowledgeCouncilEventsResponse> {
        self.handle_acknowledge_council_events(request).await
    }
    async fn release_council_events(
        &self,
        request: Request<pb::ReleaseCouncilEventsRequest>,
    ) -> GrpcResult<pb::ReleaseCouncilEventsResponse> {
        self.handle_release_council_events(request).await
    }

    type StreamCeremonyStream = RpcResultStream<pb::StreamCeremonyResponse>;
    type StreamDeliberationStream = tokio_stream::wrappers::ReceiverStream<
        std::result::Result<pb::StreamDeliberationResponse, Status>,
    >;

    async fn deliberate(
        &self,
        request: Request<pb::DeliberateRequest>,
    ) -> GrpcResult<pb::DeliberateResponse> {
        self.handle_deliberate(request).await
    }

    async fn stream_deliberation(
        &self,
        request: Request<pb::StreamDeliberationRequest>,
    ) -> GrpcResult<Self::StreamDeliberationStream> {
        self.handle_stream_deliberation(request).await
    }

    async fn get_deliberation_result(
        &self,
        request: Request<pb::GetDeliberationResultRequest>,
    ) -> GrpcResult<pb::GetDeliberationResultResponse> {
        self.handle_get_deliberation_result(request).await
    }

    async fn orchestrate(
        &self,
        request: Request<pb::OrchestrateRequest>,
    ) -> GrpcResult<pb::OrchestrateResponse> {
        self.handle_orchestrate(request).await
    }

    async fn create_council(
        &self,
        request: Request<pb::CreateCouncilRequest>,
    ) -> GrpcResult<pb::CreateCouncilResponse> {
        self.handle_create_council(request).await
    }

    async fn list_councils(
        &self,
        request: Request<pb::ListCouncilsRequest>,
    ) -> GrpcResult<pb::ListCouncilsResponse> {
        self.handle_list_councils(request).await
    }

    async fn delete_council(
        &self,
        request: Request<pb::DeleteCouncilRequest>,
    ) -> GrpcResult<pb::DeleteCouncilResponse> {
        self.handle_delete_council(request).await
    }

    async fn register_agent(
        &self,
        request: Request<pb::RegisterAgentRequest>,
    ) -> GrpcResult<pb::RegisterAgentResponse> {
        self.handle_register_agent(request).await
    }

    async fn unregister_agent(
        &self,
        request: Request<pb::UnregisterAgentRequest>,
    ) -> GrpcResult<pb::UnregisterAgentResponse> {
        self.handle_unregister_agent(request).await
    }

    async fn run_council_decision(
        &self,
        request: Request<pb::RunCouncilDecisionRequest>,
    ) -> GrpcResult<pb::RunCouncilDecisionResponse> {
        self.handle_run_council_decision(request).await
    }

    async fn register_contract(
        &self,
        request: Request<pb::RegisterContractRequest>,
    ) -> GrpcResult<pb::RegisterContractResponse> {
        self.handle_register_contract(request).await
    }

    async fn list_contracts(
        &self,
        request: Request<pb::ListContractsRequest>,
    ) -> GrpcResult<pb::ListContractsResponse> {
        self.handle_list_contracts(request).await
    }

    async fn delete_contract(
        &self,
        request: Request<pb::DeleteContractRequest>,
    ) -> GrpcResult<pb::DeleteContractResponse> {
        self.handle_delete_contract(request).await
    }

    async fn process_trigger_event(
        &self,
        request: Request<pb::ProcessTriggerEventRequest>,
    ) -> GrpcResult<pb::ProcessTriggerEventResponse> {
        self.handle_process_trigger_event(request).await
    }

    async fn run_ceremony(
        &self,
        request: Request<pb::RunCeremonyRequest>,
    ) -> GrpcResult<pb::RunCeremonyResponse> {
        let trace = trace_context_from_metadata(&request);
        run_with_ceremony_trace(trace, self.handle_run_ceremony(request)).await
    }

    async fn start_ceremony(
        &self,
        request: Request<pb::StartCeremonyRequest>,
    ) -> GrpcResult<pb::StartCeremonyResponse> {
        let trace = trace_context_from_metadata(&request);
        run_with_ceremony_trace(trace, self.handle_start_ceremony(request)).await
    }

    async fn start_published_ceremony(
        &self,
        request: Request<pb::StartPublishedCeremonyRequest>,
    ) -> GrpcResult<pb::StartPublishedCeremonyResponse> {
        let trace = trace_context_from_metadata(&request);
        run_with_ceremony_trace(trace, self.handle_start_published_ceremony(request)).await
    }

    async fn run_ceremony_step(
        &self,
        request: Request<pb::RunCeremonyStepRequest>,
    ) -> GrpcResult<pb::RunCeremonyStepResponse> {
        let trace = trace_context_from_metadata(&request);
        run_with_ceremony_trace(trace, self.handle_run_ceremony_step(request)).await
    }

    async fn prepare_ceremony_children(
        &self,
        request: Request<pb::PrepareCeremonyChildrenRequest>,
    ) -> GrpcResult<pb::PrepareCeremonyChildrenResponse> {
        let trace = trace_context_from_metadata(&request);
        run_with_ceremony_trace(trace, self.handle_prepare_ceremony_children(request)).await
    }

    async fn accept_child_completion(
        &self,
        request: Request<pb::AcceptChildCompletionRequest>,
    ) -> GrpcResult<pb::AcceptChildCompletionResponse> {
        let trace = trace_context_from_metadata(&request);
        run_with_ceremony_trace(trace, self.handle_accept_child_completion(request)).await
    }

    async fn recover_ceremony_children(
        &self,
        request: Request<pb::RecoverCeremonyChildrenRequest>,
    ) -> GrpcResult<pb::RecoverCeremonyChildrenResponse> {
        let trace = trace_context_from_metadata(&request);
        run_with_ceremony_trace(trace, self.handle_recover_ceremony_children(request)).await
    }

    async fn claim_ceremony_step(
        &self,
        request: Request<pb::ClaimCeremonyStepRequest>,
    ) -> GrpcResult<pb::ClaimCeremonyStepResponse> {
        let trace = trace_context_from_metadata(&request);
        run_with_ceremony_trace(trace, self.handle_claim_ceremony_step(request)).await
    }

    async fn complete_ceremony_step(
        &self,
        request: Request<pb::CompleteCeremonyStepRequest>,
    ) -> GrpcResult<pb::CompleteCeremonyStepResponse> {
        let trace = trace_context_from_metadata(&request);
        run_with_ceremony_trace(trace, self.handle_complete_ceremony_step(request)).await
    }

    async fn apply_ceremony_transition(
        &self,
        request: Request<pb::ApplyCeremonyTransitionRequest>,
    ) -> GrpcResult<pb::ApplyCeremonyTransitionResponse> {
        let trace = trace_context_from_metadata(&request);
        run_with_ceremony_trace(trace, self.handle_apply_ceremony_transition(request)).await
    }

    async fn pause_ceremony(
        &self,
        request: Request<pb::PauseCeremonyRequest>,
    ) -> GrpcResult<pb::PauseCeremonyResponse> {
        let trace = trace_context_from_metadata(&request);
        run_with_ceremony_trace(trace, self.handle_pause_ceremony(request)).await
    }

    async fn resume_ceremony(
        &self,
        request: Request<pb::ResumeCeremonyRequest>,
    ) -> GrpcResult<pb::ResumeCeremonyResponse> {
        let trace = trace_context_from_metadata(&request);
        run_with_ceremony_trace(trace, self.handle_resume_ceremony(request)).await
    }

    async fn cancel_ceremony(
        &self,
        request: Request<pb::CancelCeremonyRequest>,
    ) -> GrpcResult<pb::CancelCeremonyResponse> {
        let trace = trace_context_from_metadata(&request);
        run_with_ceremony_trace(trace, self.handle_cancel_ceremony(request)).await
    }

    async fn enforce_ceremony_deadlines(
        &self,
        request: Request<pb::EnforceCeremonyDeadlinesRequest>,
    ) -> GrpcResult<pb::EnforceCeremonyDeadlinesResponse> {
        let trace = trace_context_from_metadata(&request);
        run_with_ceremony_trace(trace, self.handle_enforce_ceremony_deadlines(request)).await
    }

    async fn approve_ceremony_guard(
        &self,
        request: Request<pb::ApproveCeremonyGuardRequest>,
    ) -> GrpcResult<pb::ApproveCeremonyGuardResponse> {
        let trace = trace_context_from_metadata(&request);
        run_with_ceremony_trace(trace, self.handle_approve_ceremony_guard(request)).await
    }

    async fn defer_ceremony_guard(
        &self,
        request: Request<pb::DeferCeremonyGuardRequest>,
    ) -> GrpcResult<pb::DeferCeremonyGuardResponse> {
        let trace = trace_context_from_metadata(&request);
        run_with_ceremony_trace(trace, self.handle_defer_ceremony_guard(request)).await
    }

    async fn assert_ceremony_reason(
        &self,
        request: Request<pb::AssertCeremonyReasonRequest>,
    ) -> GrpcResult<pb::AssertCeremonyReasonResponse> {
        let trace = trace_context_from_metadata(&request);
        run_with_ceremony_trace(trace, self.handle_assert_ceremony_reason(request)).await
    }

    async fn request_ceremony_intervention(
        &self,
        request: Request<pb::RequestCeremonyInterventionRequest>,
    ) -> GrpcResult<pb::RequestCeremonyInterventionResponse> {
        let trace = trace_context_from_metadata(&request);
        run_with_ceremony_trace(trace, self.handle_request_ceremony_intervention(request)).await
    }

    async fn respond_to_ceremony_intervention(
        &self,
        request: Request<pb::RespondToCeremonyInterventionRequest>,
    ) -> GrpcResult<pb::RespondToCeremonyInterventionResponse> {
        let trace = trace_context_from_metadata(&request);
        run_with_ceremony_trace(trace, self.handle_respond_to_ceremony_intervention(request)).await
    }

    async fn close_ceremony_intervention(
        &self,
        request: Request<pb::CloseCeremonyInterventionRequest>,
    ) -> GrpcResult<pb::CloseCeremonyInterventionResponse> {
        let trace = trace_context_from_metadata(&request);
        run_with_ceremony_trace(trace, self.handle_close_ceremony_intervention(request)).await
    }

    async fn collect_ceremony_evidence(
        &self,
        request: Request<pb::CollectCeremonyEvidenceRequest>,
    ) -> GrpcResult<pb::CollectCeremonyEvidenceResponse> {
        let trace = trace_context_from_metadata(&request);
        run_with_ceremony_trace(trace, self.handle_collect_ceremony_evidence(request)).await
    }

    async fn validate_ceremony_draft(
        &self,
        request: Request<pb::ValidateCeremonyDraftRequest>,
    ) -> GrpcResult<pb::ValidateCeremonyDraftResponse> {
        self.handle_validate_ceremony_draft(request).await
    }

    async fn explain_ceremony_draft(
        &self,
        request: Request<pb::ExplainCeremonyDraftRequest>,
    ) -> GrpcResult<pb::ExplainCeremonyDraftResponse> {
        self.handle_explain_ceremony_draft(request).await
    }

    async fn design_ceremony(
        &self,
        request: Request<pb::DesignCeremonyRequest>,
    ) -> GrpcResult<pb::DesignCeremonyResponse> {
        self.handle_design_ceremony(request).await
    }

    async fn read_ceremony_events(
        &self,
        request: Request<pb::ReadCeremonyEventsRequest>,
    ) -> GrpcResult<pb::ReadCeremonyEventsResponse> {
        self.handle_read_ceremony_events(request).await
    }

    async fn stream_ceremony(
        &self,
        request: Request<pb::StreamCeremonyRequest>,
    ) -> GrpcResult<Self::StreamCeremonyStream> {
        self.handle_stream_ceremony(request).await
    }

    async fn pull_ceremony_events(
        &self,
        request: Request<pb::PullCeremonyEventsRequest>,
    ) -> GrpcResult<pb::PullCeremonyEventsResponse> {
        self.handle_pull_ceremony_events(request).await
    }

    async fn verify_ceremony_journal(
        &self,
        request: Request<pb::VerifyCeremonyJournalRequest>,
    ) -> GrpcResult<pb::VerifyCeremonyJournalResponse> {
        self.handle_verify_ceremony_journal(request).await
    }

    async fn get_ceremony_transcript(
        &self,
        request: Request<pb::GetCeremonyTranscriptRequest>,
    ) -> GrpcResult<pb::GetCeremonyTranscriptResponse> {
        self.handle_get_ceremony_transcript(request).await
    }

    async fn generate_ceremony_report(
        &self,
        request: Request<pb::GenerateCeremonyReportRequest>,
    ) -> GrpcResult<pb::GenerateCeremonyReportResponse> {
        self.handle_generate_ceremony_report(request).await
    }

    async fn begin_artifact_upload(
        &self,
        request: Request<pb::BeginArtifactUploadRequest>,
    ) -> GrpcResult<pb::BeginArtifactUploadResponse> {
        self.handle_begin_artifact_upload(request).await
    }

    async fn put_artifact_chunk(
        &self,
        request: Request<pb::PutArtifactChunkRequest>,
    ) -> GrpcResult<pb::PutArtifactChunkResponse> {
        self.handle_put_artifact_chunk(request).await
    }

    async fn commit_artifact_upload(
        &self,
        request: Request<pb::CommitArtifactUploadRequest>,
    ) -> GrpcResult<pb::CommitArtifactUploadResponse> {
        self.handle_commit_artifact_upload(request).await
    }

    async fn abort_artifact_upload(
        &self,
        request: Request<pb::AbortArtifactUploadRequest>,
    ) -> GrpcResult<pb::AbortArtifactUploadResponse> {
        self.handle_abort_artifact_upload(request).await
    }

    async fn get_artifact(
        &self,
        request: Request<pb::GetArtifactRequest>,
    ) -> GrpcResult<pb::GetArtifactResponse> {
        self.handle_get_artifact(request).await
    }

    async fn list_artifacts(
        &self,
        request: Request<pb::ListArtifactsRequest>,
    ) -> GrpcResult<pb::ListArtifactsResponse> {
        self.handle_list_artifacts(request).await
    }

    async fn read_artifact_chunk(
        &self,
        request: Request<pb::ReadArtifactChunkRequest>,
    ) -> GrpcResult<pb::ReadArtifactChunkResponse> {
        self.handle_read_artifact_chunk(request).await
    }

    async fn tombstone_artifact(
        &self,
        request: Request<pb::TombstoneArtifactRequest>,
    ) -> GrpcResult<pb::TombstoneArtifactResponse> {
        self.handle_tombstone_artifact(request).await
    }

    async fn publish_ceremony_definition(
        &self,
        request: Request<pb::PublishCeremonyDefinitionRequest>,
    ) -> GrpcResult<pb::PublishCeremonyDefinitionResponse> {
        self.handle_publish_ceremony_definition(request).await
    }

    async fn bind_ceremony_participants(
        &self,
        request: Request<pb::BindCeremonyParticipantsRequest>,
    ) -> GrpcResult<pb::BindCeremonyParticipantsResponse> {
        let trace = trace_context_from_metadata(&request);
        run_with_ceremony_trace(trace, self.handle_bind_ceremony_participants(request)).await
    }

    async fn diff_ceremony_definitions(
        &self,
        request: Request<pb::DiffCeremonyDefinitionsRequest>,
    ) -> GrpcResult<pb::DiffCeremonyDefinitionsResponse> {
        self.handle_diff_ceremony_definitions(request).await
    }

    async fn get_ceremony_instance(
        &self,
        request: Request<pb::GetCeremonyInstanceRequest>,
    ) -> GrpcResult<pb::GetCeremonyInstanceResponse> {
        self.handle_get_ceremony_instance(request).await
    }

    async fn list_ceremony_instances(
        &self,
        request: Request<pb::ListCeremonyInstancesRequest>,
    ) -> GrpcResult<pb::ListCeremonyInstancesResponse> {
        self.handle_list_ceremony_instances(request).await
    }

    async fn get_status(
        &self,
        request: Request<pb::GetStatusRequest>,
    ) -> GrpcResult<pb::GetStatusResponse> {
        self.handle_get_status(request).await
    }

    async fn get_metrics(
        &self,
        request: Request<pb::GetMetricsRequest>,
    ) -> GrpcResult<pb::GetMetricsResponse> {
        self.handle_get_metrics(request).await
    }
}
