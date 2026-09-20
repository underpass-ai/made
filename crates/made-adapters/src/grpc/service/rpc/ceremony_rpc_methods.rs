macro_rules! ceremony_rpc_methods {
    ($callback:ident; { $($methods:tt)* }) => {
        resource_rpc_methods!($callback; {
            $($methods)*
        async fn run_ceremony(
            &self,
            request: Request<pb::RunCeremonyRequest>,
        ) -> GrpcResult<pb::RunCeremonyResponse> {
            let trace = trace_context_from_metadata(&request);
            authorized_ceremony!(
                self,
                request,
                RunCeremony,
                run_with_ceremony_trace(trace, self.handle_run_ceremony(request))
            )
        }
        async fn start_ceremony(
            &self,
            request: Request<pb::StartCeremonyRequest>,
        ) -> GrpcResult<pb::StartCeremonyResponse> {
            let trace = trace_context_from_metadata(&request);
            authorized_ceremony!(
                self,
                request,
                StartCeremony,
                run_with_ceremony_trace(trace, self.handle_start_ceremony(request))
            )
        }
        async fn start_published_ceremony(
            &self,
            request: Request<pb::StartPublishedCeremonyRequest>,
        ) -> GrpcResult<pb::StartPublishedCeremonyResponse> {
            let trace = trace_context_from_metadata(&request);
            authorized_ceremony!(
                self,
                request,
                StartPublishedCeremony,
                run_with_ceremony_trace(trace, self.handle_start_published_ceremony(request))
            )
        }
        async fn run_ceremony_step(
            &self,
            request: Request<pb::RunCeremonyStepRequest>,
        ) -> GrpcResult<pb::RunCeremonyStepResponse> {
            let trace = trace_context_from_metadata(&request);
            authorized_ceremony!(
                self,
                request,
                RunCeremonyStep,
                run_with_ceremony_trace(trace, self.handle_run_ceremony_step(request))
            )
        }
        async fn prepare_ceremony_children(
            &self,
            request: Request<pb::PrepareCeremonyChildrenRequest>,
        ) -> GrpcResult<pb::PrepareCeremonyChildrenResponse> {
            let trace = trace_context_from_metadata(&request);
            authorized_ceremony!(
                self,
                request,
                PrepareCeremonyChildren,
                run_with_ceremony_trace(trace, self.handle_prepare_ceremony_children(request))
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
                run_with_ceremony_trace(trace, self.handle_accept_child_completion(request)),
            )
            .await
        }
        async fn recover_ceremony_children(
            &self,
            request: Request<pb::RecoverCeremonyChildrenRequest>,
        ) -> GrpcResult<pb::RecoverCeremonyChildrenResponse> {
            let trace = trace_context_from_metadata(&request);
            authorized_global!(
                self,
                request,
                RecoverCeremonyChildren,
                run_with_ceremony_trace(trace, self.handle_recover_ceremony_children(request))
            )
        }
        async fn claim_ceremony_step(
            &self,
            request: Request<pb::ClaimCeremonyStepRequest>,
        ) -> GrpcResult<pb::ClaimCeremonyStepResponse> {
            let trace = trace_context_from_metadata(&request);
            authorized_ceremony!(
                self,
                request,
                ClaimCeremonyStep,
                run_with_ceremony_trace(trace, self.handle_claim_ceremony_step(request))
            )
        }
        async fn renew_ceremony_step_lease(
            &self,
            request: Request<pb::RenewCeremonyStepLeaseRequest>,
        ) -> GrpcResult<pb::RenewCeremonyStepLeaseResponse> {
            let trace = trace_context_from_metadata(&request);
            authorized_ceremony!(self, request, RenewCeremonyStepLease,
                run_with_ceremony_trace(trace, self.handle_renew_ceremony_step_lease(request)))
        }
        async fn complete_ceremony_step(
            &self,
            request: Request<pb::CompleteCeremonyStepRequest>,
        ) -> GrpcResult<pb::CompleteCeremonyStepResponse> {
            let trace = trace_context_from_metadata(&request);
            let ceremony_id = request.get_ref().ceremony_id.clone();
            let authorization = match self
                .authorize_ceremony(
                    &request,
                    AuthorizationAction::CompleteCeremonyStep,
                    &ceremony_id,
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
                run_with_ceremony_trace(trace, self.handle_complete_ceremony_step(request)),
            )
            .await
        }
        async fn apply_ceremony_transition(
            &self,
            request: Request<pb::ApplyCeremonyTransitionRequest>,
        ) -> GrpcResult<pb::ApplyCeremonyTransitionResponse> {
            let trace = trace_context_from_metadata(&request);
            authorized_ceremony!(
                self,
                request,
                ApplyCeremonyTransition,
                run_with_ceremony_trace(trace, self.handle_apply_ceremony_transition(request))
            )
        }
        async fn pause_ceremony(
            &self,
            request: Request<pb::PauseCeremonyRequest>,
        ) -> GrpcResult<pb::PauseCeremonyResponse> {
            let trace = trace_context_from_metadata(&request);
            authorized_ceremony!(
                self,
                request,
                PauseCeremony,
                run_with_ceremony_trace(trace, self.handle_pause_ceremony(request))
            )
        }
        async fn resume_ceremony(
            &self,
            request: Request<pb::ResumeCeremonyRequest>,
        ) -> GrpcResult<pb::ResumeCeremonyResponse> {
            let trace = trace_context_from_metadata(&request);
            authorized_ceremony!(
                self,
                request,
                ResumeCeremony,
                run_with_ceremony_trace(trace, self.handle_resume_ceremony(request))
            )
        }
        async fn cancel_ceremony(
            &self,
            request: Request<pb::CancelCeremonyRequest>,
        ) -> GrpcResult<pb::CancelCeremonyResponse> {
            let trace = trace_context_from_metadata(&request);
            authorized_ceremony!(
                self,
                request,
                CancelCeremony,
                run_with_ceremony_trace(trace, self.handle_cancel_ceremony(request))
            )
        }
        async fn record_ceremony_host_handoff(
            &self, request: Request<pb::RecordCeremonyHostHandoffRequest>,
        ) -> GrpcResult<pb::RecordCeremonyHostHandoffResponse> {
            let trace = trace_context_from_metadata(&request);
            authorized_ceremony!(self, request, RecordCeremonyHostHandoff,
                run_with_ceremony_trace(trace, self.handle_record_ceremony_host_handoff(request)))
        }
        async fn inspect_ceremony_resume(
            &self, request: Request<pb::InspectCeremonyResumeRequest>,
        ) -> GrpcResult<pb::InspectCeremonyResumeResponse> {
            let trace = trace_context_from_metadata(&request);
            authorized_ceremony!(self, request, InspectCeremonyResume,
                run_with_ceremony_trace(trace, self.handle_inspect_ceremony_resume(request)))
        }
        async fn plan_ceremony_successor(
            &self, request: Request<pb::PlanCeremonySuccessorRequest>,
        ) -> GrpcResult<pb::PlanCeremonySuccessorResponse> {
            let trace = trace_context_from_metadata(&request);
            authorized_ceremony!(self, request, PlanCeremonySuccessor,
                run_with_ceremony_trace(trace, self.handle_plan_ceremony_successor(request)))
        }
        async fn start_ceremony_successor(
            &self, request: Request<pb::StartCeremonySuccessorRequest>,
        ) -> GrpcResult<pb::StartCeremonySuccessorResponse> {
            let trace = trace_context_from_metadata(&request);
            authorized_ceremony!(self, request, StartCeremonySuccessor,
                run_with_ceremony_trace(trace, self.handle_start_ceremony_successor(request)))
        }
        async fn enforce_ceremony_deadlines(
            &self,
            request: Request<pb::EnforceCeremonyDeadlinesRequest>,
        ) -> GrpcResult<pb::EnforceCeremonyDeadlinesResponse> {
            let trace = trace_context_from_metadata(&request);
            authorized_ceremony!(
                self,
                request,
                EnforceCeremonyDeadlines,
                run_with_ceremony_trace(trace, self.handle_enforce_ceremony_deadlines(request))
            )
        }
        async fn approve_ceremony_guard(
            &self,
            request: Request<pb::ApproveCeremonyGuardRequest>,
        ) -> GrpcResult<pb::ApproveCeremonyGuardResponse> {
            let trace = trace_context_from_metadata(&request);
            authorized_ceremony!(
                self,
                request,
                ApproveCeremonyGuard,
                run_with_ceremony_trace(trace, self.handle_approve_ceremony_guard(request))
            )
        }
        async fn defer_ceremony_guard(
            &self,
            request: Request<pb::DeferCeremonyGuardRequest>,
        ) -> GrpcResult<pb::DeferCeremonyGuardResponse> {
            let trace = trace_context_from_metadata(&request);
            authorized_ceremony!(
                self,
                request,
                DeferCeremonyGuard,
                run_with_ceremony_trace(trace, self.handle_defer_ceremony_guard(request))
            )
        }
        async fn assert_ceremony_reason(
            &self,
            request: Request<pb::AssertCeremonyReasonRequest>,
        ) -> GrpcResult<pb::AssertCeremonyReasonResponse> {
            let trace = trace_context_from_metadata(&request);
            authorized_ceremony!(
                self,
                request,
                AssertCeremonyReason,
                run_with_ceremony_trace(trace, self.handle_assert_ceremony_reason(request))
            )
        }
        async fn request_ceremony_intervention(
            &self,
            request: Request<pb::RequestCeremonyInterventionRequest>,
        ) -> GrpcResult<pb::RequestCeremonyInterventionResponse> {
            let trace = trace_context_from_metadata(&request);
            authorized_ceremony!(
                self,
                request,
                RequestCeremonyIntervention,
                run_with_ceremony_trace(trace, self.handle_request_ceremony_intervention(request))
            )
        }
        async fn respond_to_ceremony_intervention(
            &self,
            request: Request<pb::RespondToCeremonyInterventionRequest>,
        ) -> GrpcResult<pb::RespondToCeremonyInterventionResponse> {
            let trace = trace_context_from_metadata(&request);
            authorized_ceremony!(
                self,
                request,
                RespondToCeremonyIntervention,
                run_with_ceremony_trace(
                    trace,
                    self.handle_respond_to_ceremony_intervention(request)
                )
            )
        }
        async fn close_ceremony_intervention(
            &self,
            request: Request<pb::CloseCeremonyInterventionRequest>,
        ) -> GrpcResult<pb::CloseCeremonyInterventionResponse> {
            let trace = trace_context_from_metadata(&request);
            authorized_ceremony!(
                self,
                request,
                CloseCeremonyIntervention,
                run_with_ceremony_trace(trace, self.handle_close_ceremony_intervention(request))
            )
        }
        async fn pull_ceremony_agent_interventions(
            &self,
            request: Request<pb::PullCeremonyAgentInterventionsRequest>,
        ) -> GrpcResult<pb::PullCeremonyAgentInterventionsResponse> {
            let trace = trace_context_from_metadata(&request);
            authorized_ceremony!(
                self,
                request,
                PullCeremonyAgentInterventions,
                run_with_ceremony_trace(
                    trace,
                    self.handle_pull_ceremony_agent_interventions(request)
                )
            )
        }
        async fn acknowledge_ceremony_agent_intervention(
            &self,
            request: Request<pb::AcknowledgeCeremonyAgentInterventionRequest>,
        ) -> GrpcResult<pb::AcknowledgeCeremonyAgentInterventionResponse> {
            let trace = trace_context_from_metadata(&request);
            authorized_ceremony!(
                self,
                request,
                AcknowledgeCeremonyAgentIntervention,
                run_with_ceremony_trace(
                    trace,
                    self.handle_acknowledge_ceremony_agent_intervention(request)
                )
            )
        }
        async fn get_ceremony_intervention(
            &self,
            request: Request<pb::GetCeremonyInterventionRequest>,
        ) -> GrpcResult<pb::GetCeremonyInterventionResponse> {
            let trace = trace_context_from_metadata(&request);
            authorized_ceremony!(
                self,
                request,
                GetCeremonyIntervention,
                run_with_ceremony_trace(trace, self.handle_get_ceremony_intervention(request))
            )
        }
        async fn list_ceremony_interventions(
            &self,
            request: Request<pb::ListCeremonyInterventionsRequest>,
        ) -> GrpcResult<pb::ListCeremonyInterventionsResponse> {
            let trace = trace_context_from_metadata(&request);
            authorized_ceremony!(
                self,
                request,
                ListCeremonyInterventions,
                run_with_ceremony_trace(trace, self.handle_list_ceremony_interventions(request))
            )
        }
        async fn collect_ceremony_evidence(
            &self,
            request: Request<pb::CollectCeremonyEvidenceRequest>,
        ) -> GrpcResult<pb::CollectCeremonyEvidenceResponse> {
            let trace = trace_context_from_metadata(&request);
            authorized_ceremony!(
                self,
                request,
                CollectCeremonyEvidence,
                run_with_ceremony_trace(trace, self.handle_collect_ceremony_evidence(request))
            )
        }
        });
    };
}

pub(super) use ceremony_rpc_methods;
