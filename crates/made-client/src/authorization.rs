use made_proto::v1::{
    ApproveAuthorizationOperationRequest, AuthorizationDecisionRecord, AuthorizationPolicyRecord,
    IssueAuthorizationGrantRequest, IssueAuthorizationGrantResponse,
    ListAuthorizationDecisionsRequest, ListAuthorizationDecisionsResponse,
    RevokeAuthorizationGrantRequest, RevokeAuthorizationGrantResponse,
};

use crate::{MadeClient, MadeClientError};

impl MadeClient {
    pub async fn approve_authorization_operation(
        &self,
        request: ApproveAuthorizationOperationRequest,
    ) -> Result<AuthorizationDecisionRecord, MadeClientError> {
        let response = self
            .rpc()
            .approve_authorization_operation(Self::request(
                &self.context(),
                "/underpass.made.v1.MadeService/ApproveAuthorizationOperation",
                request,
            ))
            .await
            .map_err(MadeClientError::from_status)?
            .into_inner();
        response.decision.ok_or_else(|| {
            MadeClientError::ProtocolViolation(
                "approve authorization operation response has no decision".to_owned(),
            )
        })
    }

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

/// Compute the exact target digest used by the direct gRPC authorization gate.
///
/// Callers construct the final execution request first, approve this digest,
/// and then send those same protobuf fields with the returned decision ID.
#[must_use]
pub fn authorization_target_digest<T: prost::Message>(request: &T) -> String {
    use sha2::{Digest, Sha256};

    format!("{:x}", Sha256::digest(request.encode_to_vec()))
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use made_proto::v1::BindCeremonyParticipantsRequest;

    use super::authorization_target_digest;

    #[test]
    fn target_digest_is_stable_for_canonical_protobuf_maps() {
        let left = BindCeremonyParticipantsRequest {
            ceremony_id: "ceremony-1".to_owned(),
            seating: BTreeMap::from([
                ("reviewer".to_owned(), "review".to_owned()),
                ("author".to_owned(), "writing".to_owned()),
            ]),
            ..Default::default()
        };
        let right = BindCeremonyParticipantsRequest {
            ceremony_id: "ceremony-1".to_owned(),
            seating: BTreeMap::from([
                ("author".to_owned(), "writing".to_owned()),
                ("reviewer".to_owned(), "review".to_owned()),
            ]),
            ..Default::default()
        };

        assert_eq!(
            authorization_target_digest(&left),
            authorization_target_digest(&right)
        );
    }
}
