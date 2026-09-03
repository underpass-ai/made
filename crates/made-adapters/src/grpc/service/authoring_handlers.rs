use super::{
    bind_ceremony_participants_input_from_proto, ceremony_definition_source_from_proto,
    diff_ceremony_definitions_response_from, domain_error_to_status,
    explain_ceremony_draft_response_from, link_span_to_metadata, pb,
    publish_ceremony_definition_response_from, statistics_to_proto,
    validate_ceremony_draft_response_from, CeremonyDefinitionYaml, CeremonyDraftView, CeremonyId,
    GrpcResult, MadeGrpcService, Request, Response,
};

impl MadeGrpcService {
    // Validating and explaining touch nothing: they answer about the
    // YAML in the request. A draft is not a definition until someone
    // publishes it, and that distinction is the point of having three
    // calls instead of one.
    #[tracing::instrument(name = "rpc.validate_ceremony_draft", skip_all)]
    pub(super) async fn handle_validate_ceremony_draft(
        &self,
        request: Request<pb::ValidateCeremonyDraftRequest>,
    ) -> GrpcResult<pb::ValidateCeremonyDraftResponse> {
        link_span_to_metadata(&request);
        let draft = CeremonyDefinitionYaml::parse_draft_str(&request.into_inner().definition_yaml)
            .map_err(domain_error_to_status)?;
        let report = draft.analyze();
        Ok(Response::new(validate_ceremony_draft_response_from(
            &CeremonyDraftView::project(&draft, &report),
        )))
    }

    #[tracing::instrument(name = "rpc.explain_ceremony_draft", skip_all)]
    pub(super) async fn handle_explain_ceremony_draft(
        &self,
        request: Request<pb::ExplainCeremonyDraftRequest>,
    ) -> GrpcResult<pb::ExplainCeremonyDraftResponse> {
        link_span_to_metadata(&request);
        let draft = CeremonyDefinitionYaml::parse_draft_str(&request.into_inner().definition_yaml)
            .map_err(domain_error_to_status)?;
        let report = draft.analyze();
        Ok(Response::new(explain_ceremony_draft_response_from(
            &CeremonyDraftView::project(&draft, &report),
        )))
    }

    #[tracing::instrument(name = "rpc.publish_ceremony_definition", skip_all)]
    pub(super) async fn handle_publish_ceremony_definition(
        &self,
        request: Request<pb::PublishCeremonyDefinitionRequest>,
    ) -> GrpcResult<pb::PublishCeremonyDefinitionResponse> {
        link_span_to_metadata(&request);
        let definition = CeremonyDefinitionYaml::parse_str(&request.into_inner().definition_yaml)
            .map_err(domain_error_to_status)?;
        let outcome = self
            .publish_ceremony_definition
            .execute(definition)
            .await
            .map_err(domain_error_to_status)?;
        Ok(Response::new(publish_ceremony_definition_response_from(
            &outcome,
        )))
    }

    #[tracing::instrument(name = "rpc.bind_ceremony_participants", skip_all)]
    pub(super) async fn handle_bind_ceremony_participants(
        &self,
        request: Request<pb::BindCeremonyParticipantsRequest>,
    ) -> GrpcResult<pb::BindCeremonyParticipantsResponse> {
        link_span_to_metadata(&request);
        let input = bind_ceremony_participants_input_from_proto(request.into_inner())
            .map_err(domain_error_to_status)?;
        let instance = self
            .bind_ceremony_participants
            .execute(input)
            .await
            .map_err(domain_error_to_status)?;
        let state = self.project(&instance).await?;
        Ok(Response::new(pb::BindCeremonyParticipantsResponse {
            instance: Some(state),
        }))
    }

    #[tracing::instrument(name = "rpc.diff_ceremony_definitions", skip_all)]
    pub(super) async fn handle_diff_ceremony_definitions(
        &self,
        request: Request<pb::DiffCeremonyDefinitionsRequest>,
    ) -> GrpcResult<pb::DiffCeremonyDefinitionsResponse> {
        link_span_to_metadata(&request);
        let request = request.into_inner();
        let before = ceremony_definition_source_from_proto(request.before, "before")
            .map_err(domain_error_to_status)?;
        let after = ceremony_definition_source_from_proto(request.after, "after")
            .map_err(domain_error_to_status)?;
        let diff = self
            .diff_ceremony_definitions
            .execute(before, after)
            .await
            .map_err(domain_error_to_status)?;
        Ok(Response::new(diff_ceremony_definitions_response_from(
            &diff,
        )))
    }

    #[tracing::instrument(name = "rpc.get_ceremony_instance", skip_all)]
    pub(super) async fn handle_get_ceremony_instance(
        &self,
        request: Request<pb::GetCeremonyInstanceRequest>,
    ) -> GrpcResult<pb::GetCeremonyInstanceResponse> {
        link_span_to_metadata(&request);
        let ceremony_id =
            CeremonyId::new(request.into_inner().ceremony_id).map_err(domain_error_to_status)?;
        let instance = self
            .get_ceremony_instance
            .execute(&ceremony_id)
            .await
            .map_err(domain_error_to_status)?;
        let state = self.project(&instance).await?;
        Ok(Response::new(pb::GetCeremonyInstanceResponse {
            instance: Some(state),
        }))
    }

    #[tracing::instrument(name = "rpc.list_ceremony_instances", skip_all)]
    pub(super) async fn handle_list_ceremony_instances(
        &self,
        request: Request<pb::ListCeremonyInstancesRequest>,
    ) -> GrpcResult<pb::ListCeremonyInstancesResponse> {
        link_span_to_metadata(&request);
        let instances = self
            .list_ceremony_instances
            .execute()
            .await
            .map_err(domain_error_to_status)?;
        let mut states = Vec::with_capacity(instances.len());
        for instance in &instances {
            states.push(self.project(instance).await?);
        }
        Ok(Response::new(pb::ListCeremonyInstancesResponse {
            instances: states,
        }))
    }

    #[tracing::instrument(name = "rpc.get_status", skip_all)]
    pub(super) async fn handle_get_status(
        &self,
        request: Request<pb::GetStatusRequest>,
    ) -> GrpcResult<pb::GetStatusResponse> {
        link_span_to_metadata(&request);
        let include_stats = request.into_inner().include_stats;
        let stats = if include_stats {
            Some(
                self.statistics
                    .snapshot()
                    .await
                    .map_err(domain_error_to_status)?,
            )
        } else {
            None
        };

        Ok(Response::new(pb::GetStatusResponse {
            version: self.service_version.to_owned(),
            uptime_seconds: self.started_at.elapsed().as_secs(),
            health: "healthy".to_owned(),
            stats: stats.as_ref().map(statistics_to_proto),
        }))
    }

    #[tracing::instrument(name = "rpc.get_metrics", skip_all)]
    pub(super) async fn handle_get_metrics(
        &self,
        request: Request<pb::GetMetricsRequest>,
    ) -> GrpcResult<pb::GetMetricsResponse> {
        link_span_to_metadata(&request);
        let _ = request;
        let snap = self
            .statistics
            .snapshot()
            .await
            .map_err(domain_error_to_status)?;
        Ok(Response::new(pb::GetMetricsResponse {
            stats: Some(statistics_to_proto(&snap)),
        }))
    }
}
