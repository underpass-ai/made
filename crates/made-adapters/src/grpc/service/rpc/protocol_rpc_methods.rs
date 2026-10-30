macro_rules! protocol_rpc_methods {
    ($callback:ident; { $($methods:tt)* }) => {
        integrator_loop_rpc_methods!($callback; {
            $($methods)*
        async fn run_protocol(
            &self,
            request: Request<pb::RunProtocolRequest>,
        ) -> GrpcResult<pb::RunProtocolResponse> {
            let trace = trace_context_from_metadata(&request);
            authorized_protocol!(
                self,
                request,
                RunProtocol,
                run_with_protocol_trace(trace, self.handle_run_protocol(request))
            )
        }
        async fn start_protocol(
            &self,
            request: Request<pb::StartProtocolRequest>,
        ) -> GrpcResult<pb::StartProtocolResponse> {
            let trace = trace_context_from_metadata(&request);
            authorized_protocol!(
                self,
                request,
                StartProtocol,
                run_with_protocol_trace(trace, self.handle_start_protocol(request))
            )
        }
        async fn start_published_protocol(
            &self,
            request: Request<pb::StartPublishedProtocolRequest>,
        ) -> GrpcResult<pb::StartPublishedProtocolResponse> {
            let trace = trace_context_from_metadata(&request);
            authorized_protocol!(
                self,
                request,
                StartPublishedProtocol,
                run_with_protocol_trace(trace, self.handle_start_published_protocol(request))
            )
        }
        async fn run_protocol_step(
            &self,
            request: Request<pb::RunProtocolStepRequest>,
        ) -> GrpcResult<pb::RunProtocolStepResponse> {
            let trace = trace_context_from_metadata(&request);
            authorized_protocol!(
                self,
                request,
                RunProtocolStep,
                run_with_protocol_trace(trace, self.handle_run_protocol_step(request))
            )
        }
        async fn prepare_protocol_children(
            &self,
            request: Request<pb::PrepareProtocolChildrenRequest>,
        ) -> GrpcResult<pb::PrepareProtocolChildrenResponse> {
            let trace = trace_context_from_metadata(&request);
            authorized_protocol!(
                self,
                request,
                PrepareProtocolChildren,
                run_with_protocol_trace(trace, self.handle_prepare_protocol_children(request))
            )
        }
        async fn accept_child_completion(
            &self,
            request: Request<pb::AcceptChildCompletionRequest>,
        ) -> GrpcResult<pb::AcceptChildCompletionResponse> {
            let trace = trace_context_from_metadata(&request);
            let child_id = request.get_ref().child_id.clone();
            let authorization = self
                .authorize_child_parent(
                    &request,
                    AuthorizationAction::AcceptChildCompletion,
                    &child_id,
                )
                .await?;
            AuthorizationOperationScope::run(
                authorization,
                run_with_protocol_trace(trace, self.handle_accept_child_completion(request)),
            )
            .await
        }
        async fn recover_protocol_children(
            &self,
            request: Request<pb::RecoverProtocolChildrenRequest>,
        ) -> GrpcResult<pb::RecoverProtocolChildrenResponse> {
            let trace = trace_context_from_metadata(&request);
            authorized_global!(
                self,
                request,
                RecoverProtocolChildren,
                run_with_protocol_trace(trace, self.handle_recover_protocol_children(request))
            )
        }
        async fn claim_protocol_step(
            &self,
            request: Request<pb::ClaimProtocolStepRequest>,
        ) -> GrpcResult<pb::ClaimProtocolStepResponse> {
            let trace = trace_context_from_metadata(&request);
            authorized_protocol!(
                self,
                request,
                ClaimProtocolStep,
                run_with_protocol_trace(trace, self.handle_claim_protocol_step(request))
            )
        }
        async fn renew_protocol_step_lease(
            &self,
            request: Request<pb::RenewProtocolStepLeaseRequest>,
        ) -> GrpcResult<pb::RenewProtocolStepLeaseResponse> {
            let trace = trace_context_from_metadata(&request);
            authorized_protocol!(self, request, RenewProtocolStepLease,
                run_with_protocol_trace(trace, self.handle_renew_protocol_step_lease(request)))
        }
        async fn complete_protocol_step(
            &self,
            request: Request<pb::CompleteProtocolStepRequest>,
        ) -> GrpcResult<pb::CompleteProtocolStepResponse> {
            let trace = trace_context_from_metadata(&request);
            let protocol_id = request.get_ref().protocol_id.clone();
            let authorization = match self
                .authorize_protocol(
                    &request,
                    AuthorizationAction::CompleteProtocolStep,
                    &protocol_id,
                )
                .await
            {
                Ok(operation) => operation,
                Err(status) if status.code() == tonic::Code::PermissionDenied => {
                    self.continue_accepted_step_completion(&request).await?
                }
                Err(status) => return Err(status),
            };
            AuthorizationOperationScope::run(
                authorization,
                run_with_protocol_trace(trace, self.handle_complete_protocol_step(request)),
            )
            .await
        }
        async fn apply_protocol_transition(
            &self,
            request: Request<pb::ApplyProtocolTransitionRequest>,
        ) -> GrpcResult<pb::ApplyProtocolTransitionResponse> {
            let trace = trace_context_from_metadata(&request);
            authorized_protocol!(
                self,
                request,
                ApplyProtocolTransition,
                run_with_protocol_trace(trace, self.handle_apply_protocol_transition(request))
            )
        }
        async fn pause_protocol(
            &self,
            request: Request<pb::PauseProtocolRequest>,
        ) -> GrpcResult<pb::PauseProtocolResponse> {
            let trace = trace_context_from_metadata(&request);
            authorized_protocol!(
                self,
                request,
                PauseProtocol,
                run_with_protocol_trace(trace, self.handle_pause_protocol(request))
            )
        }
        async fn resume_protocol(
            &self,
            request: Request<pb::ResumeProtocolRequest>,
        ) -> GrpcResult<pb::ResumeProtocolResponse> {
            let trace = trace_context_from_metadata(&request);
            authorized_protocol!(
                self,
                request,
                ResumeProtocol,
                run_with_protocol_trace(trace, self.handle_resume_protocol(request))
            )
        }
        async fn cancel_protocol(
            &self,
            request: Request<pb::CancelProtocolRequest>,
        ) -> GrpcResult<pb::CancelProtocolResponse> {
            let trace = trace_context_from_metadata(&request);
            authorized_protocol!(
                self,
                request,
                CancelProtocol,
                run_with_protocol_trace(trace, self.handle_cancel_protocol(request))
            )
        }
        async fn record_protocol_host_handoff(
            &self, request: Request<pb::RecordProtocolHostHandoffRequest>,
        ) -> GrpcResult<pb::RecordProtocolHostHandoffResponse> {
            let trace = trace_context_from_metadata(&request);
            authorized_protocol!(self, request, RecordProtocolHostHandoff,
                run_with_protocol_trace(trace, self.handle_record_protocol_host_handoff(request)))
        }
        async fn inspect_protocol_resume(
            &self, request: Request<pb::InspectProtocolResumeRequest>,
        ) -> GrpcResult<pb::InspectProtocolResumeResponse> {
            let trace = trace_context_from_metadata(&request);
            authorized_protocol!(self, request, InspectProtocolResume,
                run_with_protocol_trace(trace, self.handle_inspect_protocol_resume(request)))
        }
        async fn plan_protocol_successor(
            &self, request: Request<pb::PlanProtocolSuccessorRequest>,
        ) -> GrpcResult<pb::PlanProtocolSuccessorResponse> {
            let trace = trace_context_from_metadata(&request);
            authorized_protocol!(self, request, PlanProtocolSuccessor,
                run_with_protocol_trace(trace, self.handle_plan_protocol_successor(request)))
        }
        async fn start_protocol_successor(
            &self, request: Request<pb::StartProtocolSuccessorRequest>,
        ) -> GrpcResult<pb::StartProtocolSuccessorResponse> {
            let trace = trace_context_from_metadata(&request);
            authorized_protocol!(self, request, StartProtocolSuccessor,
                run_with_protocol_trace(trace, self.handle_start_protocol_successor(request)))
        }
        async fn enforce_protocol_deadlines(
            &self,
            request: Request<pb::EnforceProtocolDeadlinesRequest>,
        ) -> GrpcResult<pb::EnforceProtocolDeadlinesResponse> {
            let trace = trace_context_from_metadata(&request);
            authorized_protocol!(
                self,
                request,
                EnforceProtocolDeadlines,
                run_with_protocol_trace(trace, self.handle_enforce_protocol_deadlines(request))
            )
        }
        async fn approve_protocol_guard(
            &self,
            request: Request<pb::ApproveProtocolGuardRequest>,
        ) -> GrpcResult<pb::ApproveProtocolGuardResponse> {
            let trace = trace_context_from_metadata(&request);
            authorized_protocol!(
                self,
                request,
                ApproveProtocolGuard,
                run_with_protocol_trace(trace, self.handle_approve_protocol_guard(request))
            )
        }
        async fn defer_protocol_guard(
            &self,
            request: Request<pb::DeferProtocolGuardRequest>,
        ) -> GrpcResult<pb::DeferProtocolGuardResponse> {
            let trace = trace_context_from_metadata(&request);
            authorized_protocol!(
                self,
                request,
                DeferProtocolGuard,
                run_with_protocol_trace(trace, self.handle_defer_protocol_guard(request))
            )
        }
        async fn assert_protocol_reason(
            &self,
            request: Request<pb::AssertProtocolReasonRequest>,
        ) -> GrpcResult<pb::AssertProtocolReasonResponse> {
            let trace = trace_context_from_metadata(&request);
            authorized_protocol!(
                self,
                request,
                AssertProtocolReason,
                run_with_protocol_trace(trace, self.handle_assert_protocol_reason(request))
            )
        }
        async fn request_protocol_intervention(
            &self,
            request: Request<pb::RequestProtocolInterventionRequest>,
        ) -> GrpcResult<pb::RequestProtocolInterventionResponse> {
            let trace = trace_context_from_metadata(&request);
            authorized_protocol!(
                self,
                request,
                RequestProtocolIntervention,
                run_with_protocol_trace(trace, self.handle_request_protocol_intervention(request))
            )
        }
        async fn respond_to_protocol_intervention(
            &self,
            request: Request<pb::RespondToProtocolInterventionRequest>,
        ) -> GrpcResult<pb::RespondToProtocolInterventionResponse> {
            let trace = trace_context_from_metadata(&request);
            authorized_protocol!(
                self,
                request,
                RespondToProtocolIntervention,
                run_with_protocol_trace(
                    trace,
                    self.handle_respond_to_protocol_intervention(request)
                )
            )
        }
        async fn close_protocol_intervention(
            &self,
            request: Request<pb::CloseProtocolInterventionRequest>,
        ) -> GrpcResult<pb::CloseProtocolInterventionResponse> {
            let trace = trace_context_from_metadata(&request);
            authorized_protocol!(
                self,
                request,
                CloseProtocolIntervention,
                run_with_protocol_trace(trace, self.handle_close_protocol_intervention(request))
            )
        }
        async fn pull_protocol_agent_interventions(
            &self,
            request: Request<pb::PullProtocolAgentInterventionsRequest>,
        ) -> GrpcResult<pb::PullProtocolAgentInterventionsResponse> {
            let trace = trace_context_from_metadata(&request);
            authorized_protocol!(
                self,
                request,
                PullProtocolAgentInterventions,
                run_with_protocol_trace(
                    trace,
                    self.handle_pull_protocol_agent_interventions(request)
                )
            )
        }
        async fn acknowledge_protocol_agent_intervention(
            &self,
            request: Request<pb::AcknowledgeProtocolAgentInterventionRequest>,
        ) -> GrpcResult<pb::AcknowledgeProtocolAgentInterventionResponse> {
            let trace = trace_context_from_metadata(&request);
            authorized_protocol!(
                self,
                request,
                AcknowledgeProtocolAgentIntervention,
                run_with_protocol_trace(
                    trace,
                    self.handle_acknowledge_protocol_agent_intervention(request)
                )
            )
        }
        async fn get_protocol_intervention(
            &self,
            request: Request<pb::GetProtocolInterventionRequest>,
        ) -> GrpcResult<pb::GetProtocolInterventionResponse> {
            let trace = trace_context_from_metadata(&request);
            authorized_protocol!(
                self,
                request,
                GetProtocolIntervention,
                run_with_protocol_trace(trace, self.handle_get_protocol_intervention(request))
            )
        }
        async fn list_protocol_interventions(
            &self,
            request: Request<pb::ListProtocolInterventionsRequest>,
        ) -> GrpcResult<pb::ListProtocolInterventionsResponse> {
            let trace = trace_context_from_metadata(&request);
            authorized_protocol!(
                self,
                request,
                ListProtocolInterventions,
                run_with_protocol_trace(trace, self.handle_list_protocol_interventions(request))
            )
        }
        async fn collect_protocol_evidence(
            &self,
            request: Request<pb::CollectProtocolEvidenceRequest>,
        ) -> GrpcResult<pb::CollectProtocolEvidenceResponse> {
            let trace = trace_context_from_metadata(&request);
            authorized_protocol!(
                self,
                request,
                CollectProtocolEvidence,
                run_with_protocol_trace(trace, self.handle_collect_protocol_evidence(request))
            )
        }
        });
    };
}

pub(super) use protocol_rpc_methods;
