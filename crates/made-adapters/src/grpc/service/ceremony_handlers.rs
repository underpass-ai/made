use super::{
    apply_ceremony_transition_input_from_proto, approve_ceremony_guard_input_from_proto,
    assert_ceremony_reason_input_from_proto, close_ceremony_intervention_input_from_proto,
    collect_ceremony_evidence_input_from_proto, debug, defer_ceremony_guard_input_from_proto,
    domain_error_to_status, link_span_to_metadata, pb,
    request_ceremony_intervention_input_from_proto,
    respond_to_ceremony_intervention_input_from_proto, run_ceremony_input_from_proto,
    run_ceremony_response_from, run_ceremony_step_input_from_proto, start_ceremony_from_proto,
    start_published_ceremony_input_from_proto, CeremonyId, CeremonyParticipantPlanAdapter,
    GrpcResult, MadeGrpcService, Request, Response, StartCeremonyFromYaml,
};

impl MadeGrpcService {
    #[tracing::instrument(name = "rpc.run_ceremony", skip_all)]
    pub(super) async fn handle_run_ceremony(
        &self,
        request: Request<pb::RunCeremonyRequest>,
    ) -> GrpcResult<pb::RunCeremonyResponse> {
        link_span_to_metadata(&request);
        let input =
            run_ceremony_input_from_proto(request.into_inner()).map_err(domain_error_to_status)?;
        let participant_input = CeremonyParticipantPlanAdapter::from_definition(input.definition())
            .map_err(domain_error_to_status)?;
        self.prepare_ceremony_participants
            .execute(participant_input)
            .await
            .map_err(domain_error_to_status)?;
        let output = self
            .run_ceremony
            .execute(input)
            .await
            .map_err(domain_error_to_status)?;
        debug!(
            ceremony_id = output.instance().id().as_str(),
            final_state = output.instance().current_state().as_str(),
            completed = output.instance().is_completed(output.definition()),
            "run_ceremony rpc ok"
        );
        Ok(Response::new(run_ceremony_response_from(&output)))
    }

    #[tracing::instrument(name = "rpc.start_ceremony", skip_all)]
    pub(super) async fn handle_start_ceremony(
        &self,
        request: Request<pb::StartCeremonyRequest>,
    ) -> GrpcResult<pb::StartCeremonyResponse> {
        link_span_to_metadata(&request);
        let StartCeremonyFromYaml { definition, input } =
            start_ceremony_from_proto(request.into_inner()).map_err(domain_error_to_status)?;
        // A session started from supplied YAML has to be able to find
        // its definition again on the next call, which may well land
        // on a different process.
        self.ceremony_definitions
            .save(&definition)
            .await
            .map_err(domain_error_to_status)?;
        self.prepare_participants(&definition).await?;
        let instance = self
            .start_ceremony
            .execute(input)
            .await
            .map_err(domain_error_to_status)?;
        Ok(Response::new(pb::StartCeremonyResponse {
            instance: Some(Self::render(&instance, &definition).map_err(domain_error_to_status)?),
        }))
    }

    #[tracing::instrument(name = "rpc.start_published_ceremony", skip_all)]
    pub(super) async fn handle_start_published_ceremony(
        &self,
        request: Request<pb::StartPublishedCeremonyRequest>,
    ) -> GrpcResult<pb::StartPublishedCeremonyResponse> {
        link_span_to_metadata(&request);
        let input = start_published_ceremony_input_from_proto(request.into_inner())
            .map_err(domain_error_to_status)?;
        let instance = self
            .start_published_ceremony
            .execute(input)
            .await
            .map_err(domain_error_to_status)?;
        // Resolved rather than read from the publication directly: the
        // instance records a digest, and resolving it back through the
        // same path every other RPC uses is what proves the digest it
        // recorded still matches what is published.
        let definition = self
            .resolve_ceremony_definition
            .execute(&instance)
            .await
            .map_err(domain_error_to_status)?;
        self.prepare_participants(&definition).await?;
        Ok(Response::new(pb::StartPublishedCeremonyResponse {
            instance: Some(Self::render(&instance, &definition).map_err(domain_error_to_status)?),
        }))
    }

    #[tracing::instrument(name = "rpc.run_ceremony_step", skip_all)]
    pub(super) async fn handle_run_ceremony_step(
        &self,
        request: Request<pb::RunCeremonyStepRequest>,
    ) -> GrpcResult<pb::RunCeremonyStepResponse> {
        link_span_to_metadata(&request);
        let request = request.into_inner();
        let ceremony_id =
            CeremonyId::new(request.ceremony_id.clone()).map_err(domain_error_to_status)?;
        let (instance, definition) = self.session(&ceremony_id).await?;
        let input = run_ceremony_step_input_from_proto(request, &definition, &instance)
            .map_err(domain_error_to_status)?;
        let output = self
            .run_ceremony_step
            .execute(input)
            .await
            .map_err(domain_error_to_status)?;
        Ok(Response::new(pb::RunCeremonyStepResponse {
            instance: Some(
                Self::render(output.instance(), &definition).map_err(domain_error_to_status)?,
            ),
        }))
    }

    #[tracing::instrument(name = "rpc.apply_ceremony_transition", skip_all)]
    pub(super) async fn handle_apply_ceremony_transition(
        &self,
        request: Request<pb::ApplyCeremonyTransitionRequest>,
    ) -> GrpcResult<pb::ApplyCeremonyTransitionResponse> {
        link_span_to_metadata(&request);
        let request = request.into_inner();
        let ceremony_id =
            CeremonyId::new(request.ceremony_id.clone()).map_err(domain_error_to_status)?;
        let (instance, definition) = self.session(&ceremony_id).await?;
        let input = apply_ceremony_transition_input_from_proto(request, &definition, &instance)
            .map_err(domain_error_to_status)?;
        let moved = self
            .apply_ceremony_transition
            .execute(input)
            .await
            .map_err(domain_error_to_status)?;
        Ok(Response::new(pb::ApplyCeremonyTransitionResponse {
            instance: Some(Self::render(&moved, &definition).map_err(domain_error_to_status)?),
        }))
    }

    // Each of these answers with the session, like every other move.
    // The definition is resolved once per call rather than carried
    // over from a previous one: these are the calls a person makes,
    // minutes or hours apart, and nothing says the process handling
    // this one saw the last.

    #[tracing::instrument(name = "rpc.approve_ceremony_guard", skip_all)]
    pub(super) async fn handle_approve_ceremony_guard(
        &self,
        request: Request<pb::ApproveCeremonyGuardRequest>,
    ) -> GrpcResult<pb::ApproveCeremonyGuardResponse> {
        link_span_to_metadata(&request);
        let input = approve_ceremony_guard_input_from_proto(request.into_inner())
            .map_err(domain_error_to_status)?;
        let instance = self
            .approve_ceremony_guard
            .execute(input)
            .await
            .map_err(domain_error_to_status)?;
        let state = self.project(&instance).await?;
        Ok(Response::new(pb::ApproveCeremonyGuardResponse {
            instance: Some(state),
        }))
    }

    #[tracing::instrument(name = "rpc.defer_ceremony_guard", skip_all)]
    pub(super) async fn handle_defer_ceremony_guard(
        &self,
        request: Request<pb::DeferCeremonyGuardRequest>,
    ) -> GrpcResult<pb::DeferCeremonyGuardResponse> {
        link_span_to_metadata(&request);
        let input = defer_ceremony_guard_input_from_proto(request.into_inner())
            .map_err(domain_error_to_status)?;
        let instance = self
            .defer_ceremony_guard
            .execute(input)
            .await
            .map_err(domain_error_to_status)?;
        let state = self.project(&instance).await?;
        Ok(Response::new(pb::DeferCeremonyGuardResponse {
            instance: Some(state),
        }))
    }

    #[tracing::instrument(name = "rpc.assert_ceremony_reason", skip_all)]
    pub(super) async fn handle_assert_ceremony_reason(
        &self,
        request: Request<pb::AssertCeremonyReasonRequest>,
    ) -> GrpcResult<pb::AssertCeremonyReasonResponse> {
        link_span_to_metadata(&request);
        let input = assert_ceremony_reason_input_from_proto(request.into_inner())
            .map_err(domain_error_to_status)?;
        let instance = self
            .assert_ceremony_reason
            .execute(input)
            .await
            .map_err(domain_error_to_status)?;
        let state = self.project(&instance).await?;
        Ok(Response::new(pb::AssertCeremonyReasonResponse {
            instance: Some(state),
        }))
    }

    #[tracing::instrument(name = "rpc.request_ceremony_intervention", skip_all)]
    pub(super) async fn handle_request_ceremony_intervention(
        &self,
        request: Request<pb::RequestCeremonyInterventionRequest>,
    ) -> GrpcResult<pb::RequestCeremonyInterventionResponse> {
        link_span_to_metadata(&request);
        let input = request_ceremony_intervention_input_from_proto(request.into_inner())
            .map_err(domain_error_to_status)?;
        let instance = self
            .request_ceremony_intervention
            .execute(input)
            .await
            .map_err(domain_error_to_status)?;
        let state = self.project(&instance).await?;
        Ok(Response::new(pb::RequestCeremonyInterventionResponse {
            instance: Some(state),
        }))
    }

    #[tracing::instrument(name = "rpc.respond_to_ceremony_intervention", skip_all)]
    pub(super) async fn handle_respond_to_ceremony_intervention(
        &self,
        request: Request<pb::RespondToCeremonyInterventionRequest>,
    ) -> GrpcResult<pb::RespondToCeremonyInterventionResponse> {
        link_span_to_metadata(&request);
        let input = respond_to_ceremony_intervention_input_from_proto(request.into_inner())
            .map_err(domain_error_to_status)?;
        let instance = self
            .respond_to_ceremony_intervention
            .execute(input)
            .await
            .map_err(domain_error_to_status)?;
        let state = self.project(&instance).await?;
        Ok(Response::new(pb::RespondToCeremonyInterventionResponse {
            instance: Some(state),
        }))
    }

    #[tracing::instrument(name = "rpc.close_ceremony_intervention", skip_all)]
    pub(super) async fn handle_close_ceremony_intervention(
        &self,
        request: Request<pb::CloseCeremonyInterventionRequest>,
    ) -> GrpcResult<pb::CloseCeremonyInterventionResponse> {
        link_span_to_metadata(&request);
        let input = close_ceremony_intervention_input_from_proto(request.into_inner())
            .map_err(domain_error_to_status)?;
        let instance = self
            .close_ceremony_intervention
            .execute(input)
            .await
            .map_err(domain_error_to_status)?;
        let state = self.project(&instance).await?;
        Ok(Response::new(pb::CloseCeremonyInterventionResponse {
            instance: Some(state),
        }))
    }

    #[tracing::instrument(name = "rpc.collect_ceremony_evidence", skip_all)]
    pub(super) async fn handle_collect_ceremony_evidence(
        &self,
        request: Request<pb::CollectCeremonyEvidenceRequest>,
    ) -> GrpcResult<pb::CollectCeremonyEvidenceResponse> {
        link_span_to_metadata(&request);
        let input = collect_ceremony_evidence_input_from_proto(request.into_inner())
            .map_err(domain_error_to_status)?;
        let instance = self
            .collect_ceremony_evidence
            .execute(input)
            .await
            .map_err(domain_error_to_status)?;
        let state = self.project(&instance).await?;
        Ok(Response::new(pb::CollectCeremonyEvidenceResponse {
            instance: Some(state),
        }))
    }
}
