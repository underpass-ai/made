use super::{
    apply_protocol_transition_input_from_proto, approve_protocol_guard_input_from_proto,
    assert_protocol_reason_input_from_proto, child_completion_state_from,
    close_protocol_intervention_input_from_proto, collect_protocol_evidence_input_from_proto,
    debug, defer_protocol_guard_input_from_proto, domain_error_to_status, link_span_to_metadata,
    pb, request_protocol_intervention_input_from_proto,
    respond_to_protocol_intervention_input_from_proto, run_protocol_input_from_proto,
    run_protocol_response_from, run_protocol_step_input_from_proto, start_protocol_from_proto,
    GrpcResult, MadeGrpcService, ProtocolId, ProtocolParticipantPlanAdapter, Request, Response,
    StartProtocolFromYaml,
};
use made_app::usecases::AcceptChildCompletionInput;
use made_core::value_objects::{EventId, ProtocolEventPageLimit};

mod budgeted_prepare;
mod budgeted_start;

impl MadeGrpcService {
    #[tracing::instrument(name = "rpc.run_protocol", skip_all)]
    pub(super) async fn handle_run_protocol(
        &self,
        request: Request<pb::RunProtocolRequest>,
    ) -> GrpcResult<pb::RunProtocolResponse> {
        link_span_to_metadata(&request);
        let input =
            run_protocol_input_from_proto(request.into_inner()).map_err(domain_error_to_status)?;
        let participant_input = ProtocolParticipantPlanAdapter::from_definition(input.definition())
            .map_err(domain_error_to_status)?;
        self.prepare_protocol_participants
            .execute(participant_input)
            .await
            .map_err(domain_error_to_status)?;
        let output = self
            .run_protocol
            .execute(input)
            .await
            .map_err(domain_error_to_status)?;
        debug!(
            protocol_id = output.instance().id().as_str(),
            final_state = output.instance().current_state().as_str(),
            completed = output.instance().is_completed(output.definition()),
            "run_protocol rpc ok"
        );
        Ok(Response::new(run_protocol_response_from(&output)))
    }

    #[tracing::instrument(name = "rpc.start_protocol", skip_all)]
    pub(super) async fn handle_start_protocol(
        &self,
        request: Request<pb::StartProtocolRequest>,
    ) -> GrpcResult<pb::StartProtocolResponse> {
        link_span_to_metadata(&request);
        let StartProtocolFromYaml { definition, input } =
            start_protocol_from_proto(request.into_inner()).map_err(domain_error_to_status)?;
        // A session started from supplied YAML has to be able to find
        // its definition again on the next call, which may well land
        // on a different process.
        self.protocol_definitions
            .save(&definition)
            .await
            .map_err(domain_error_to_status)?;
        self.prepare_participants(&definition).await?;
        let instance = self
            .start_protocol
            .execute(input)
            .await
            .map_err(domain_error_to_status)?;
        Ok(Response::new(pb::StartProtocolResponse {
            instance: Some(self.render(&instance, &definition).await?),
        }))
    }

    #[tracing::instrument(name = "rpc.start_published_protocol", skip_all)]
    pub(super) async fn handle_start_published_protocol(
        &self,
        request: Request<pb::StartPublishedProtocolRequest>,
    ) -> GrpcResult<pb::StartPublishedProtocolResponse> {
        link_span_to_metadata(&request);
        let instance = budgeted_start::execute(self, request.into_inner()).await?;
        // Resolved rather than read from the publication directly: the
        // instance records a digest, and resolving it back through the
        // same path every other RPC uses is what proves the digest it
        // recorded still matches what is published.
        let definition = self
            .resolve_protocol_definition
            .execute(&instance)
            .await
            .map_err(domain_error_to_status)?;
        self.prepare_participants(&definition).await?;
        Ok(Response::new(pb::StartPublishedProtocolResponse {
            instance: Some(self.render(&instance, &definition).await?),
        }))
    }

    #[tracing::instrument(name = "rpc.run_protocol_step", skip_all)]
    pub(super) async fn handle_run_protocol_step(
        &self,
        request: Request<pb::RunProtocolStepRequest>,
    ) -> GrpcResult<pb::RunProtocolStepResponse> {
        link_span_to_metadata(&request);
        let request = request.into_inner();
        let protocol_id =
            ProtocolId::new(request.protocol_id.clone()).map_err(domain_error_to_status)?;
        let (instance, definition) = self.session(&protocol_id).await?;
        let input = run_protocol_step_input_from_proto(request, &definition, &instance)
            .map_err(domain_error_to_status)?;
        let output = self
            .run_protocol_step
            .execute(input)
            .await
            .map_err(domain_error_to_status)?;
        Ok(Response::new(pb::RunProtocolStepResponse {
            instance: Some(self.render(output.instance(), &definition).await?),
        }))
    }

    #[tracing::instrument(name = "rpc.accept_child_completion", skip_all)]
    pub(super) async fn handle_accept_child_completion(
        &self,
        request: Request<pb::AcceptChildCompletionRequest>,
    ) -> GrpcResult<pb::AcceptChildCompletionResponse> {
        link_span_to_metadata(&request);
        let request = request.into_inner();
        let output = self
            .accept_child_completion
            .execute(AcceptChildCompletionInput::new(
                ProtocolId::new(request.child_id).map_err(domain_error_to_status)?,
                EventId::new(request.terminal_event_id).map_err(domain_error_to_status)?,
            ))
            .await
            .map_err(domain_error_to_status)?;
        let definition = self
            .resolve_protocol_definition
            .execute(output.parent())
            .await
            .map_err(domain_error_to_status)?;
        Ok(Response::new(pb::AcceptChildCompletionResponse {
            parent: Some(self.render(output.parent(), &definition).await?),
            completion: Some(child_completion_state_from(output.completion())),
        }))
    }

    #[tracing::instrument(name = "rpc.recover_protocol_children", skip_all)]
    pub(super) async fn handle_recover_protocol_children(
        &self,
        request: Request<pb::RecoverProtocolChildrenRequest>,
    ) -> GrpcResult<pb::RecoverProtocolChildrenResponse> {
        link_span_to_metadata(&request);
        let requested = request.into_inner().limit;
        let limit = if requested == 0 {
            ProtocolEventPageLimit::DEFAULT
        } else {
            ProtocolEventPageLimit::new(requested as usize).map_err(domain_error_to_status)?
        };
        let round = self
            .recover_protocol_children
            .execute(limit)
            .await
            .map_err(domain_error_to_status)?;
        Ok(Response::new(pb::RecoverProtocolChildrenResponse {
            recovered_plans: u32::try_from(round.recovered_plans).unwrap_or(u32::MAX),
            accepted_completions: u32::try_from(round.accepted_completions).unwrap_or(u32::MAX),
            skipped: u32::try_from(round.skipped).unwrap_or(u32::MAX),
            failed: u32::try_from(round.failed).unwrap_or(u32::MAX),
            busy: round.busy,
        }))
    }

    #[tracing::instrument(name = "rpc.apply_protocol_transition", skip_all)]
    pub(super) async fn handle_apply_protocol_transition(
        &self,
        request: Request<pb::ApplyProtocolTransitionRequest>,
    ) -> GrpcResult<pb::ApplyProtocolTransitionResponse> {
        link_span_to_metadata(&request);
        let request = request.into_inner();
        let protocol_id =
            ProtocolId::new(request.protocol_id.clone()).map_err(domain_error_to_status)?;
        let (instance, definition) = self.session(&protocol_id).await?;
        let input = apply_protocol_transition_input_from_proto(request, &definition, &instance)
            .map_err(domain_error_to_status)?;
        let moved = self
            .apply_protocol_transition
            .execute(input)
            .await
            .map_err(domain_error_to_status)?;
        Ok(Response::new(pb::ApplyProtocolTransitionResponse {
            instance: Some(self.render(&moved, &definition).await?),
        }))
    }

    // Each of these answers with the session, like every other move.
    // The definition is resolved once per call rather than carried
    // over from a previous one: these are the calls a person makes,
    // minutes or hours apart, and nothing says the process handling
    // this one saw the last.

    #[tracing::instrument(name = "rpc.approve_protocol_guard", skip_all)]
    pub(super) async fn handle_approve_protocol_guard(
        &self,
        request: Request<pb::ApproveProtocolGuardRequest>,
    ) -> GrpcResult<pb::ApproveProtocolGuardResponse> {
        link_span_to_metadata(&request);
        let input = approve_protocol_guard_input_from_proto(request.into_inner())
            .map_err(domain_error_to_status)?;
        let instance = self
            .approve_protocol_guard
            .execute(input)
            .await
            .map_err(domain_error_to_status)?;
        let state = self.project(&instance).await?;
        Ok(Response::new(pb::ApproveProtocolGuardResponse {
            instance: Some(state),
        }))
    }

    #[tracing::instrument(name = "rpc.defer_protocol_guard", skip_all)]
    pub(super) async fn handle_defer_protocol_guard(
        &self,
        request: Request<pb::DeferProtocolGuardRequest>,
    ) -> GrpcResult<pb::DeferProtocolGuardResponse> {
        link_span_to_metadata(&request);
        let input = defer_protocol_guard_input_from_proto(request.into_inner())
            .map_err(domain_error_to_status)?;
        let instance = self
            .defer_protocol_guard
            .execute(input)
            .await
            .map_err(domain_error_to_status)?;
        let state = self.project(&instance).await?;
        Ok(Response::new(pb::DeferProtocolGuardResponse {
            instance: Some(state),
        }))
    }

    #[tracing::instrument(name = "rpc.assert_protocol_reason", skip_all)]
    pub(super) async fn handle_assert_protocol_reason(
        &self,
        request: Request<pb::AssertProtocolReasonRequest>,
    ) -> GrpcResult<pb::AssertProtocolReasonResponse> {
        link_span_to_metadata(&request);
        let input = assert_protocol_reason_input_from_proto(request.into_inner())
            .map_err(domain_error_to_status)?;
        let instance = self
            .assert_protocol_reason
            .execute(input)
            .await
            .map_err(domain_error_to_status)?;
        let state = self.project(&instance).await?;
        Ok(Response::new(pb::AssertProtocolReasonResponse {
            instance: Some(state),
        }))
    }

    #[tracing::instrument(name = "rpc.request_protocol_intervention", skip_all)]
    pub(super) async fn handle_request_protocol_intervention(
        &self,
        request: Request<pb::RequestProtocolInterventionRequest>,
    ) -> GrpcResult<pb::RequestProtocolInterventionResponse> {
        link_span_to_metadata(&request);
        let input = request_protocol_intervention_input_from_proto(request.into_inner())
            .map_err(domain_error_to_status)?;
        let instance = self
            .request_protocol_intervention
            .execute(input)
            .await
            .map_err(domain_error_to_status)?;
        let state = self.project(&instance).await?;
        Ok(Response::new(pb::RequestProtocolInterventionResponse {
            instance: Some(state),
        }))
    }

    #[tracing::instrument(name = "rpc.respond_to_protocol_intervention", skip_all)]
    pub(super) async fn handle_respond_to_protocol_intervention(
        &self,
        request: Request<pb::RespondToProtocolInterventionRequest>,
    ) -> GrpcResult<pb::RespondToProtocolInterventionResponse> {
        link_span_to_metadata(&request);
        let input = respond_to_protocol_intervention_input_from_proto(request.into_inner())
            .map_err(domain_error_to_status)?;
        let instance = self
            .respond_to_protocol_intervention
            .execute(input)
            .await
            .map_err(domain_error_to_status)?;
        let state = self.project(&instance).await?;
        Ok(Response::new(pb::RespondToProtocolInterventionResponse {
            instance: Some(state),
        }))
    }

    #[tracing::instrument(name = "rpc.close_protocol_intervention", skip_all)]
    pub(super) async fn handle_close_protocol_intervention(
        &self,
        request: Request<pb::CloseProtocolInterventionRequest>,
    ) -> GrpcResult<pb::CloseProtocolInterventionResponse> {
        link_span_to_metadata(&request);
        let input = close_protocol_intervention_input_from_proto(request.into_inner())
            .map_err(domain_error_to_status)?;
        let instance = self
            .close_protocol_intervention
            .execute(input)
            .await
            .map_err(domain_error_to_status)?;
        let state = self.project(&instance).await?;
        Ok(Response::new(pb::CloseProtocolInterventionResponse {
            instance: Some(state),
        }))
    }

    #[tracing::instrument(name = "rpc.collect_protocol_evidence", skip_all)]
    pub(super) async fn handle_collect_protocol_evidence(
        &self,
        request: Request<pb::CollectProtocolEvidenceRequest>,
    ) -> GrpcResult<pb::CollectProtocolEvidenceResponse> {
        link_span_to_metadata(&request);
        let input = collect_protocol_evidence_input_from_proto(request.into_inner())
            .map_err(domain_error_to_status)?;
        let instance = self
            .collect_protocol_evidence
            .execute(input)
            .await
            .map_err(domain_error_to_status)?;
        let state = self.project(&instance).await?;
        Ok(Response::new(pb::CollectProtocolEvidenceResponse {
            instance: Some(state),
        }))
    }
}
