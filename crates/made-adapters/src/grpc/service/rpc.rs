use std::pin::Pin;
use std::task::{Context, Poll};

use async_trait::async_trait;
use made_app::services::AuthorizationOperationScope;
use made_core::value_objects::{AuthorizationAction, AuthorizationScope};
use made_proto::v1 as pb;
use tonic::{Request, Response, Status};

use super::{
    run_with_ceremony_trace, trace_context_from_metadata, GrpcResult, MadeGrpcService, MadeService,
};

mod agentic_system_rpc_methods;
mod authorization_rpc_methods;
mod ceremony_rpc_methods;
mod council_rpc_methods;
mod resource_rpc_methods;

use agentic_system_rpc_methods::agentic_system_rpc_methods;
use authorization_rpc_methods::authorization_rpc_methods;
use ceremony_rpc_methods::ceremony_rpc_methods;
use council_rpc_methods::council_rpc_methods;
use resource_rpc_methods::resource_rpc_methods;

macro_rules! authorized_global {
    ($service:expr, $request:ident, $action:ident, $future:expr) => {{
        let authorization = $service
            .authorize_global(&$request, AuthorizationAction::$action)
            .await?;
        AuthorizationOperationScope::run(authorization, $future).await
    }};
}

macro_rules! authorized_definition {
    ($service:expr, $request:ident, $action:ident, $future:expr) => {{
        let definition_yaml = $request.get_ref().definition_yaml.clone();
        let authorization = $service
            .authorize_definition(&$request, AuthorizationAction::$action, &definition_yaml)
            .await?;
        AuthorizationOperationScope::run(authorization, $future).await
    }};
}

macro_rules! authorized_ceremony {
    ($service:expr, $request:ident, $action:ident, $future:expr) => {{
        let ceremony_id = $request.get_ref().ceremony_id.clone();
        let authorization = $service
            .authorize_ceremony(&$request, AuthorizationAction::$action, &ceremony_id)
            .await?;
        AuthorizationOperationScope::run(authorization, $future).await
    }};
}

macro_rules! authorized_artifact {
    ($service:expr, $request:ident, $action:ident, $artifact_id:expr, $future:expr) => {{
        let artifact_id = $artifact_id.clone();
        let authorization = $service
            .authorize_artifact(&$request, AuthorizationAction::$action, &artifact_id)
            .await?;
        AuthorizationOperationScope::run(authorization, $future).await
    }};
}

macro_rules! authorized_budget_for_ceremony {
    ($service:expr, $request:ident, $action:ident, $future:expr) => {{
        let ceremony_id = $request.get_ref().ceremony_id.clone();
        let authorization = $service
            .authorize_budget_for_ceremony(&$request, AuthorizationAction::$action, &ceremony_id)
            .await?;
        AuthorizationOperationScope::run(authorization, $future).await
    }};
}

macro_rules! authorized_artifact_upload {
    ($service:expr, $request:ident, $action:ident, $upload_id:expr, $future:expr) => {{
        let upload_id = $upload_id.clone();
        let authorization = $service
            .authorize_artifact_upload(&$request, AuthorizationAction::$action, &upload_id)
            .await?;
        AuthorizationOperationScope::run(authorization, $future).await
    }};
}

macro_rules! authorized_artifact_upload_begin {
    ($service:expr, $request:ident, $action:ident, $future:expr) => {{
        let requested_artifact_id = $request.get_ref().requested_artifact_id.clone();
        let authorization = $service
            .authorize_artifact_upload_begin(
                &$request,
                AuthorizationAction::$action,
                requested_artifact_id.as_deref(),
            )
            .await?;
        AuthorizationOperationScope::run(authorization, $future).await
    }};
}

/// Boxed server stream used by the generated gRPC associated type.
pub struct RpcResultStream<T>(Pin<Box<dyn futures::Stream<Item = Result<T, Status>> + Send>>);

impl<T> std::fmt::Debug for RpcResultStream<T> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.debug_struct("RpcResultStream").finish()
    }
}

impl<T> RpcResultStream<T> {
    pub(super) fn new(
        stream: impl futures::Stream<Item = Result<T, Status>> + Send + 'static,
    ) -> Self {
        Self(Box::pin(stream))
    }
}

impl<T> futures::Stream for RpcResultStream<T> {
    type Item = Result<T, Status>;

    fn poll_next(self: Pin<&mut Self>, context: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        self.get_mut().0.as_mut().poll_next(context)
    }
}

macro_rules! build_rpc_impl {
    ({ $($methods:tt)* }) => {
        #[async_trait]
        impl MadeService for MadeGrpcService {
            $($methods)*
        }
    };
}

authorization_rpc_methods!(build_rpc_impl);
