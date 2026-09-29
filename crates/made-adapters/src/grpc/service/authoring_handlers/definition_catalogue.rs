//! Reading the published catalogue back.
//!
//! Both calls answer from the catalogue only. A definition mounted in
//! this process is not something a remote caller published, and a read
//! that fell back to it would answer a different question.
//!
//! The use cases are built per call from the catalogue port, the only
//! thing either holds, as `DesignCeremonyUseCase` is built per call from
//! nothing.

use crate::grpc::mappers::{
    get_ceremony_definition_response_from, list_ceremony_definitions_response_from,
    published_ceremony_definition_identity_from_proto,
    published_ceremony_definition_query_from_proto,
};

use made_app::usecases::{
    GetPublishedCeremonyDefinitionUseCase, ListPublishedCeremonyDefinitionsUseCase,
};
use made_core::value_objects::{
    AuthorizationAction, AuthorizationScope, AuthorizedOperation, CeremonyName, CeremonyVersion,
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
        ceremony: &str,
        version: &str,
    ) -> Result<AuthorizedOperation, Status> {
        let scope = AuthorizationScope::Definition {
            name: CeremonyName::new(ceremony).map_err(domain_error_to_status)?,
            version: Some(CeremonyVersion::new(version).map_err(domain_error_to_status)?),
        };
        self.authorization
            .authorize(request, action, scope, None)
            .await
    }

    #[tracing::instrument(name = "rpc.list_ceremony_definitions", skip_all)]
    pub(in crate::grpc::service) async fn handle_list_ceremony_definitions(
        &self,
        request: Request<pb::ListCeremonyDefinitionsRequest>,
    ) -> GrpcResult<pb::ListCeremonyDefinitionsResponse> {
        link_span_to_metadata(&request);
        let query = published_ceremony_definition_query_from_proto(request.into_inner())
            .map_err(domain_error_to_status)?;
        let page = ListPublishedCeremonyDefinitionsUseCase::new(self.ceremony_publications.clone())
            .execute(&query)
            .await
            .map_err(domain_error_to_status)?;
        Ok(Response::new(list_ceremony_definitions_response_from(
            &page,
        )))
    }

    #[tracing::instrument(name = "rpc.get_ceremony_definition", skip_all)]
    pub(in crate::grpc::service) async fn handle_get_ceremony_definition(
        &self,
        request: Request<pb::GetCeremonyDefinitionRequest>,
    ) -> GrpcResult<pb::GetCeremonyDefinitionResponse> {
        link_span_to_metadata(&request);
        let (name, version) = published_ceremony_definition_identity_from_proto(request.get_ref())
            .map_err(domain_error_to_status)?;
        let published =
            GetPublishedCeremonyDefinitionUseCase::new(self.ceremony_publications.clone())
                .execute(&name, &version)
                .await
                .map_err(domain_error_to_status)?;
        get_ceremony_definition_response_from(&published)
            .map(Response::new)
            .map_err(domain_error_to_status)
    }
}
