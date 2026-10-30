use made_app::usecases::ProtocolInstanceView;
use made_core::entities::{ProtocolDefinition, ProtocolInstance};
use made_core::error::DomainError;

use crate::EmbeddedMade;

/// Projects an embedded protocol using the engine's configured clock and
/// concurrency ceiling.
pub trait EmbeddedProtocolProjection {
    fn project_instance<'a>(
        &self,
        instance: &'a ProtocolInstance,
        definition: &'a ProtocolDefinition,
    ) -> Result<ProtocolInstanceView<'a>, DomainError>;
}

impl EmbeddedProtocolProjection for EmbeddedMade {
    fn project_instance<'a>(
        &self,
        instance: &'a ProtocolInstance,
        definition: &'a ProtocolDefinition,
    ) -> Result<ProtocolInstanceView<'a>, DomainError> {
        ProtocolInstanceView::project_at(
            instance,
            definition,
            self.clock.now(),
            self.max_parallel_ceiling,
        )
    }
}
