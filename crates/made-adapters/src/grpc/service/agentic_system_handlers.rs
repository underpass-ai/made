//! The nine agentic-system operations, over gRPC.
//!
//! Every response is built from the same JSON projection the
//! in-process backend answers with, so a client that switches
//! backends reads the same words about the same design.

use made_app::usecases::agentic_system::AgenticSystemView;
use made_core::ports::AgenticSystemQuery;
use made_core::value_objects::AgenticSystemDigest;
use made_proto::v1 as pb;

use crate::json::{AgenticSystemExecutionJson, AgenticSystemJson, AgenticSystemValidationJson};

use super::agentic_system_requests as requests;
use super::{domain_error_to_status, GrpcResult, MadeGrpcService, Request, Response, Status};

impl MadeGrpcService {
    pub(super) async fn handle_design_agentic_system(
        &self,
        request: Request<pb::DesignAgenticSystemRequest>,
    ) -> GrpcResult<pb::DesignAgenticSystemResponse> {
        let document = requests::design_document(request.into_inner())?;
        let view = self
            .agentic_system()?
            .design()
            .execute(document)
            .await
            .map_err(domain_error_to_status)?;
        Ok(Response::new(pb::DesignAgenticSystemResponse {
            system: Some(requests::system_state(&view)?),
        }))
    }

    pub(super) async fn handle_get_agentic_system(
        &self,
        request: Request<pb::GetAgenticSystemRequest>,
    ) -> GrpcResult<pb::GetAgenticSystemResponse> {
        let request = request.into_inner();
        let id = requests::system_id(&request.system_id)?;
        let view = self
            .agentic_system()?
            .get()
            .execute(&id, requests::revision(request.revision)?)
            .await
            .map_err(domain_error_to_status)?;
        Ok(Response::new(pb::GetAgenticSystemResponse {
            system: Some(requests::system_state(&view)?),
        }))
    }

    pub(super) async fn handle_list_agentic_systems(
        &self,
        request: Request<pb::ListAgenticSystemsRequest>,
    ) -> GrpcResult<pb::ListAgenticSystemsResponse> {
        let query: AgenticSystemQuery = requests::list_query(request.get_ref())?;
        let page = self
            .agentic_system()?
            .list()
            .execute(&query)
            .await
            .map_err(domain_error_to_status)?;
        let mut systems = Vec::with_capacity(page.systems().len());
        for system in page.systems() {
            systems.push(requests::summary_state(system)?);
        }
        Ok(Response::new(pb::ListAgenticSystemsResponse {
            systems,
            next_cursor: page
                .next_cursor()
                .map(|cursor| cursor.as_str().to_owned())
                .unwrap_or_default(),
        }))
    }

    pub(super) async fn handle_validate_agentic_system(
        &self,
        request: Request<pb::ValidateAgenticSystemRequest>,
    ) -> GrpcResult<pb::ValidateAgenticSystemResponse> {
        let request = request.into_inner();
        let id = requests::system_id(&request.system_id)?;
        let view = self
            .agentic_system()?
            .validate()
            .execute(&id, requests::revision(request.revision)?)
            .await
            .map_err(domain_error_to_status)?;
        Ok(Response::new(requests::validation_response(
            &view,
            &AgenticSystemValidationJson::of(&view),
        )))
    }

    pub(super) async fn handle_publish_agentic_system(
        &self,
        request: Request<pb::PublishAgenticSystemRequest>,
    ) -> GrpcResult<pb::PublishAgenticSystemResponse> {
        let request = request.into_inner();
        let id = requests::system_id(&request.system_id)?;
        let revision = requests::required_revision(request.revision)?;
        let view = self
            .agentic_system()?
            .publish()
            .execute(&id, revision)
            .await
            .map_err(domain_error_to_status)?;
        Ok(Response::new(pb::PublishAgenticSystemResponse {
            outcome: view.outcome().as_str().to_owned(),
            system_id: id.as_str().to_owned(),
            sealed_revision: view.sealed_revision().get(),
            head_revision: view.head_revision().get(),
            digest: view
                .digest()
                .map(AgenticSystemDigest::to_hex)
                .unwrap_or_default(),
        }))
    }

    pub(super) async fn handle_instantiate_agentic_system(
        &self,
        request: Request<pb::InstantiateAgenticSystemRequest>,
    ) -> GrpcResult<pb::InstantiateAgenticSystemResponse> {
        let input = requests::instantiate_input(request.into_inner())?;
        let execution = Box::pin(self.agentic_system()?.instantiate().execute(input))
            .await
            .map_err(domain_error_to_status)?;
        Ok(Response::new(pb::InstantiateAgenticSystemResponse {
            execution: Some(requests::execution_state(
                &execution,
                &AgenticSystemExecutionJson::of(&execution),
            )),
        }))
    }

    pub(super) async fn handle_advance_agentic_system_execution(
        &self,
        request: Request<pb::AdvanceAgenticSystemExecutionRequest>,
    ) -> GrpcResult<pb::AdvanceAgenticSystemExecutionResponse> {
        let (id, actor_id, actor_kind) = requests::advance_input(request.into_inner())?;
        let execution = Box::pin(
            self.agentic_system()?
                .advance()
                .execute(&id, &actor_id, actor_kind),
        )
        .await
        .map_err(domain_error_to_status)?;
        Ok(Response::new(pb::AdvanceAgenticSystemExecutionResponse {
            execution: Some(requests::execution_state(
                &execution,
                &AgenticSystemExecutionJson::of(&execution),
            )),
        }))
    }

    pub(super) async fn handle_get_agentic_system_execution(
        &self,
        request: Request<pb::GetAgenticSystemExecutionRequest>,
    ) -> GrpcResult<pb::GetAgenticSystemExecutionResponse> {
        let id = requests::execution_id(&request.into_inner().execution_id)?;
        let view = self
            .agentic_system()?
            .get_execution()
            .execute(&id)
            .await
            .map_err(domain_error_to_status)?;
        let rendered = AgenticSystemExecutionJson::view(&view).map_err(domain_error_to_status)?;
        let design =
            AgenticSystemView::of(view.system().clone()).map_err(domain_error_to_status)?;
        Ok(Response::new(pb::GetAgenticSystemExecutionResponse {
            execution: Some(requests::execution_state(view.execution(), &rendered)),
            system: Some(requests::system_state(&design)?),
        }))
    }

    pub(super) async fn handle_render_agentic_system_diagram(
        &self,
        request: Request<pb::RenderAgenticSystemDiagramRequest>,
    ) -> GrpcResult<pb::RenderAgenticSystemDiagramResponse> {
        let request = request.into_inner();
        let id = requests::system_id(&request.system_id)?;
        let execution = requests::optional_execution_id(&request.execution_id)?;
        let diagram = self
            .agentic_system()?
            .render()
            .execute(
                &id,
                requests::revision(request.revision)?,
                execution.as_ref(),
            )
            .await
            .map_err(domain_error_to_status)?;
        let _ = AgenticSystemJson::diagram(&diagram);
        Ok(Response::new(pb::RenderAgenticSystemDiagramResponse {
            mermaid: diagram.mermaid().to_owned(),
            text_equivalent: diagram.text_equivalent().to_vec(),
        }))
    }

    /// The bundle, or an honest refusal.
    ///
    /// `Status` is tonic's error type and is large; every handler in
    /// this layer carries it, so the lint is answered once here
    /// rather than by boxing an error nobody ever stores.
    ///
    /// A deployment that composed no agentic-system stores says so
    /// rather than answering with an empty catalogue that looks like
    /// a system nobody has designed yet.
    #[allow(clippy::result_large_err)]
    fn agentic_system(&self) -> Result<&super::AgenticSystemOperations, Status> {
        self.agentic_system
            .as_ref()
            .map(std::convert::AsRef::as_ref)
            .ok_or_else(|| Status::unimplemented("agentic systems are not configured"))
    }
}
