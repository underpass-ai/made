use made_proto::v1::{
    AuthorizationPolicyRecord, IssueAuthorizationGrantRequest, IssueAuthorizationGrantResponse,
    ListAuthorizationDecisionsRequest, ListAuthorizationDecisionsResponse,
    RevokeAuthorizationGrantRequest, RevokeAuthorizationGrantResponse,
};

use crate::{MadeClient, MadeClientError};

impl MadeClient {
    pub async fn authorization_policy(&self) -> Result<AuthorizationPolicyRecord, MadeClientError> {
        let response = self
            .rpc()
            .get_authorization_policy(Self::request(
                &self.context(),
                "/underpass.made.v1.MadeService/GetAuthorizationPolicy",
                made_proto::v1::GetAuthorizationPolicyRequest {},
            ))
            .await
            .map_err(MadeClientError::from_status)?
            .into_inner();
        response.policy.ok_or_else(|| {
            MadeClientError::ProtocolViolation(
                "get authorization policy response has no policy".to_owned(),
            )
        })
    }

    pub async fn issue_authorization_grant(
        &self,
        request: IssueAuthorizationGrantRequest,
    ) -> Result<IssueAuthorizationGrantResponse, MadeClientError> {
        self.rpc()
            .issue_authorization_grant(Self::request(
                &self.context(),
                "/underpass.made.v1.MadeService/IssueAuthorizationGrant",
                request,
            ))
            .await
            .map(tonic::Response::into_inner)
            .map_err(MadeClientError::from_status)
    }

    pub async fn revoke_authorization_grant(
        &self,
        grant_id: impl Into<String>,
        reason: impl Into<String>,
    ) -> Result<RevokeAuthorizationGrantResponse, MadeClientError> {
        self.rpc()
            .revoke_authorization_grant(Self::request(
                &self.context(),
                "/underpass.made.v1.MadeService/RevokeAuthorizationGrant",
                RevokeAuthorizationGrantRequest {
                    grant_id: grant_id.into(),
                    reason: reason.into(),
                },
            ))
            .await
            .map(tonic::Response::into_inner)
            .map_err(MadeClientError::from_status)
    }

    pub async fn authorization_decisions(
        &self,
        after_decision_id: Option<String>,
        limit: u32,
    ) -> Result<ListAuthorizationDecisionsResponse, MadeClientError> {
        self.rpc()
            .list_authorization_decisions(Self::request(
                &self.context(),
                "/underpass.made.v1.MadeService/ListAuthorizationDecisions",
                ListAuthorizationDecisionsRequest {
                    after_decision_id,
                    limit,
                },
            ))
            .await
            .map(tonic::Response::into_inner)
            .map_err(MadeClientError::from_status)
    }
}
