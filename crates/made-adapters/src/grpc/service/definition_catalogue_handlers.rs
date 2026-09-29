//! Reading the published catalogue back.
//!
//! Both calls answer from the catalogue only. A definition mounted in
//! this process is not something a remote caller published, and a read
//! that fell back to it would answer a different question.

use crate::grpc::mappers::{
    get_ceremony_definition_response_from, list_ceremony_definitions_response_from,
    published_ceremony_definition_identity_from_proto,
    published_ceremony_definition_query_from_proto,
};

use super::{
    domain_error_to_status, link_span_to_metadata, pb, GrpcResult, MadeGrpcService, Request,
    Response,
};

impl MadeGrpcService {
    #[tracing::instrument(name = "rpc.list_ceremony_definitions", skip_all)]
    pub(super) async fn handle_list_ceremony_definitions(
        &self,
        request: Request<pb::ListCeremonyDefinitionsRequest>,
    ) -> GrpcResult<pb::ListCeremonyDefinitionsResponse> {
        link_span_to_metadata(&request);
        let query = published_ceremony_definition_query_from_proto(request.into_inner())
            .map_err(domain_error_to_status)?;
        let page = self
            .list_published_ceremony_definitions
            .execute(&query)
            .await
            .map_err(domain_error_to_status)?;
        Ok(Response::new(list_ceremony_definitions_response_from(
            &page,
        )))
    }

    #[tracing::instrument(name = "rpc.get_ceremony_definition", skip_all)]
    pub(super) async fn handle_get_ceremony_definition(
        &self,
        request: Request<pb::GetCeremonyDefinitionRequest>,
    ) -> GrpcResult<pb::GetCeremonyDefinitionResponse> {
        link_span_to_metadata(&request);
        let (name, version) = published_ceremony_definition_identity_from_proto(request.get_ref())
            .map_err(domain_error_to_status)?;
        let published = self
            .get_published_ceremony_definition
            .execute(&name, &version)
            .await
            .map_err(domain_error_to_status)?;
        get_ceremony_definition_response_from(&published)
            .map(Response::new)
            .map_err(domain_error_to_status)
    }
}
