macro_rules! council_rpc_methods {
    ($callback:ident) => {
        council_rpc_methods!($callback; {});
    };
    ($callback:ident; { $($methods:tt)* }) => {
        ceremony_rpc_methods!($callback; {
        $($methods)*
        async fn get_execution_receipt(
            &self,
            request: Request<pb::GetExecutionReceiptRequest>,
        ) -> GrpcResult<pb::GetExecutionReceiptResponse> {
            authorized_global!(
                self,
                request,
            GetExecutionReceipt,
                self.handle_get_execution_receipt(request)
            )
        }
        async fn inspect_execution_recovery(
            &self,
            request: Request<pb::InspectExecutionRecoveryRequest>,
        ) -> GrpcResult<pb::InspectExecutionRecoveryResponse> {
            authorized_global!(
                self,
                request,
            InspectExecutionRecovery,
                self.handle_inspect_execution_recovery(request)
            )
        }
        async fn complete_execution_receipt(
            &self,
            request: Request<pb::CompleteExecutionReceiptRequest>,
        ) -> GrpcResult<pb::CompleteExecutionReceiptResponse> {
            let trace = trace_context_from_metadata(&request);
            authorized_global!(
                self,
                request,
            CompleteExecutionReceipt,
                run_with_ceremony_trace(trace, self.handle_complete_execution_receipt(request))
            )
        }
        async fn adopt_execution_receipt(
            &self,
            request: Request<pb::AdoptExecutionReceiptRequest>,
        ) -> GrpcResult<pb::AdoptExecutionReceiptResponse> {
            let trace = trace_context_from_metadata(&request);
            authorized_global!(
                self,
                request,
            AdoptExecutionReceipt,
                run_with_ceremony_trace(trace, self.handle_adopt_execution_receipt(request))
            )
        }
        async fn read_council_events(
            &self,
            request: Request<pb::ReadCouncilEventsRequest>,
        ) -> GrpcResult<pb::ReadCouncilEventsResponse> {
            authorized_global!(
                self,
                request,
                ReadCouncilEvents,
                self.handle_read_council_events(request)
            )
        }
        type StreamCeremonyStream = RpcResultStream<pb::StreamCeremonyResponse>;
        type StreamDeliberationStream = tokio_stream::wrappers::ReceiverStream<
            std::result::Result<pb::StreamDeliberationResponse, Status>,
        >;
        async fn get_council_event_cursor(
            &self,
            request: Request<pb::GetCouncilEventCursorRequest>,
        ) -> GrpcResult<pb::GetCouncilEventCursorResponse> {
            authorized_global!(
                self,
                request,
                GetCouncilEventCursor,
                self.handle_get_council_event_cursor(request)
            )
        }
        async fn lease_council_events(
            &self,
            request: Request<pb::LeaseCouncilEventsRequest>,
        ) -> GrpcResult<pb::LeaseCouncilEventsResponse> {
            authorized_global!(
                self,
                request,
                LeaseCouncilEvents,
                self.handle_lease_council_events(request)
            )
        }
        async fn acknowledge_council_events(
            &self,
            request: Request<pb::AcknowledgeCouncilEventsRequest>,
        ) -> GrpcResult<pb::AcknowledgeCouncilEventsResponse> {
            authorized_global!(
                self,
                request,
                AcknowledgeCouncilEvents,
                self.handle_acknowledge_council_events(request)
            )
        }
        async fn release_council_events(
            &self,
            request: Request<pb::ReleaseCouncilEventsRequest>,
        ) -> GrpcResult<pb::ReleaseCouncilEventsResponse> {
            authorized_global!(
                self,
                request,
                ReleaseCouncilEvents,
                self.handle_release_council_events(request)
            )
        }
        async fn deliberate(
            &self,
            request: Request<pb::DeliberateRequest>,
        ) -> GrpcResult<pb::DeliberateResponse> {
            authorized_global!(self, request, Deliberate, self.handle_deliberate(request))
        }
        async fn stream_deliberation(
            &self,
            request: Request<pb::StreamDeliberationRequest>,
        ) -> GrpcResult<Self::StreamDeliberationStream> {
            authorized_global!(
                self,
                request,
                StreamDeliberation,
                self.handle_stream_deliberation(request)
            )
        }
        async fn get_deliberation_result(
            &self,
            request: Request<pb::GetDeliberationResultRequest>,
        ) -> GrpcResult<pb::GetDeliberationResultResponse> {
            authorized_global!(
                self,
                request,
                GetDeliberationResult,
                self.handle_get_deliberation_result(request)
            )
        }
        async fn orchestrate(
            &self,
            request: Request<pb::OrchestrateRequest>,
        ) -> GrpcResult<pb::OrchestrateResponse> {
            authorized_global!(self, request, Orchestrate, self.handle_orchestrate(request))
        }
        async fn create_council(
            &self,
            request: Request<pb::CreateCouncilRequest>,
        ) -> GrpcResult<pb::CreateCouncilResponse> {
            authorized_global!(
                self,
                request,
                CreateCouncil,
                self.handle_create_council(request)
            )
        }
        async fn list_councils(
            &self,
            request: Request<pb::ListCouncilsRequest>,
        ) -> GrpcResult<pb::ListCouncilsResponse> {
            authorized_global!(
                self,
                request,
                ListCouncils,
                self.handle_list_councils(request)
            )
        }
        async fn delete_council(
            &self,
            request: Request<pb::DeleteCouncilRequest>,
        ) -> GrpcResult<pb::DeleteCouncilResponse> {
            authorized_global!(
                self,
                request,
                DeleteCouncil,
                self.handle_delete_council(request)
            )
        }
        async fn register_agent(
            &self,
            request: Request<pb::RegisterAgentRequest>,
        ) -> GrpcResult<pb::RegisterAgentResponse> {
            authorized_global!(
                self,
                request,
                RegisterAgent,
                self.handle_register_agent(request)
            )
        }
        async fn unregister_agent(
            &self,
            request: Request<pb::UnregisterAgentRequest>,
        ) -> GrpcResult<pb::UnregisterAgentResponse> {
            authorized_global!(
                self,
                request,
                UnregisterAgent,
                self.handle_unregister_agent(request)
            )
        }
        async fn run_council_decision(
            &self,
            request: Request<pb::RunCouncilDecisionRequest>,
        ) -> GrpcResult<pb::RunCouncilDecisionResponse> {
            let authorization = match request.get_ref().selector.as_ref() {
                Some(pb::run_council_decision_request::Selector::CouncilId(council_id)) => {
                    self.authorize_council(
                        &request,
                        AuthorizationAction::RunCouncilDecision,
                        council_id,
                    )
                    .await?
                }
                Some(pb::run_council_decision_request::Selector::Specialty(_)) | None => {
                    self.authorize_global(&request, AuthorizationAction::RunCouncilDecision)
                        .await?
                }
            };
            AuthorizationOperationScope::run(
                authorization,
                self.handle_run_council_decision(request),
            )
            .await
        }
        async fn register_contract(
            &self,
            request: Request<pb::RegisterContractRequest>,
        ) -> GrpcResult<pb::RegisterContractResponse> {
            authorized_global!(
                self,
                request,
                RegisterContract,
                self.handle_register_contract(request)
            )
        }
        async fn list_contracts(
            &self,
            request: Request<pb::ListContractsRequest>,
        ) -> GrpcResult<pb::ListContractsResponse> {
            authorized_global!(
                self,
                request,
                ListContracts,
                self.handle_list_contracts(request)
            )
        }
        async fn delete_contract(
            &self,
            request: Request<pb::DeleteContractRequest>,
        ) -> GrpcResult<pb::DeleteContractResponse> {
            authorized_global!(
                self,
                request,
                DeleteContract,
                self.handle_delete_contract(request)
            )
        }
        async fn process_trigger_event(
            &self,
            request: Request<pb::ProcessTriggerEventRequest>,
        ) -> GrpcResult<pb::ProcessTriggerEventResponse> {
            authorized_global!(
                self,
                request,
                ProcessTriggerEvent,
                self.handle_process_trigger_event(request)
            )
        }
        });
    };
}

pub(super) use council_rpc_methods;
