use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use made_app::authorization::{AuthorizationGateOutcome, AuthorizeOperationUseCase};
use made_core::value_objects::{
    AuthenticatedPrincipal, AuthenticationMethod, AuthorizationAction, AuthorizationDecisionId,
    AuthorizationRequest, AuthorizationRequestId, AuthorizationScope, AuthorizationTargetDigest,
    PrincipalKind,
};
use prost::Message;
use tonic::{Request, Status};

use super::{AuthorizedGrpcInvocation, GrpcAuthorizationError, MutualTlsPrincipalMap};

const REQUEST_ID_HEADER: &str = "x-made-request-id";

/// Authenticates a gRPC caller and persists the policy decision before dispatch.
#[derive(Debug, Clone)]
pub struct GrpcAuthorizationGate {
    authorize: Arc<AuthorizeOperationUseCase>,
    mutual_tls_principals: Option<Arc<MutualTlsPrincipalMap>>,
    trusted_host: Option<AuthenticatedPrincipal>,
    trusted_request_namespace: Option<String>,
    trusted_request_sequence: Arc<AtomicU64>,
}

impl GrpcAuthorizationGate {
    #[must_use]
    pub fn mutual_tls(
        authorize: Arc<AuthorizeOperationUseCase>,
        principals: Arc<MutualTlsPrincipalMap>,
    ) -> Self {
        Self {
            authorize,
            mutual_tls_principals: Some(principals),
            trusted_host: None,
            trusted_request_namespace: None,
            trusted_request_sequence: Arc::new(AtomicU64::new(0)),
        }
    }

    pub fn trusted_host(
        authorize: Arc<AuthorizeOperationUseCase>,
        principal: AuthenticatedPrincipal,
        request_namespace: impl Into<String>,
    ) -> Result<Self, GrpcAuthorizationError> {
        let request_namespace = request_namespace.into();
        if principal.kind() != PrincipalKind::TrustedHost
            || principal.method() != AuthenticationMethod::LocalHostPolicy
        {
            return Err(GrpcAuthorizationError::InvalidTrustedHost);
        }
        if request_namespace.trim().is_empty() {
            return Err(GrpcAuthorizationError::EmptyRequestNamespace);
        }
        Ok(Self {
            authorize,
            mutual_tls_principals: None,
            trusted_host: Some(principal),
            trusted_request_namespace: Some(request_namespace),
            trusted_request_sequence: Arc::new(AtomicU64::new(0)),
        })
    }

    pub async fn authorize<T: Message>(
        &self,
        request: &Request<T>,
        action: AuthorizationAction,
        scope: AuthorizationScope,
        approval: Option<AuthorizationDecisionId>,
    ) -> Result<AuthorizedGrpcInvocation, Status> {
        let principal = self.authenticate(request).map_err(Status::from)?;
        let request_id = self.request_id(request, action).map_err(Status::from)?;
        let target_digest =
            AuthorizationTargetDigest::for_bytes(&request.get_ref().encode_to_vec());
        let mut authorization =
            AuthorizationRequest::new(request_id, principal.clone(), action, scope, target_digest);
        if let Some(approval) = approval {
            authorization = authorization.with_approval(approval);
        }
        match self
            .authorize
            .execute(authorization)
            .await
            .map_err(super::domain_error_to_status)?
        {
            AuthorizationGateOutcome::Allowed { evidence, .. } => {
                Ok(AuthorizedGrpcInvocation::new(principal, evidence))
            }
            AuthorizationGateOutcome::Denied { decision } => {
                Err(Status::permission_denied(format!(
                    "authorization decision {} denied the operation",
                    decision.id().as_str()
                )))
            }
            AuthorizationGateOutcome::Expired { decision } => {
                Err(Status::permission_denied(format!(
                    "authorization decision {} has expired",
                    decision.id().as_str()
                )))
            }
        }
    }

    fn authenticate<T>(
        &self,
        request: &Request<T>,
    ) -> Result<AuthenticatedPrincipal, GrpcAuthorizationError> {
        if let Some(principals) = &self.mutual_tls_principals {
            return principals.authenticate(request).map_err(Into::into);
        }
        self.trusted_host
            .clone()
            .ok_or(GrpcAuthorizationError::Unconfigured)
    }

    fn request_id<T>(
        &self,
        request: &Request<T>,
        action: AuthorizationAction,
    ) -> Result<AuthorizationRequestId, GrpcAuthorizationError> {
        if let Some(value) = request.metadata().get(REQUEST_ID_HEADER) {
            let value = value
                .to_str()
                .map_err(|_| GrpcAuthorizationError::InvalidRequestIdEncoding)?;
            return AuthorizationRequestId::new(value)
                .map_err(GrpcAuthorizationError::InvalidRequestId);
        }
        let namespace = self
            .trusted_request_namespace
            .as_ref()
            .ok_or(GrpcAuthorizationError::MissingRequestId)?;
        let sequence = self
            .trusted_request_sequence
            .fetch_add(1, Ordering::Relaxed)
            .saturating_add(1);
        AuthorizationRequestId::new(format!("{namespace}:{action:?}:{sequence}"))
            .map_err(GrpcAuthorizationError::InvalidRequestId)
    }
}
