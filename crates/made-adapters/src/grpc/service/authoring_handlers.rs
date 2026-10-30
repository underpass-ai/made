use made_app::usecases::DesignProtocolUseCase;

// Reading back what authoring published: a child of the authoring
// handlers because it answers about the same definitions.
mod definition_catalogue;

use super::{
    bind_protocol_participants_input_from_proto, design_protocol_response_from,
    diff_protocol_definitions_response_from, domain_error_to_status,
    explain_protocol_draft_response_from, link_span_to_metadata, metric_family_to_proto, pb,
    protocol_definition_source_from_proto, protocol_design_document_from_proto,
    publish_protocol_definition_response_from, service_status_to_proto, statistics_to_proto,
    unrehydratable_protocol_instance_state_from, validate_protocol_draft_response_from, GrpcResult,
    MadeGrpcService, ProtocolDefinitionYaml, ProtocolDraftView, ProtocolId, Request, Response,
};

impl MadeGrpcService {
    // Validating and explaining touch nothing: they answer about the
    // YAML in the request. A draft is not a definition until someone
    // publishes it, and that distinction is the point of having three
    // calls instead of one.
    #[tracing::instrument(name = "rpc.validate_protocol_draft", skip_all)]
    pub(super) async fn handle_validate_protocol_draft(
        &self,
        request: Request<pb::ValidateProtocolDraftRequest>,
    ) -> GrpcResult<pb::ValidateProtocolDraftResponse> {
        link_span_to_metadata(&request);
        let draft = ProtocolDefinitionYaml::parse_draft_str(&request.into_inner().definition_yaml)
            .map_err(domain_error_to_status)?;
        let report = draft.analyze();
        Ok(Response::new(validate_protocol_draft_response_from(
            &ProtocolDraftView::project(&draft, &report),
        )))
    }

    #[tracing::instrument(name = "rpc.explain_protocol_draft", skip_all)]
    pub(super) async fn handle_explain_protocol_draft(
        &self,
        request: Request<pb::ExplainProtocolDraftRequest>,
    ) -> GrpcResult<pb::ExplainProtocolDraftResponse> {
        link_span_to_metadata(&request);
        let draft = ProtocolDefinitionYaml::parse_draft_str(&request.into_inner().definition_yaml)
            .map_err(domain_error_to_status)?;
        let report = draft.analyze();
        Ok(Response::new(explain_protocol_draft_response_from(
            &ProtocolDraftView::project(&draft, &report),
        )))
    }

    /// Designing touches nothing either, and answers with the draft
    /// it rendered already analysed — through the same parser and the
    /// same projection `ValidateProtocolDraft` uses, so a designed
    /// draft is read exactly like a hand-authored one.
    ///
    /// The use case is built here rather than injected: it holds no
    /// port, and a dependency that does not exist is not one the
    /// composition root can supply.
    #[tracing::instrument(name = "rpc.design_protocol", skip_all)]
    pub(super) async fn handle_design_protocol(
        &self,
        request: Request<pb::DesignProtocolRequest>,
    ) -> GrpcResult<pb::DesignProtocolResponse> {
        link_span_to_metadata(&request);
        let document = protocol_design_document_from_proto(request.into_inner())
            .map_err(domain_error_to_status)?;
        let designed = DesignProtocolUseCase::new()
            .execute(&document)
            .map_err(domain_error_to_status)?;
        let draft = designed.definition();
        let yaml =
            crate::yaml::DesignedProtocolYaml::render(&designed).map_err(domain_error_to_status)?;
        let report = draft.analyze();
        Ok(Response::new(design_protocol_response_from(
            &designed,
            yaml,
            &ProtocolDraftView::project(draft, &report),
        )))
    }

    #[tracing::instrument(name = "rpc.publish_protocol_definition", skip_all)]
    pub(super) async fn handle_publish_protocol_definition(
        &self,
        request: Request<pb::PublishProtocolDefinitionRequest>,
    ) -> GrpcResult<pb::PublishProtocolDefinitionResponse> {
        link_span_to_metadata(&request);
        let definition = ProtocolDefinitionYaml::parse_str(&request.into_inner().definition_yaml)
            .map_err(domain_error_to_status)?;
        let outcome = self
            .publish_protocol_definition
            .execute(definition)
            .await
            .map_err(domain_error_to_status)?;
        Ok(Response::new(publish_protocol_definition_response_from(
            &outcome,
        )))
    }

    #[tracing::instrument(name = "rpc.bind_protocol_participants", skip_all)]
    pub(super) async fn handle_bind_protocol_participants(
        &self,
        request: Request<pb::BindProtocolParticipantsRequest>,
    ) -> GrpcResult<pb::BindProtocolParticipantsResponse> {
        link_span_to_metadata(&request);
        let input = bind_protocol_participants_input_from_proto(request.into_inner())
            .map_err(domain_error_to_status)?;
        let instance = self
            .bind_protocol_participants
            .execute(input)
            .await
            .map_err(domain_error_to_status)?;
        let state = self.project(&instance).await?;
        Ok(Response::new(pb::BindProtocolParticipantsResponse {
            instance: Some(state),
        }))
    }

    #[tracing::instrument(name = "rpc.diff_protocol_definitions", skip_all)]
    pub(super) async fn handle_diff_protocol_definitions(
        &self,
        request: Request<pb::DiffProtocolDefinitionsRequest>,
    ) -> GrpcResult<pb::DiffProtocolDefinitionsResponse> {
        link_span_to_metadata(&request);
        let request = request.into_inner();
        let before = protocol_definition_source_from_proto(request.before, "before")
            .map_err(domain_error_to_status)?;
        let after = protocol_definition_source_from_proto(request.after, "after")
            .map_err(domain_error_to_status)?;
        let diff = self
            .diff_protocol_definitions
            .execute(before, after)
            .await
            .map_err(domain_error_to_status)?;
        Ok(Response::new(diff_protocol_definitions_response_from(
            &diff,
        )))
    }

    #[tracing::instrument(name = "rpc.get_protocol_instance", skip_all)]
    pub(super) async fn handle_get_protocol_instance(
        &self,
        request: Request<pb::GetProtocolInstanceRequest>,
    ) -> GrpcResult<pb::GetProtocolInstanceResponse> {
        link_span_to_metadata(&request);
        let protocol_id =
            ProtocolId::new(request.into_inner().protocol_id).map_err(domain_error_to_status)?;
        let read = self
            .get_protocol_instance
            .execute_with_ids(&protocol_id)
            .await
            .map_err(domain_error_to_status)?;
        let definition = self
            .resolve_protocol_definition
            .execute(read.instance())
            .await
            .map_err(domain_error_to_status)?;
        let state = self
            .render_read(&read, &definition)
            .map_err(domain_error_to_status)?;
        Ok(Response::new(pb::GetProtocolInstanceResponse {
            instance: Some(state),
        }))
    }

    #[tracing::instrument(name = "rpc.list_protocol_instances", skip_all)]
    pub(super) async fn handle_list_protocol_instances(
        &self,
        request: Request<pb::ListProtocolInstancesRequest>,
    ) -> GrpcResult<pb::ListProtocolInstancesResponse> {
        link_span_to_metadata(&request);
        let instances = self
            .list_protocol_instances
            .execute()
            .await
            .map_err(domain_error_to_status)?;
        let mut states = Vec::with_capacity(instances.len());
        for instance in &instances {
            // The fold of a stream never needs the definition; rendering
            // it does. An instance naming a definition this store does
            // not hold becomes one unreadable entry rather than an error
            // that takes every readable session with it — the answer the
            // in-process backend has always given. Asking for that one
            // by id still fails loudly.
            states.push(match self.project(instance).await {
                Ok(state) => state,
                Err(unreadable) => {
                    unrehydratable_protocol_instance_state_from(instance.id(), unreadable.message())
                }
            });
        }
        Ok(Response::new(pb::ListProtocolInstancesResponse {
            instances: states,
        }))
    }

    // Observability is two mappers. The composition of a status —
    // version, uptime, condition, recorder, counters — is
    // `GetServiceStatusUseCase`'s, and the in-process edition calls
    // the same one, so the two editions cannot answer differently
    // about what they are (ADR-014, plan §3.7 G3).
    #[tracing::instrument(name = "rpc.get_status", skip_all)]
    pub(super) async fn handle_get_status(
        &self,
        request: Request<pb::GetStatusRequest>,
    ) -> GrpcResult<pb::GetStatusResponse> {
        link_span_to_metadata(&request);
        let status = self
            .get_service_status
            .execute(request.into_inner().include_stats)
            .await
            .map_err(domain_error_to_status)?;
        Ok(Response::new(service_status_to_proto(&status)))
    }

    #[tracing::instrument(name = "rpc.get_metrics", skip_all)]
    pub(super) async fn handle_get_metrics(
        &self,
        request: Request<pb::GetMetricsRequest>,
    ) -> GrpcResult<pb::GetMetricsResponse> {
        link_span_to_metadata(&request);
        let _ = request;
        let snapshot = self
            .get_service_metrics
            .execute()
            .await
            .map_err(domain_error_to_status)?;
        Ok(Response::new(pb::GetMetricsResponse {
            stats: Some(statistics_to_proto(snapshot.statistics())),
            registry_text: snapshot.registry().text().as_str().to_owned(),
            registry: snapshot
                .registry()
                .families()
                .iter()
                .map(metric_family_to_proto)
                .collect(),
        }))
    }
}
