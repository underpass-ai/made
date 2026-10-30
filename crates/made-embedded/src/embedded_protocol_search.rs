use std::sync::Arc;

use made_app::authorization::TrustedHostAuthorizationGate;
use made_app::services::{AuthorizationOperationScope, SessionStream};
use made_app::usecases::{
    ProtocolInstancePage, ProtocolSearchCursorCodec, SearchProtocolInstancesInput,
    SearchProtocolInstancesUseCase,
};
use made_core::error::DomainError;
use made_core::ports::ProtocolInstanceIndexPort;
use made_core::value_objects::{AuthorizationAction, AuthorizationRequestId, AuthorizationScope};

pub(crate) async fn execute(
    authorization: Option<&Arc<TrustedHostAuthorizationGate>>,
    cursors: Option<ProtocolSearchCursorCodec>,
    index: Arc<dyn ProtocolInstanceIndexPort>,
    stream: Arc<SessionStream>,
    request_id: AuthorizationRequestId,
    input: &SearchProtocolInstancesInput,
) -> Result<ProtocolInstancePage, DomainError> {
    let target_digest = input.authorization_target_digest();
    if let Some(operation) = AuthorizationOperationScope::current() {
        let evidence = operation.evidence();
        if evidence.action() != AuthorizationAction::SearchProtocolInstances
            || evidence.scope() != &AuthorizationScope::Global
            || evidence.target_digest() != &target_digest
        {
            return Err(DomainError::InvariantViolated {
                reason: "active authorization evidence does not admit this protocol search",
            });
        }
    } else {
        authorization
            .ok_or(DomainError::InvariantViolated {
                reason: "embedded protocol search requires an explicit authorization gate",
            })?
            .authorize(
                request_id,
                AuthorizationAction::SearchProtocolInstances,
                AuthorizationScope::Global,
                target_digest,
                None,
            )
            .await?;
    }
    let cursors = cursors.ok_or(DomainError::InvariantViolated {
        reason: "protocol search cursors require an explicit stable key and namespace",
    })?;
    SearchProtocolInstancesUseCase::new(index, stream, cursors)
        .execute(input)
        .await
}
