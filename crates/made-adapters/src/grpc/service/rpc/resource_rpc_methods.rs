macro_rules! resource_rpc_methods {
    ($callback:ident; { $($methods:tt)* }) => {
        agentic_system_rpc_methods!($callback; {
            $($methods)*
        async fn validate_protocol_draft(
            &self,
            request: Request<pb::ValidateProtocolDraftRequest>,
        ) -> GrpcResult<pb::ValidateProtocolDraftResponse> {
            authorized_definition!(
                self,
                request,
                ValidateProtocolDraft,
                self.handle_validate_protocol_draft(request)
            )
        }
        async fn explain_protocol_draft(
            &self,
            request: Request<pb::ExplainProtocolDraftRequest>,
        ) -> GrpcResult<pb::ExplainProtocolDraftResponse> {
            authorized_definition!(
                self,
                request,
                ExplainProtocolDraft,
                self.handle_explain_protocol_draft(request)
            )
        }
        async fn design_protocol(
            &self,
            request: Request<pb::DesignProtocolRequest>,
        ) -> GrpcResult<pb::DesignProtocolResponse> {
            authorized_global!(
                self,
                request,
                DesignProtocol,
                self.handle_design_protocol(request)
            )
        }
        async fn read_protocol_events(
            &self,
            request: Request<pb::ReadProtocolEventsRequest>,
        ) -> GrpcResult<pb::ReadProtocolEventsResponse> {
            authorized_protocol!(
                self,
                request,
                ReadProtocolEvents,
                self.handle_read_protocol_events(request)
            )
        }
        async fn stream_protocol(
            &self,
            request: Request<pb::StreamProtocolRequest>,
        ) -> GrpcResult<Self::StreamProtocolStream> {
            authorized_protocol!(
                self,
                request,
                StreamProtocol,
                self.handle_stream_protocol(request)
            )
        }
        async fn pull_protocol_events(
            &self,
            request: Request<pb::PullProtocolEventsRequest>,
        ) -> GrpcResult<pb::PullProtocolEventsResponse> {
            authorized_global!(
                self,
                request,
                PullProtocolEvents,
                self.handle_pull_protocol_events(request)
            )
        }
        async fn verify_protocol_journal(
            &self,
            request: Request<pb::VerifyProtocolJournalRequest>,
        ) -> GrpcResult<pb::VerifyProtocolJournalResponse> {
            authorized_protocol!(
                self,
                request,
                VerifyProtocolJournal,
                self.handle_verify_protocol_journal(request)
            )
        }
        async fn get_protocol_transcript(
            &self,
            request: Request<pb::GetProtocolTranscriptRequest>,
        ) -> GrpcResult<pb::GetProtocolTranscriptResponse> {
            authorized_protocol!(
                self,
                request,
                GetProtocolTranscript,
                self.handle_get_protocol_transcript(request)
            )
        }
        async fn generate_protocol_report(
            &self,
            request: Request<pb::GenerateProtocolReportRequest>,
        ) -> GrpcResult<pb::GenerateProtocolReportResponse> {
            authorized_global!(
                self,
                request,
                GenerateProtocolReport,
                self.handle_generate_protocol_report(request)
            )
        }
        async fn get_budget_report(
            &self,
            request: Request<pb::GetBudgetReportRequest>,
        ) -> GrpcResult<pb::GetBudgetReportResponse> {
        authorized_budget_for_protocol!(
            self,
            request,
            ReadBudget,
                self.handle_get_budget_report(request)
            )
        }
        async fn list_pending_budget_reservations(
            &self,
            request: Request<pb::ListPendingBudgetReservationsRequest>,
        ) -> GrpcResult<pb::ListPendingBudgetReservationsResponse> {
            authorized_global!(
                self,
                request,
                ReadBudget,
                self.handle_list_pending_budget_reservations(request)
            )
        }
        async fn begin_artifact_upload(
            &self,
            request: Request<pb::BeginArtifactUploadRequest>,
        ) -> GrpcResult<pb::BeginArtifactUploadResponse> {
        authorized_artifact_upload_begin!(
            self,
            request,
            BeginArtifactUpload,
                self.handle_begin_artifact_upload(request)
            )
        }
        async fn put_artifact_chunk(
            &self,
            request: Request<pb::PutArtifactChunkRequest>,
        ) -> GrpcResult<pb::PutArtifactChunkResponse> {
        authorized_artifact_upload!(
            self,
            request,
            PutArtifactChunk,
            request.get_ref().upload_id,
            self.handle_put_artifact_chunk(request)
            )
        }
        async fn commit_artifact_upload(
            &self,
            request: Request<pb::CommitArtifactUploadRequest>,
        ) -> GrpcResult<pb::CommitArtifactUploadResponse> {
        authorized_artifact_upload!(
            self,
            request,
            CommitArtifactUpload,
            request.get_ref().upload_id,
            self.handle_commit_artifact_upload(request)
            )
        }
        async fn abort_artifact_upload(
            &self,
            request: Request<pb::AbortArtifactUploadRequest>,
        ) -> GrpcResult<pb::AbortArtifactUploadResponse> {
        authorized_artifact_upload!(
            self,
            request,
            AbortArtifactUpload,
            request.get_ref().upload_id,
            self.handle_abort_artifact_upload(request)
            )
        }
        async fn get_artifact(
            &self,
            request: Request<pb::GetArtifactRequest>,
        ) -> GrpcResult<pb::GetArtifactResponse> {
        authorized_artifact!(
            self,
            request,
            GetArtifact,
            request.get_ref().artifact_id,
            self.handle_get_artifact(request)
            )
        }
        async fn list_artifacts(
            &self,
            request: Request<pb::ListArtifactsRequest>,
        ) -> GrpcResult<pb::ListArtifactsResponse> {
            authorized_global!(
                self,
                request,
                ListArtifacts,
                self.handle_list_artifacts(request)
            )
        }
        async fn read_artifact_chunk(
            &self,
            request: Request<pb::ReadArtifactChunkRequest>,
        ) -> GrpcResult<pb::ReadArtifactChunkResponse> {
        authorized_artifact!(
            self,
            request,
            ReadArtifactChunk,
            request.get_ref().artifact_id,
            self.handle_read_artifact_chunk(request)
            )
        }
        async fn tombstone_artifact(
            &self,
            request: Request<pb::TombstoneArtifactRequest>,
        ) -> GrpcResult<pb::TombstoneArtifactResponse> {
        authorized_artifact!(
            self,
            request,
            TombstoneArtifact,
            request.get_ref().artifact_id,
            self.handle_tombstone_artifact(request)
            )
        }
        async fn publish_protocol_definition(
            &self,
            request: Request<pb::PublishProtocolDefinitionRequest>,
        ) -> GrpcResult<pb::PublishProtocolDefinitionResponse> {
            authorized_definition!(
                self,
                request,
                PublishProtocolDefinition,
                self.handle_publish_protocol_definition(request)
            )
        }
        async fn bind_protocol_participants(
            &self,
            request: Request<pb::BindProtocolParticipantsRequest>,
        ) -> GrpcResult<pb::BindProtocolParticipantsResponse> {
            let trace = trace_context_from_metadata(&request);
            authorized_protocol!(
                self,
                request,
                BindProtocolParticipants,
                run_with_protocol_trace(trace, self.handle_bind_protocol_participants(request))
            )
        }
        async fn diff_protocol_definitions(
            &self,
            request: Request<pb::DiffProtocolDefinitionsRequest>,
        ) -> GrpcResult<pb::DiffProtocolDefinitionsResponse> {
            authorized_global!(
                self,
                request,
                DiffProtocolDefinitions,
                self.handle_diff_protocol_definitions(request)
            )
        }
        async fn list_protocol_definitions(
            &self,
            request: Request<pb::ListProtocolDefinitionsRequest>,
        ) -> GrpcResult<pb::ListProtocolDefinitionsResponse> {
            authorized_global!(
                self,
                request,
                ListProtocolDefinitions,
                self.handle_list_protocol_definitions(request)
            )
        }
        async fn get_protocol_definition(
            &self,
            request: Request<pb::GetProtocolDefinitionRequest>,
        ) -> GrpcResult<pb::GetProtocolDefinitionResponse> {
            authorized_named_definition!(
                self,
                request,
                GetProtocolDefinition,
                self.handle_get_protocol_definition(request)
            )
        }
        async fn get_protocol_instance(
            &self,
            request: Request<pb::GetProtocolInstanceRequest>,
        ) -> GrpcResult<pb::GetProtocolInstanceResponse> {
            authorized_protocol!(
                self,
                request,
                GetProtocolInstance,
                self.handle_get_protocol_instance(request)
            )
        }
        async fn list_protocol_agents(
            &self,
            request: Request<pb::ListProtocolAgentsRequest>,
        ) -> GrpcResult<pb::ListProtocolAgentsResponse> {
            authorized_protocol!(
                self,
                request,
                ListProtocolAgents,
                self.handle_list_protocol_agents(request)
            )
        }
        async fn get_protocol_agent(
            &self,
            request: Request<pb::GetProtocolAgentRequest>,
        ) -> GrpcResult<pb::GetProtocolAgentResponse> {
            authorized_protocol!(
                self,
                request,
                GetProtocolAgent,
                self.handle_get_protocol_agent(request)
            )
        }
        async fn report_protocol_agent_status(
            &self,
            request: Request<pb::ReportProtocolAgentStatusRequest>,
        ) -> GrpcResult<pb::ReportProtocolAgentStatusResponse> {
            let protocol_id = request
                .get_ref()
                .status
                .as_ref()
                .map(|status| status.protocol_id.clone())
                .unwrap_or_default();
            let authorization = self
                .authorize_protocol(&request, AuthorizationAction::ReportProtocolAgentStatus, &protocol_id)
                .await?;
            AuthorizationOperationScope::run(
                authorization,
                self.handle_report_protocol_agent_status(request),
            )
            .await
        }
        async fn list_protocol_instances(
            &self,
            request: Request<pb::ListProtocolInstancesRequest>,
        ) -> GrpcResult<pb::ListProtocolInstancesResponse> {
            authorized_global!(
                self,
                request,
                ListProtocolInstances,
                self.handle_list_protocol_instances(request)
            )
        }
        async fn search_protocol_instances(
            &self,
            request: Request<pb::SearchProtocolInstancesRequest>,
        ) -> GrpcResult<pb::SearchProtocolInstancesResponse> {
            authorized_global!(
                self,
                request,
                SearchProtocolInstances,
                self.handle_search_protocol_instances(request)
            )
        }
        async fn get_status(
            &self,
            request: Request<pb::GetStatusRequest>,
        ) -> GrpcResult<pb::GetStatusResponse> {
            authorized_global!(self, request, GetStatus, self.handle_get_status(request))
        }
        async fn get_metrics(
            &self,
            request: Request<pb::GetMetricsRequest>,
        ) -> GrpcResult<pb::GetMetricsResponse> {
            authorized_global!(self, request, GetMetrics, self.handle_get_metrics(request))
        }
        });
    };
}

pub(super) use resource_rpc_methods;
