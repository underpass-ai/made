use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use made_app::authorization::{AuthorizationGateOutcome, AuthorizeOperationUseCase};
use made_core::value_objects::{
    AuthenticatedPrincipal, AuthenticationMethod, AuthorizationAction, AuthorizationDecision,
    AuthorizationDecisionId, AuthorizationRequest, AuthorizationRequestId, AuthorizationScope,
    AuthorizationTargetDigest, AuthorizedOperation, PrincipalKind,
};
use prost::Message;
use tonic::{Request, Status};

use super::{GrpcAuthorizationError, MutualTlsPrincipalMap};

const REQUEST_ID_HEADER: &str = "x-made-request-id";
const TARGET_DIGEST_HEADER: &str = "x-made-target-digest";
const APPROVAL_DECISION_HEADER: &str = "x-made-approval-decision-id";

/// Authenticates a gRPC caller and persists the policy decision before dispatch.
#[derive(Debug, Clone)]
pub struct GrpcAuthorizationGate {
    authorize: Arc<AuthorizeOperationUseCase>,
    mutual_tls_principals: Option<Arc<MutualTlsPrincipalMap>>,
    trusted_host: Option<AuthenticatedPrincipal>,
    target_digest_proxy_principals: Arc<Vec<AuthenticatedPrincipal>>,
    trusted_request_namespace: Option<String>,
    trusted_request_sequence: Arc<AtomicU64>,
}

impl GrpcAuthorizationGate {
    /// Keep internal appends on the same policy as this transport boundary.
    pub fn protect_session_stream(&self, stream: &made_app::services::SessionStream) {
        stream.authorize_appends(Arc::new(self.authorize.clone().for_ceremony_appends()));
    }

    #[must_use]
    pub fn mutual_tls(
        authorize: Arc<AuthorizeOperationUseCase>,
        principals: Arc<MutualTlsPrincipalMap>,
    ) -> Self {
        Self {
            authorize,
            mutual_tls_principals: Some(principals),
            trusted_host: None,
            target_digest_proxy_principals: Arc::new(Vec::new()),
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
            target_digest_proxy_principals: Arc::new(Vec::new()),
            trusted_request_namespace: Some(request_namespace),
            trusted_request_sequence: Arc::new(AtomicU64::new(0)),
        })
    }

    /// Trust transport-neutral MCP target digests only from these exact,
    /// already-authenticated proxy identities. An empty set keeps the boundary
    /// closed and direct gRPC callers continue to use their protobuf bytes.
    #[must_use]
    pub fn with_target_digest_proxy_principals(
        mut self,
        principals: Vec<AuthenticatedPrincipal>,
    ) -> Self {
        self.target_digest_proxy_principals = Arc::new(principals);
        self
    }

    pub async fn authorize<T: Message>(
        &self,
        request: &Request<T>,
        action: AuthorizationAction,
        scope: AuthorizationScope,
        approval: Option<AuthorizationDecisionId>,
    ) -> Result<AuthorizedOperation, Status> {
        let principal = self.authenticate(request)?;
        self.authorize_authenticated(request, principal, action, scope, approval)
            .await
    }

    pub async fn authorize_authenticated<T: Message>(
        &self,
        request: &Request<T>,
        principal: AuthenticatedPrincipal,
        action: AuthorizationAction,
        scope: AuthorizationScope,
        approval: Option<AuthorizationDecisionId>,
    ) -> Result<AuthorizedOperation, Status> {
        let request_id = self.request_id(request, action).map_err(Status::from)?;
        let target_digest = self
            .target_digest_for_authenticated(request, &principal)
            .map_err(Status::from)?;
        let mut authorization =
            AuthorizationRequest::new(request_id, principal.clone(), action, scope, target_digest);
        let approval = match approval {
            Some(approval) => Some(approval),
            None => Self::approval_decision_id(request).map_err(Status::from)?,
        };
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
                AuthorizedOperation::new(principal, evidence).map_err(super::domain_error_to_status)
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

    pub async fn approve_authorization_operation<T: Message>(
        &self,
        request: &Request<T>,
        approval_action: AuthorizationAction,
        execution_action: AuthorizationAction,
        scope: AuthorizationScope,
        target_digest: AuthorizationTargetDigest,
    ) -> Result<AuthorizationDecision, Status> {
        let principal = self.authenticate(request)?;
        let request_id = self
            .request_id(request, approval_action)
            .map_err(Status::from)?;
        let authorization =
            AuthorizationRequest::new(request_id, principal, approval_action, scope, target_digest)
                .with_approved_action(execution_action);
        match self
            .authorize
            .execute(authorization)
            .await
            .map_err(super::domain_error_to_status)?
        {
            AuthorizationGateOutcome::Allowed { decision, .. } => Ok(decision),
            AuthorizationGateOutcome::Denied { decision } => {
                Err(Status::permission_denied(format!(
                    "authorization decision {} denied the approval",
                    decision.id().as_str()
                )))
            }
            AuthorizationGateOutcome::Expired { decision } => {
                Err(Status::permission_denied(format!(
                    "authorization decision {} expired before approval",
                    decision.id().as_str()
                )))
            }
        }
    }

    pub(super) fn target_digest_for_authenticated<T: Message>(
        &self,
        request: &Request<T>,
        principal: &AuthenticatedPrincipal,
    ) -> Result<AuthorizationTargetDigest, GrpcAuthorizationError> {
        let Some(value) = request.metadata().get(TARGET_DIGEST_HEADER) else {
            return Ok(target_digest(request.get_ref()));
        };
        if !self
            .target_digest_proxy_principals
            .iter()
            .any(|trusted| trusted == principal)
        {
            return Err(GrpcAuthorizationError::UntrustedTargetDigestProxy);
        }
        let value = value
            .to_str()
            .map_err(|_| GrpcAuthorizationError::InvalidTargetDigestEncoding)?;
        AuthorizationTargetDigest::new(value).map_err(GrpcAuthorizationError::InvalidTargetDigest)
    }

    pub fn authenticate<T>(
        &self,
        request: &Request<T>,
    ) -> Result<AuthenticatedPrincipal, GrpcAuthorizationError> {
        if let Some(principals) = &self.mutual_tls_principals {
            return principals
                .authenticate(request)
                .map_err(GrpcAuthorizationError::from);
        }
        self.trusted_host
            .clone()
            .ok_or(GrpcAuthorizationError::Unconfigured)
    }

    pub(super) fn request_id<T>(
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

    fn approval_decision_id<T>(
        request: &Request<T>,
    ) -> Result<Option<AuthorizationDecisionId>, GrpcAuthorizationError> {
        request
            .metadata()
            .get(APPROVAL_DECISION_HEADER)
            .map(|value| {
                let value = value
                    .to_str()
                    .map_err(|_| GrpcAuthorizationError::InvalidApprovalDecisionIdEncoding)?;
                AuthorizationDecisionId::new(value)
                    .map_err(GrpcAuthorizationError::InvalidApprovalDecisionId)
            })
            .transpose()
    }
}

pub(super) fn target_digest<T: Message>(request: &T) -> AuthorizationTargetDigest {
    AuthorizationTargetDigest::for_bytes(&request.encode_to_vec())
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use made_proto::v1::BindCeremonyParticipantsRequest;

    use super::*;

    #[test]
    fn canonical_map_order_produces_the_same_target_digest() {
        let left = BindCeremonyParticipantsRequest {
            ceremony_id: "ceremony-1".to_owned(),
            seating: BTreeMap::from([
                ("reviewer".to_owned(), "review".to_owned()),
                ("author".to_owned(), "writing".to_owned()),
            ]),
            actor_id: "operator".to_owned(),
            actor_kind: "human".to_owned(),
        };
        let right = BindCeremonyParticipantsRequest {
            ceremony_id: "ceremony-1".to_owned(),
            seating: BTreeMap::from([
                ("author".to_owned(), "writing".to_owned()),
                ("reviewer".to_owned(), "review".to_owned()),
            ]),
            actor_id: "operator".to_owned(),
            actor_kind: "human".to_owned(),
        };

        assert_eq!(target_digest(&left), target_digest(&right));
        assert_eq!(left.encode_to_vec(), right.encode_to_vec());
    }

    #[test]
    fn target_digest_override_requires_the_exact_trusted_principal() {
        let trusted = fixture_principal("proxy", PrincipalKind::Service);
        let other = fixture_principal("proxy", PrincipalKind::Worker);
        let gate = fixture_gate(trusted.clone());
        let mut request = Request::new(BindCeremonyParticipantsRequest::default());
        request.metadata_mut().insert(
            TARGET_DIGEST_HEADER,
            "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
                .parse()
                .unwrap(),
        );

        assert_eq!(
            gate.target_digest_for_authenticated(&request, &trusted)
                .unwrap()
                .as_str(),
            "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
        );
        assert!(matches!(
            gate.target_digest_for_authenticated(&request, &other),
            Err(GrpcAuthorizationError::UntrustedTargetDigestProxy)
        ));
    }

    fn fixture_principal(id: &str, kind: PrincipalKind) -> AuthenticatedPrincipal {
        AuthenticatedPrincipal::new(
            made_core::value_objects::PrincipalId::new(id).unwrap(),
            kind,
            AuthenticationMethod::MutualTls,
        )
        .unwrap()
    }

    fn fixture_gate(principal: AuthenticatedPrincipal) -> GrpcAuthorizationGate {
        use crate::clock::SystemClock;
        use crate::memory::InMemoryAuthorizationPolicyStore;
        use made_app::authorization::AuthorizeOperationUseCase;
        use made_core::value_objects::{AuthorizationDecisionTtl, AuthorizationPolicyId};

        let authorize = Arc::new(AuthorizeOperationUseCase::new(
            AuthorizationPolicyId::new("digest-test").unwrap(),
            Arc::new(InMemoryAuthorizationPolicyStore::new()),
            Arc::new(SystemClock::new()),
            AuthorizationDecisionTtl::from_seconds(60).unwrap(),
        ));
        GrpcAuthorizationGate::mutual_tls(
            authorize,
            Arc::new(MutualTlsPrincipalMap::from_json(
                br#"[{"certificate_sha256":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa","principal_id":"unused","principal_kind":"service"}]"#,
            )
            .unwrap()),
        )
        .with_target_digest_proxy_principals(vec![principal])
    }
}
