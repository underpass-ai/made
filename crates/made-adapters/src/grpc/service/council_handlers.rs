use super::{
    council_summary_from, debug, deliberate_response_from, descriptor_from_register_request,
    domain_error_to_status, link_span_to_metadata, orchestrate_response_from,
    output_contract_from_proto, output_contract_to_proto, pb,
    run_council_decision_input_from_proto, run_council_decision_response_from, task_from_proto,
    trigger_event_from_proto, AgentId, Arc, CreateCouncilInput, DescriptorError, DomainError,
    GrpcResult, MadeGrpcService, MadeService, OutputContractId, Request, Response, Specialty,
    Status, TaskId,
};

impl MadeGrpcService {
    #[tracing::instrument(name = "rpc.deliberate", skip_all)]
    pub(super) async fn handle_deliberate(
        &self,
        request: Request<pb::DeliberateRequest>,
    ) -> GrpcResult<pb::DeliberateResponse> {
        link_span_to_metadata(&request);
        let task_proto = request
            .into_inner()
            .task
            .ok_or_else(|| Status::invalid_argument("task is required"))?;
        let task = task_from_proto(task_proto).map_err(domain_error_to_status)?;
        let out = self
            .deliberate
            .execute(task)
            .await
            .map_err(domain_error_to_status)?;
        debug!(
            task_id = out.deliberation.task_id().as_str(),
            winner = out.winner_proposal_id.as_str(),
            "deliberate rpc ok"
        );
        Ok(Response::new(deliberate_response_from(&out)))
    }

    #[tracing::instrument(name = "rpc.stream_deliberation", skip_all)]
    pub(super) async fn handle_stream_deliberation(
        &self,
        request: Request<pb::StreamDeliberationRequest>,
    ) -> GrpcResult<<Self as MadeService>::StreamDeliberationStream> {
        link_span_to_metadata(&request);
        let task_proto = request
            .into_inner()
            .task
            .ok_or_else(|| Status::invalid_argument("task is required"))?;
        let task = task_from_proto(task_proto).map_err(domain_error_to_status)?;

        // Bounded channel: backpressure shields the deliberation from
        // unbounded buffering if the client reads slowly, and the
        // observer is a no-op on sink-closed so a slow/disconnected
        // client never deadlocks the use case.
        let (tx, rx) = tokio::sync::mpsc::channel(16);
        let observer: Arc<dyn made_core::ports::DeliberationObserverPort> =
            Arc::new(crate::grpc::stream::ChannelObserver::new(tx.clone()));
        let usecase = self.deliberate.clone();

        tokio::spawn(async move {
            match usecase.execute_with_observer(task, observer).await {
                Ok(out) => {
                    // Final frame carries the winner projection so
                    // clients that only wanted the result can read
                    // exactly one message and close.
                    let response = deliberate_response_from(&out);
                    let winner_result = response.results.first().cloned().unwrap_or_default();
                    let frame = pb::StreamDeliberationResponse {
                        update: Some(pb::DeliberationUpdate {
                            task_id: response.task_id,
                            phase: pb::DeliberationPhase::Completed as i32,
                            emitted_at: None,
                            payload: Some(pb::deliberation_update::Payload::Result(winner_result)),
                        }),
                    };
                    let _ = tx.send(Ok(frame)).await;
                }
                Err(err) => {
                    let _ = tx.send(Err(domain_error_to_status(err))).await;
                }
            }
        });

        Ok(Response::new(tokio_stream::wrappers::ReceiverStream::new(
            rx,
        )))
    }

    #[tracing::instrument(name = "rpc.get_deliberation_result", skip_all)]
    pub(super) async fn handle_get_deliberation_result(
        &self,
        request: Request<pb::GetDeliberationResultRequest>,
    ) -> GrpcResult<pb::GetDeliberationResultResponse> {
        link_span_to_metadata(&request);
        let task_id = TaskId::new(request.into_inner().task_id).map_err(domain_error_to_status)?;
        match self.get_deliberation.execute(&task_id).await {
            Ok(deliberation) => {
                let winner = deliberation
                    .ranking()
                    .first()
                    .cloned()
                    .unwrap_or_else(|| made_core::value_objects::ProposalId::new("_").unwrap());
                let out = made_app::usecases::DeliberateOutput {
                    deliberation,
                    winner_proposal_id: winner,
                };
                Ok(Response::new(pb::GetDeliberationResultResponse {
                    found: true,
                    result: Some(deliberate_response_from(&out)),
                }))
            }
            Err(DomainError::NotFound { .. }) => {
                Ok(Response::new(pb::GetDeliberationResultResponse {
                    found: false,
                    result: None,
                }))
            }
            Err(err) => Err(domain_error_to_status(err)),
        }
    }

    #[tracing::instrument(name = "rpc.orchestrate", skip_all)]
    pub(super) async fn handle_orchestrate(
        &self,
        request: Request<pb::OrchestrateRequest>,
    ) -> GrpcResult<pb::OrchestrateResponse> {
        link_span_to_metadata(&request);
        let req = request.into_inner();
        let task_proto = req
            .task
            .ok_or_else(|| Status::invalid_argument("task is required"))?;
        let task = task_from_proto(task_proto).map_err(domain_error_to_status)?;
        // execution_options flows untouched to the executor adapter.
        let options = crate::grpc::mappers::attributes_from_struct(req.execution_options)
            .map_err(domain_error_to_status)?;
        let out = self
            .orchestrate
            .execute(task, options)
            .await
            .map_err(domain_error_to_status)?;
        Ok(Response::new(orchestrate_response_from(&out)))
    }

    #[tracing::instrument(name = "rpc.create_council", skip_all)]
    pub(super) async fn handle_create_council(
        &self,
        request: Request<pb::CreateCouncilRequest>,
    ) -> GrpcResult<pb::CreateCouncilResponse> {
        link_span_to_metadata(&request);
        let req = request.into_inner();
        // Bound the council size through the domain value object: rejects
        // zero and caps at MAX_NUM_AGENTS, so a hostile `num_agents` can't
        // request a multi-billion-id allocation.
        let num_agents = made_core::value_objects::NumAgents::new(req.num_agents)
            .map_err(domain_error_to_status)?;
        let n = num_agents.get() as usize;
        // The create-council RPC does not carry pre-minted agent ids;
        // we mint one id per slot and expect the caller to have
        // previously registered matching agents through the (future)
        // RegisterAgent RPC or through the composition root.
        let agent_ids: Vec<AgentId> = (0..n)
            .map(|i| AgentId::new(format!("agent-{}-{}", req.specialty, i)))
            .collect::<Result<_, _>>()
            .map_err(domain_error_to_status)?;

        let council_id = made_core::value_objects::CouncilId::new(uuid::Uuid::new_v4().to_string())
            .map_err(domain_error_to_status)?;
        let specialty = Specialty::new(&req.specialty).map_err(domain_error_to_status)?;

        let council = self
            .create_council
            .execute(CreateCouncilInput {
                council_id,
                specialty,
                agents: agent_ids,
            })
            .await
            .map_err(domain_error_to_status)?;

        Ok(Response::new(pb::CreateCouncilResponse {
            council: Some(council_summary_from(&council, vec![])),
        }))
    }

    #[tracing::instrument(name = "rpc.list_councils", skip_all)]
    pub(super) async fn handle_list_councils(
        &self,
        request: Request<pb::ListCouncilsRequest>,
    ) -> GrpcResult<pb::ListCouncilsResponse> {
        link_span_to_metadata(&request);
        let _ = request;
        let councils = self
            .list_councils
            .execute()
            .await
            .map_err(domain_error_to_status)?;
        let summaries = councils
            .iter()
            .map(|c| council_summary_from(c, vec![]))
            .collect();
        Ok(Response::new(pb::ListCouncilsResponse {
            councils: summaries,
        }))
    }

    #[tracing::instrument(name = "rpc.delete_council", skip_all)]
    pub(super) async fn handle_delete_council(
        &self,
        request: Request<pb::DeleteCouncilRequest>,
    ) -> GrpcResult<pb::DeleteCouncilResponse> {
        link_span_to_metadata(&request);
        let specialty =
            Specialty::new(request.into_inner().specialty).map_err(domain_error_to_status)?;
        match self.delete_council.execute(&specialty).await {
            Ok(()) => Ok(Response::new(pb::DeleteCouncilResponse { deleted: true })),
            Err(DomainError::NotFound { .. }) => {
                Ok(Response::new(pb::DeleteCouncilResponse { deleted: false }))
            }
            Err(err) => Err(domain_error_to_status(err)),
        }
    }

    #[tracing::instrument(name = "rpc.register_agent", skip_all)]
    pub(super) async fn handle_register_agent(
        &self,
        request: Request<pb::RegisterAgentRequest>,
    ) -> GrpcResult<pb::RegisterAgentResponse> {
        link_span_to_metadata(&request);
        let descriptor =
            descriptor_from_register_request(request.into_inner()).map_err(|err| match err {
                DescriptorError::MissingAgentSummary => {
                    Status::invalid_argument("agent summary is required")
                }
                DescriptorError::Domain(err) => domain_error_to_status(err),
            })?;
        let id = self
            .register_agent
            .execute(descriptor)
            .await
            .map_err(domain_error_to_status)?;
        Ok(Response::new(pb::RegisterAgentResponse {
            agent_id: id.into_inner(),
        }))
    }

    #[tracing::instrument(name = "rpc.unregister_agent", skip_all)]
    pub(super) async fn handle_unregister_agent(
        &self,
        request: Request<pb::UnregisterAgentRequest>,
    ) -> GrpcResult<pb::UnregisterAgentResponse> {
        link_span_to_metadata(&request);
        let id = AgentId::new(request.into_inner().agent_id).map_err(domain_error_to_status)?;
        match self.unregister_agent.execute(&id).await {
            Ok(()) => Ok(Response::new(pb::UnregisterAgentResponse {
                unregistered: true,
            })),
            Err(DomainError::NotFound { .. }) => Ok(Response::new(pb::UnregisterAgentResponse {
                unregistered: false,
            })),
            Err(err) => Err(domain_error_to_status(err)),
        }
    }

    #[tracing::instrument(name = "rpc.run_council_decision", skip_all)]
    pub(super) async fn handle_run_council_decision(
        &self,
        request: Request<pb::RunCouncilDecisionRequest>,
    ) -> GrpcResult<pb::RunCouncilDecisionResponse> {
        link_span_to_metadata(&request);
        let input =
            run_council_decision_input_from_proto(request.into_inner()).map_err(Status::from)?;
        let output = self
            .run_council_decision
            .execute(input)
            .await
            .map_err(domain_error_to_status)?;
        debug!(
            task_id = output.task_id.as_str(),
            passed = output.passed,
            duration_ms = output.duration_ms.get(),
            "run_council_decision rpc ok"
        );
        Ok(Response::new(run_council_decision_response_from(&output)))
    }

    #[tracing::instrument(name = "rpc.register_contract", skip_all)]
    pub(super) async fn handle_register_contract(
        &self,
        request: Request<pb::RegisterContractRequest>,
    ) -> GrpcResult<pb::RegisterContractResponse> {
        link_span_to_metadata(&request);
        let proto = request
            .into_inner()
            .contract
            .ok_or_else(|| Status::invalid_argument("contract is required"))?;
        let contract = output_contract_from_proto(Some(proto))
            .map_err(domain_error_to_status)?
            .ok_or_else(|| Status::invalid_argument("contract is required"))?;
        let contract_id = contract.contract_id().to_owned();
        self.contract_registry
            .register(contract)
            .await
            .map_err(domain_error_to_status)?;
        Ok(Response::new(pb::RegisterContractResponse {
            contract_id: contract_id.as_str().to_owned(),
        }))
    }

    #[tracing::instrument(name = "rpc.list_contracts", skip_all)]
    pub(super) async fn handle_list_contracts(
        &self,
        request: Request<pb::ListContractsRequest>,
    ) -> GrpcResult<pb::ListContractsResponse> {
        link_span_to_metadata(&request);
        let _ = request;
        let contracts = self
            .contract_registry
            .list()
            .await
            .map_err(domain_error_to_status)?;
        Ok(Response::new(pb::ListContractsResponse {
            contracts: contracts.iter().map(output_contract_to_proto).collect(),
        }))
    }

    #[tracing::instrument(name = "rpc.delete_contract", skip_all)]
    pub(super) async fn handle_delete_contract(
        &self,
        request: Request<pb::DeleteContractRequest>,
    ) -> GrpcResult<pb::DeleteContractResponse> {
        link_span_to_metadata(&request);
        let contract_id = OutputContractId::new(request.into_inner().contract_id)
            .map_err(domain_error_to_status)?;
        match self.contract_registry.delete(&contract_id).await {
            Ok(()) => Ok(Response::new(pb::DeleteContractResponse { deleted: true })),
            Err(DomainError::NotFound { .. }) => {
                Ok(Response::new(pb::DeleteContractResponse { deleted: false }))
            }
            Err(err) => Err(domain_error_to_status(err)),
        }
    }

    #[tracing::instrument(name = "rpc.process_trigger_event", skip_all)]
    pub(super) async fn handle_process_trigger_event(
        &self,
        request: Request<pb::ProcessTriggerEventRequest>,
    ) -> GrpcResult<pb::ProcessTriggerEventResponse> {
        link_span_to_metadata(&request);
        let inner = request.into_inner();
        let ev_proto = inner
            .event
            .ok_or_else(|| Status::invalid_argument("event is required"))?;
        let trigger = trigger_event_from_proto(ev_proto, time::OffsetDateTime::now_utc())
            .map_err(domain_error_to_status)?;

        let outcome = self
            .auto_dispatch
            .dispatch(&trigger)
            .await
            .map_err(domain_error_to_status)?;

        Ok(Response::new(pb::ProcessTriggerEventResponse {
            ack: Some(pb::TriggerAck {
                event_id: trigger.envelope().event_id().as_str().to_owned(),
                accepted: outcome.accepted(),
                dispatched_task_ids: outcome
                    .dispatched_task_ids()
                    .iter()
                    .map(|id| id.as_str().to_owned())
                    .collect(),
                reason: if outcome.accepted() {
                    String::new()
                } else {
                    "no specialties produced a deliberation".to_owned()
                },
            }),
        }))
    }
}
