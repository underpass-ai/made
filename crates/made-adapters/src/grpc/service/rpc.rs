use std::pin::Pin;
use std::task::{Context, Poll};

use async_trait::async_trait;
use made_app::services::AuthorizationOperationScope;
use made_core::value_objects::{AuthorizationAction, AuthorizationScope};
use made_proto::v1 as pb;
use tonic::{Request, Response, Status};

use super::{
    run_with_protocol_trace, trace_context_from_metadata, GrpcResult, MadeGrpcService, MadeService,
};

mod agentic_system_rpc_methods;
mod authorization_rpc_methods;
mod council_rpc_methods;
mod integrator_loop_rpc_methods;
mod protocol_rpc_methods;
mod resource_rpc_methods;

use agentic_system_rpc_methods::agentic_system_rpc_methods;
use authorization_rpc_methods::authorization_rpc_methods;
use council_rpc_methods::council_rpc_methods;
use integrator_loop_rpc_methods::integrator_loop_rpc_methods;
use protocol_rpc_methods::protocol_rpc_methods;
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

/// A read of one published version, scoped to that version the way
/// the in-process facade scopes it: a grant over one definition can
/// read it without being a grant over the whole catalogue.
macro_rules! authorized_named_definition {
    ($service:expr, $request:ident, $action:ident, $future:expr) => {{
        let protocol = $request.get_ref().protocol.clone();
        let version = $request.get_ref().version.clone();
        let authorization = $service
            .authorize_named_definition(
                &$request,
                AuthorizationAction::$action,
                &protocol,
                &version,
            )
            .await?;
        AuthorizationOperationScope::run(authorization, $future).await
    }};
}

macro_rules! authorized_protocol {
    ($service:expr, $request:ident, $action:ident, $future:expr) => {{
        let protocol_id = $request.get_ref().protocol_id.clone();
        let authorization = $service
            .authorize_protocol(&$request, AuthorizationAction::$action, &protocol_id)
            .await?;
        AuthorizationOperationScope::run(authorization, $future).await
    }};
}

/// A scope that is a message and not a field: a protocol when it names
/// one, and the host level when it names a system run instead. Said
/// here rather than in each arm, so the three calls that take a scope
/// cannot come to disagree about what it authorizes.
macro_rules! authorized_integrator_scope {
    ($service:expr, $request:ident, $action:ident, $future:expr) => {{
        let protocol_id = $request
            .get_ref()
            .scope
            .as_ref()
            .filter(|scope| scope.kind == "protocol")
            .map(|scope| scope.protocol_id.clone());
        let authorization = match protocol_id {
            Some(protocol_id) => {
                $service
                    .authorize_protocol(&$request, AuthorizationAction::$action, &protocol_id)
                    .await?
            }
            None => {
                $service
                    .authorize_global(&$request, AuthorizationAction::$action)
                    .await?
            }
        };
        AuthorizationOperationScope::run(authorization, $future).await
    }};
}

/// A read that may narrow itself to one protocol. Unnarrowed it is a
/// question about the deployment, which is a Global grant and not a
/// protocol one.
macro_rules! authorized_protocol_or_global {
    ($service:expr, $request:ident, $action:ident, $future:expr) => {{
        let protocol_id = $request.get_ref().protocol_id.clone();
        let authorization = if protocol_id.is_empty() {
            $service
                .authorize_global(&$request, AuthorizationAction::$action)
                .await?
        } else {
            $service
                .authorize_protocol(&$request, AuthorizationAction::$action, &protocol_id)
                .await?
        };
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

macro_rules! authorized_budget_for_protocol {
    ($service:expr, $request:ident, $action:ident, $future:expr) => {{
        let protocol_id = $request.get_ref().protocol_id.clone();
        let authorization = $service
            .authorize_budget_for_protocol(&$request, AuthorizationAction::$action, &protocol_id)
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
