macro_rules! resource_rpc_methods {
    ($callback:ident; { $($methods:tt)* }) => {
        $callback!({
            $($methods)*
        async fn validate_ceremony_draft(
            &self,
            request: Request<pb::ValidateCeremonyDraftRequest>,
        ) -> GrpcResult<pb::ValidateCeremonyDraftResponse> {
            authorized_global!(
                self,
                request,
                ValidateCeremonyDraft,
                self.handle_validate_ceremony_draft(request)
            )
        }
        async fn explain_ceremony_draft(
            &self,
            request: Request<pb::ExplainCeremonyDraftRequest>,
        ) -> GrpcResult<pb::ExplainCeremonyDraftResponse> {
            authorized_global!(
                self,
                request,
                ExplainCeremonyDraft,
                self.handle_explain_ceremony_draft(request)
            )
        }
        async fn design_ceremony(
            &self,
            request: Request<pb::DesignCeremonyRequest>,
        ) -> GrpcResult<pb::DesignCeremonyResponse> {
            authorized_global!(
                self,
                request,
                DesignCeremony,
                self.handle_design_ceremony(request)
            )
        }
        async fn read_ceremony_events(
            &self,
            request: Request<pb::ReadCeremonyEventsRequest>,
        ) -> GrpcResult<pb::ReadCeremonyEventsResponse> {
            authorized_ceremony!(
                self,
                request,
                ReadCeremonyEvents,
                self.handle_read_ceremony_events(request)
            )
        }
        async fn stream_ceremony(
            &self,
            request: Request<pb::StreamCeremonyRequest>,
        ) -> GrpcResult<Self::StreamCeremonyStream> {
            authorized_ceremony!(
                self,
                request,
                StreamCeremony,
                self.handle_stream_ceremony(request)
            )
        }
        async fn pull_ceremony_events(
            &self,
            request: Request<pb::PullCeremonyEventsRequest>,
        ) -> GrpcResult<pb::PullCeremonyEventsResponse> {
            authorized_global!(
                self,
                request,
                PullCeremonyEvents,
                self.handle_pull_ceremony_events(request)
            )
        }
        async fn verify_ceremony_journal(
            &self,
            request: Request<pb::VerifyCeremonyJournalRequest>,
        ) -> GrpcResult<pb::VerifyCeremonyJournalResponse> {
            authorized_ceremony!(
                self,
                request,
                VerifyCeremonyJournal,
                self.handle_verify_ceremony_journal(request)
            )
        }
        async fn get_ceremony_transcript(
            &self,
            request: Request<pb::GetCeremonyTranscriptRequest>,
        ) -> GrpcResult<pb::GetCeremonyTranscriptResponse> {
            authorized_ceremony!(
                self,
                request,
                GetCeremonyTranscript,
                self.handle_get_ceremony_transcript(request)
            )
        }
        async fn generate_ceremony_report(
            &self,
            request: Request<pb::GenerateCeremonyReportRequest>,
        ) -> GrpcResult<pb::GenerateCeremonyReportResponse> {
            authorized_global!(
                self,
                request,
                GenerateCeremonyReport,
                self.handle_generate_ceremony_report(request)
            )
        }
        async fn get_budget_report(
            &self,
            request: Request<pb::GetBudgetReportRequest>,
        ) -> GrpcResult<pb::GetBudgetReportResponse> {
        authorized_budget_for_ceremony!(
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
        async fn publish_ceremony_definition(
            &self,
            request: Request<pb::PublishCeremonyDefinitionRequest>,
        ) -> GrpcResult<pb::PublishCeremonyDefinitionResponse> {
            authorized_global!(
                self,
                request,
                PublishCeremonyDefinition,
                self.handle_publish_ceremony_definition(request)
            )
        }
        async fn bind_ceremony_participants(
            &self,
            request: Request<pb::BindCeremonyParticipantsRequest>,
        ) -> GrpcResult<pb::BindCeremonyParticipantsResponse> {
            let trace = trace_context_from_metadata(&request);
            authorized_ceremony!(
                self,
                request,
                BindCeremonyParticipants,
                run_with_ceremony_trace(trace, self.handle_bind_ceremony_participants(request))
            )
        }
        async fn diff_ceremony_definitions(
            &self,
            request: Request<pb::DiffCeremonyDefinitionsRequest>,
        ) -> GrpcResult<pb::DiffCeremonyDefinitionsResponse> {
            authorized_global!(
                self,
                request,
                DiffCeremonyDefinitions,
                self.handle_diff_ceremony_definitions(request)
            )
        }
        async fn get_ceremony_instance(
            &self,
            request: Request<pb::GetCeremonyInstanceRequest>,
        ) -> GrpcResult<pb::GetCeremonyInstanceResponse> {
            authorized_ceremony!(
                self,
                request,
                GetCeremonyInstance,
                self.handle_get_ceremony_instance(request)
            )
        }
        async fn list_ceremony_instances(
            &self,
            request: Request<pb::ListCeremonyInstancesRequest>,
        ) -> GrpcResult<pb::ListCeremonyInstancesResponse> {
            authorized_global!(
                self,
                request,
                ListCeremonyInstances,
                self.handle_list_ceremony_instances(request)
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
