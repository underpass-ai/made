//! Reading the published catalogue back.
//!
//! Both calls answer from the catalogue only. A definition mounted in
//! this process is not something a remote caller published, and a read
//! that fell back to it would answer a different question.
//!
//! The use cases are built per call from the catalogue port, the only
//! thing either holds, as `DesignProtocolUseCase` is built per call from
//! nothing.

use crate::grpc::mappers::{
    get_protocol_definition_response_from, list_protocol_definitions_response_from,
    published_protocol_definition_identity_from_proto,
    published_protocol_definition_query_from_proto,
};

use made_app::usecases::{
    GetPublishedProtocolDefinitionUseCase, ListPublishedProtocolDefinitionsUseCase,
};
use made_core::value_objects::{
    AuthorizationAction, AuthorizationScope, AuthorizedOperation, ProtocolName, ProtocolVersion,
};

use crate::grpc::service::{
    domain_error_to_status, link_span_to_metadata, pb, GrpcResult, MadeGrpcService, Request,
    Response, Status,
};

impl MadeGrpcService {
    /// A read of one published version, scoped to that version the way
    /// the in-process facade scopes it: a grant over one definition can
    /// read it without being a grant over the whole catalogue.
    pub(in crate::grpc::service) async fn authorize_named_definition<T: prost::Message>(
        &self,
        request: &Request<T>,
        action: AuthorizationAction,
        protocol: &str,
        version: &str,
    ) -> Result<AuthorizedOperation, Status> {
        let scope = AuthorizationScope::Definition {
            name: ProtocolName::new(protocol).map_err(domain_error_to_status)?,
            version: Some(ProtocolVersion::new(version).map_err(domain_error_to_status)?),
        };
        self.authorization
            .authorize(request, action, scope, None)
            .await
    }

    #[tracing::instrument(name = "rpc.list_protocol_definitions", skip_all)]
    pub(in crate::grpc::service) async fn handle_list_protocol_definitions(
        &self,
        request: Request<pb::ListProtocolDefinitionsRequest>,
    ) -> GrpcResult<pb::ListProtocolDefinitionsResponse> {
        link_span_to_metadata(&request);
        let query = published_protocol_definition_query_from_proto(request.into_inner())
            .map_err(domain_error_to_status)?;
        let page = ListPublishedProtocolDefinitionsUseCase::new(self.protocol_publications.clone())
            .execute(&query)
            .await
            .map_err(domain_error_to_status)?;
        Ok(Response::new(list_protocol_definitions_response_from(
            &page,
        )))
    }

    #[tracing::instrument(name = "rpc.get_protocol_definition", skip_all)]
    pub(in crate::grpc::service) async fn handle_get_protocol_definition(
        &self,
        request: Request<pb::GetProtocolDefinitionRequest>,
    ) -> GrpcResult<pb::GetProtocolDefinitionResponse> {
        link_span_to_metadata(&request);
        let (name, version) = published_protocol_definition_identity_from_proto(request.get_ref())
            .map_err(domain_error_to_status)?;
        let published =
            GetPublishedProtocolDefinitionUseCase::new(self.protocol_publications.clone())
                .execute(&name, &version)
                .await
                .map_err(domain_error_to_status)?;
        get_protocol_definition_response_from(&published)
            .map(Response::new)
            .map_err(domain_error_to_status)
    }
}
