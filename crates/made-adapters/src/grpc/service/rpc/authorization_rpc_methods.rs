macro_rules! authorization_rpc_methods {
    ($callback:ident) => {
        council_rpc_methods!($callback; {
        async fn get_authorization_policy(
            &self,
            request: Request<pb::GetAuthorizationPolicyRequest>,
        ) -> GrpcResult<pb::GetAuthorizationPolicyResponse> {
            authorized_global!(
                self,
                request,
                ReadAuthorizationPolicy,
                self.handle_get_authorization_policy(request)
            )
        }

        async fn issue_authorization_grant(
            &self,
            request: Request<pb::IssueAuthorizationGrantRequest>,
        ) -> GrpcResult<pb::IssueAuthorizationGrantResponse> {
            let principal = self.authorization.authenticate(&request)?;
            let grant = crate::grpc::mappers::authorization_grant_from_proto(
                request.get_ref().clone(),
                &principal,
            )
            .map_err(crate::grpc::domain_error_to_status)?;
            let authorization = self
                .authorization
                .authorize_authenticated(
                    &request,
                    principal.clone(),
                    AuthorizationAction::IssueAuthorizationGrant,
                    grant.scope().clone(),
                    None,
                )
                .await?;
            AuthorizationOperationScope::run(authorization, async {
                let outcome = self
                    .authorization_administration
                    .issue(&principal, grant)
                    .await
                    .map_err(crate::grpc::domain_error_to_status)?;
                let (version, existing) = super::authorization_handlers::mutation_response(outcome);
                Ok(Response::new(pb::IssueAuthorizationGrantResponse { version, existing }))
            })
            .await
        }

        async fn revoke_authorization_grant(
            &self,
            request: Request<pb::RevokeAuthorizationGrantRequest>,
        ) -> GrpcResult<pb::RevokeAuthorizationGrantResponse> {
            let principal = self.authorization.authenticate(&request)?;
            let grant_id = made_core::value_objects::AuthorizationGrantId::new(
                request.get_ref().grant_id.clone(),
            )
            .map_err(crate::grpc::domain_error_to_status)?;
            let snapshot = self
                .read_authorization_policy
                .execute()
                .await
                .map_err(crate::grpc::domain_error_to_status)?;
            let scope = snapshot
                .policy
                .grants()
                .find(|grant| grant.id() == &grant_id)
                .map(|grant| grant.scope().clone());
            let authorization = self
                .authorization
                .authorize_authenticated(
                    &request,
                    principal.clone(),
                    AuthorizationAction::RevokeAuthorizationGrant,
                    scope.clone().unwrap_or(AuthorizationScope::Global),
                    None,
                )
                .await?;
            if scope.is_none() {
                return Err(tonic::Status::not_found("authorization grant not found"));
            }
            AuthorizationOperationScope::run(
                authorization,
                self.handle_revoke_authorization_grant(&principal, request.into_inner()),
            )
            .await
        }

        async fn list_authorization_decisions(
            &self,
            request: Request<pb::ListAuthorizationDecisionsRequest>,
        ) -> GrpcResult<pb::ListAuthorizationDecisionsResponse> {
            authorized_global!(
                self,
                request,
                ReadAuthorizationDecisions,
                self.handle_list_authorization_decisions(request)
            )
        }

        async fn approve_authorization_operation(
            &self,
            request: Request<pb::ApproveAuthorizationOperationRequest>,
        ) -> GrpcResult<pb::ApproveAuthorizationOperationResponse> {
            let approval_action = crate::grpc::mappers::authorization_action_from_proto(
                &request.get_ref().approval_action,
            )
            .map_err(crate::grpc::domain_error_to_status)?;
            let execution_action = crate::grpc::mappers::authorization_action_from_proto(
                &request.get_ref().execution_action,
            )
            .map_err(crate::grpc::domain_error_to_status)?;
            let scope = crate::grpc::mappers::authorization_scope_from_proto(
                request
                    .get_ref()
                    .scope
                    .clone()
                    .ok_or_else(|| tonic::Status::invalid_argument("scope is required"))?,
            )
            .map_err(crate::grpc::domain_error_to_status)?;
            let target_digest = made_core::value_objects::AuthorizationTargetDigest::new(
                request.get_ref().target_digest.clone(),
            )
            .map_err(crate::grpc::domain_error_to_status)?;
            let decision = self
                .authorization
                .approve_authorization_operation(
                    &request,
                    approval_action,
                    execution_action,
                    scope,
                    target_digest,
                )
                .await?;
            Ok(Response::new(pb::ApproveAuthorizationOperationResponse {
                decision: Some(crate::grpc::mappers::authorization_decision_to_proto(
                    &decision,
                )),
            }))
        }
        });
    };
}

pub(super) use authorization_rpc_methods;
