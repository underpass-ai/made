use std::sync::Arc;

use made_app::authorization::TrustedHostAuthorizationGate;
use made_app::services::{AuthorizationOperationScope, SessionStream};
use made_app::usecases::{
    CeremonyInstancePage, CeremonySearchCursorCodec, SearchCeremonyInstancesInput,
    SearchCeremonyInstancesUseCase,
};
use made_core::error::DomainError;
use made_core::ports::CeremonyInstanceIndexPort;
use made_core::value_objects::{AuthorizationAction, AuthorizationRequestId, AuthorizationScope};

pub(crate) async fn execute(
    authorization: Option<&Arc<TrustedHostAuthorizationGate>>,
    cursors: Option<CeremonySearchCursorCodec>,
    index: Arc<dyn CeremonyInstanceIndexPort>,
    stream: Arc<SessionStream>,
    request_id: AuthorizationRequestId,
    input: &SearchCeremonyInstancesInput,
) -> Result<CeremonyInstancePage, DomainError> {
    let target_digest = input.authorization_target_digest();
    if let Some(operation) = AuthorizationOperationScope::current() {
        let evidence = operation.evidence();
        if evidence.action() != AuthorizationAction::SearchCeremonyInstances
            || evidence.scope() != &AuthorizationScope::Global
            || evidence.target_digest() != &target_digest
        {
            return Err(DomainError::InvariantViolated {
                reason: "active authorization evidence does not admit this ceremony search",
            });
        }
    } else {
        authorization
            .ok_or(DomainError::InvariantViolated {
                reason: "embedded ceremony search requires an explicit authorization gate",
            })?
            .authorize(
                request_id,
                AuthorizationAction::SearchCeremonyInstances,
                AuthorizationScope::Global,
                target_digest,
                None,
            )
            .await?;
    }
    let cursors = cursors.ok_or(DomainError::InvariantViolated {
        reason: "ceremony search cursors require an explicit stable key and namespace",
    })?;
    SearchCeremonyInstancesUseCase::new(index, stream, cursors)
        .execute(input)
        .await
}
