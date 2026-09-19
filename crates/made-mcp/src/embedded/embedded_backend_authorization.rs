use made_app::authorization::{
    ContinueAcceptedStepClaimUseCase, ReadAuthorizationPolicyUseCase, TrustedHostAuthorizationGate,
};
use made_core::ports::{ArtifactStorePort, ExecutionReceiptStorePort};
use made_embedded::EmbeddedMade;

use super::{embedded_tool_authorizer::EmbeddedToolAuthorizer, EmbeddedMadeMcpBackend};

impl EmbeddedMadeMcpBackend {
    /// Protect every embedded tool invocation with the configured local policy.
    #[must_use]
    pub fn with_authorization(
        made: EmbeddedMade,
        gate: TrustedHostAuthorizationGate,
        read_policy: ReadAuthorizationPolicyUseCase,
        step_continuation: std::sync::Arc<ContinueAcceptedStepClaimUseCase>,
        artifacts: std::sync::Arc<dyn ArtifactStorePort>,
        execution_receipts: std::sync::Arc<dyn ExecutionReceiptStorePort>,
    ) -> Self {
        let scopes: std::sync::Arc<dyn made_core::ports::AuthorizationScopeResolverPort> =
            std::sync::Arc::new(made.clone());
        Self {
            made,
            authorization: Some(EmbeddedToolAuthorizer::new(
                gate,
                read_policy,
                step_continuation,
                artifacts,
                execution_receipts,
                scopes,
            )),
        }
    }
}
