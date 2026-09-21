//! [`GetCeremonyIntegratorBindingUseCase`] — who is driving this, if
//! anyone.

use std::sync::Arc;

use made_core::error::DomainError;
use made_core::ports::IntegratorBindingPort;
use made_core::value_objects::{IntegratorBinding, IntegratorScope};

/// Reads the binding in force for a scope.
///
/// `None` is an ordinary answer and the one a caller has to handle
/// first: a ceremony nobody is driving is the normal state of most
/// ceremonies, and a host that treated the absence as an error would
/// refuse to bind itself to the very thing it was asked to drive.
pub struct GetCeremonyIntegratorBindingUseCase {
    bindings: Arc<dyn IntegratorBindingPort>,
}

impl std::fmt::Debug for GetCeremonyIntegratorBindingUseCase {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("GetCeremonyIntegratorBindingUseCase")
            .finish_non_exhaustive()
    }
}

impl GetCeremonyIntegratorBindingUseCase {
    #[must_use]
    pub fn new(bindings: Arc<dyn IntegratorBindingPort>) -> Self {
        Self { bindings }
    }

    #[tracing::instrument(name = "get_ceremony_integrator_binding", skip_all)]
    pub async fn execute(
        &self,
        scope: &IntegratorScope,
    ) -> Result<Option<IntegratorBinding>, DomainError> {
        self.bindings.current(scope).await
    }
}
