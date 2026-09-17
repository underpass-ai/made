use made_app::usecases::CeremonyInstanceView;
use made_core::entities::{CeremonyDefinition, CeremonyInstance};
use made_core::error::DomainError;

use crate::EmbeddedMade;

/// Projects an embedded ceremony using the engine's configured clock and
/// concurrency ceiling.
pub trait EmbeddedCeremonyProjection {
    fn project_instance<'a>(
        &self,
        instance: &'a CeremonyInstance,
        definition: &'a CeremonyDefinition,
    ) -> Result<CeremonyInstanceView<'a>, DomainError>;
}

impl EmbeddedCeremonyProjection for EmbeddedMade {
    fn project_instance<'a>(
        &self,
        instance: &'a CeremonyInstance,
        definition: &'a CeremonyDefinition,
    ) -> Result<CeremonyInstanceView<'a>, DomainError> {
        CeremonyInstanceView::project_at(
            instance,
            definition,
            self.clock.now(),
            self.max_parallel_ceiling,
        )
    }
}
