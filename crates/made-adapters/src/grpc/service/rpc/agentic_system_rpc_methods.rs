/// The nine agentic-system RPCs.
///
/// Every one is authorized globally: authority over the aggregate and
/// its runs is coarse in this version by decision, not by omission
/// (ADR-021), and a scope the service could not yet enforce well
/// would be worse than the honest coarse answer.
macro_rules! agentic_system_rpc_methods {
    ($callback:ident; { $($methods:tt)* }) => {
        $callback!({
            $($methods)*
        async fn design_agentic_system(
            &self,
            request: Request<pb::DesignAgenticSystemRequest>,
        ) -> GrpcResult<pb::DesignAgenticSystemResponse> {
            authorized_global!(
                self,
                request,
                DesignAgenticSystem,
                self.handle_design_agentic_system(request)
            )
        }
        async fn get_agentic_system(
            &self,
            request: Request<pb::GetAgenticSystemRequest>,
        ) -> GrpcResult<pb::GetAgenticSystemResponse> {
            authorized_global!(
                self,
                request,
                GetAgenticSystem,
                self.handle_get_agentic_system(request)
            )
        }
        async fn list_agentic_systems(
            &self,
            request: Request<pb::ListAgenticSystemsRequest>,
        ) -> GrpcResult<pb::ListAgenticSystemsResponse> {
            authorized_global!(
                self,
                request,
                ListAgenticSystems,
                self.handle_list_agentic_systems(request)
            )
        }
        async fn validate_agentic_system(
            &self,
            request: Request<pb::ValidateAgenticSystemRequest>,
        ) -> GrpcResult<pb::ValidateAgenticSystemResponse> {
            authorized_global!(
                self,
                request,
                ValidateAgenticSystem,
                self.handle_validate_agentic_system(request)
            )
        }
        async fn publish_agentic_system(
            &self,
            request: Request<pb::PublishAgenticSystemRequest>,
        ) -> GrpcResult<pb::PublishAgenticSystemResponse> {
            authorized_global!(
                self,
                request,
                PublishAgenticSystem,
                self.handle_publish_agentic_system(request)
            )
        }
        async fn instantiate_agentic_system(
            &self,
            request: Request<pb::InstantiateAgenticSystemRequest>,
        ) -> GrpcResult<pb::InstantiateAgenticSystemResponse> {
            authorized_global!(
                self,
                request,
                InstantiateAgenticSystem,
                self.handle_instantiate_agentic_system(request)
            )
        }
        async fn advance_agentic_system_execution(
            &self,
            request: Request<pb::AdvanceAgenticSystemExecutionRequest>,
        ) -> GrpcResult<pb::AdvanceAgenticSystemExecutionResponse> {
            authorized_global!(
                self,
                request,
                AdvanceAgenticSystemExecution,
                self.handle_advance_agentic_system_execution(request)
            )
        }
        async fn get_agentic_system_execution(
            &self,
            request: Request<pb::GetAgenticSystemExecutionRequest>,
        ) -> GrpcResult<pb::GetAgenticSystemExecutionResponse> {
            authorized_global!(
                self,
                request,
                GetAgenticSystemExecution,
                self.handle_get_agentic_system_execution(request)
            )
        }
        async fn render_agentic_system_diagram(
            &self,
            request: Request<pb::RenderAgenticSystemDiagramRequest>,
        ) -> GrpcResult<pb::RenderAgenticSystemDiagramResponse> {
            authorized_global!(
                self,
                request,
                RenderAgenticSystemDiagram,
                self.handle_render_agentic_system_diagram(request)
            )
        }
        });
    };
}

pub(super) use agentic_system_rpc_methods;
