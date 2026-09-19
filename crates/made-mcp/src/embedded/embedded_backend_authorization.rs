use made_app::authorization::{ReadAuthorizationPolicyUseCase, TrustedHostAuthorizationGate};
use made_embedded::EmbeddedMade;

use super::{embedded_tool_authorizer::EmbeddedToolAuthorizer, EmbeddedMadeMcpBackend};

impl EmbeddedMadeMcpBackend {
    /// Protect every embedded tool invocation with the configured local policy.
    #[must_use]
    pub fn with_authorization(
        made: EmbeddedMade,
        gate: TrustedHostAuthorizationGate,
        read_policy: ReadAuthorizationPolicyUseCase,
    ) -> Self {
        Self {
            made,
            authorization: Some(EmbeddedToolAuthorizer::new(gate, read_policy)),
        }
    }
}
