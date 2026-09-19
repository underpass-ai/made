use made_app::authorization::{ReadAuthorizationPolicyUseCase, TrustedHostAuthorizationGate};
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
        artifacts: std::sync::Arc<dyn ArtifactStorePort>,
        execution_receipts: std::sync::Arc<dyn ExecutionReceiptStorePort>,
    ) -> Self {
        Self {
            made,
            authorization: Some(EmbeddedToolAuthorizer::new(
                gate,
                read_policy,
                artifacts,
                execution_receipts,
            )),
        }
    }
}
