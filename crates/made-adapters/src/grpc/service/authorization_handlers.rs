use made_app::authorization::AuthorizationMutationOutcome;
use made_core::value_objects::{
    AuthorizationDecisionId, AuthorizationDecisionPageLimit, AuthorizationGrantId,
    AuthorizationRevocationReason,
};
use made_proto::v1 as pb;
use tonic::{Request, Response};

use super::{domain_error_to_status, GrpcResult, MadeGrpcService};
use crate::grpc::mappers::{authorization_decision_to_proto, authorization_policy_to_proto};

impl MadeGrpcService {
    pub(super) async fn handle_get_authorization_policy(
        &self,
        _request: Request<pb::GetAuthorizationPolicyRequest>,
    ) -> GrpcResult<pb::GetAuthorizationPolicyResponse> {
        let snapshot = self
            .read_authorization_policy
            .execute()
            .await
            .map_err(domain_error_to_status)?;
        Ok(Response::new(pb::GetAuthorizationPolicyResponse {
            policy: Some(authorization_policy_to_proto(&snapshot.policy)),
        }))
    }

    pub(super) async fn handle_revoke_authorization_grant(
        &self,
        principal: &made_core::value_objects::AuthenticatedPrincipal,
        request: pb::RevokeAuthorizationGrantRequest,
    ) -> GrpcResult<pb::RevokeAuthorizationGrantResponse> {
        let outcome = self
            .authorization_administration
            .revoke(
                principal,
                &AuthorizationGrantId::new(request.grant_id).map_err(domain_error_to_status)?,
                AuthorizationRevocationReason::new(request.reason)
                    .map_err(domain_error_to_status)?,
            )
            .await
            .map_err(domain_error_to_status)?;
        let (version, existing) = mutation_response(outcome);
        Ok(Response::new(pb::RevokeAuthorizationGrantResponse {
            version,
            existing,
        }))
    }

    pub(super) async fn handle_list_authorization_decisions(
        &self,
        request: Request<pb::ListAuthorizationDecisionsRequest>,
    ) -> GrpcResult<pb::ListAuthorizationDecisionsResponse> {
        let request = request.into_inner();
        let after = request
            .after_decision_id
            .map(AuthorizationDecisionId::new)
            .transpose()
            .map_err(domain_error_to_status)?;
        let limit = AuthorizationDecisionPageLimit::new(if request.limit == 0 {
            100
        } else {
            request.limit as usize
        })
        .map_err(domain_error_to_status)?;
        let page = self
            .read_authorization_decisions
            .execute(after.as_ref(), limit)
            .await
            .map_err(domain_error_to_status)?;
        let next_after_decision_id = (page.decisions().len() == limit.value())
            .then(|| page.decisions().last())
            .flatten()
            .map(|decision| decision.id().as_str().to_owned());
        Ok(Response::new(pb::ListAuthorizationDecisionsResponse {
            decisions: page
                .decisions()
                .iter()
                .map(authorization_decision_to_proto)
                .collect(),
            next_after_decision_id,
        }))
    }
}

pub(super) fn mutation_response(outcome: AuthorizationMutationOutcome) -> (u64, bool) {
    match outcome {
        AuthorizationMutationOutcome::Applied { version } => (version.value(), false),
        AuthorizationMutationOutcome::Existing { version } => (version.value(), true),
    }
}
