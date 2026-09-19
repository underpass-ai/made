use std::future::Future;

use made_core::value_objects::AuthorizedOperation;

tokio::task_local! {
    static ACTIVE_AUTHORIZED_OPERATION: AuthorizedOperation;
}

/// Keeps one admitted operation active across every append it produces.
#[derive(Debug)]
pub struct AuthorizationOperationScope;

impl AuthorizationOperationScope {
    pub async fn run<F>(operation: AuthorizedOperation, future: F) -> F::Output
    where
        F: Future,
    {
        ACTIVE_AUTHORIZED_OPERATION.scope(operation, future).await
    }
}

pub(crate) fn current_authorized_operation() -> Option<AuthorizedOperation> {
    ACTIVE_AUTHORIZED_OPERATION.try_with(Clone::clone).ok()
}
