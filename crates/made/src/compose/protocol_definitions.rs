use std::sync::Arc;

use made_adapters::memory::InMemoryProtocolDefinitionRepository;
use made_core::ports::ProtocolDefinitionRepositoryPort;

pub(super) fn wire() -> Arc<dyn ProtocolDefinitionRepositoryPort> {
    Arc::new(InMemoryProtocolDefinitionRepository::new())
}

/// Reading a definition is asking two places in one fixed order: what
/// a run was pinned to, then what is mounted. Every caller needs the
/// same order, so it is composed once.
pub(super) fn resolver(
    definitions: &Arc<dyn ProtocolDefinitionRepositoryPort>,
    publications: &Arc<dyn made_core::ports::ProtocolDefinitionPublicationPort>,
) -> Arc<made_app::usecases::ResolveProtocolDefinitionUseCase> {
    Arc::new(made_app::usecases::ResolveProtocolDefinitionUseCase::new(
        definitions.clone(),
        publications.clone(),
    ))
}
