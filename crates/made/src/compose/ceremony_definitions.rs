use std::sync::Arc;

use made_adapters::memory::InMemoryCeremonyDefinitionRepository;
use made_core::ports::CeremonyDefinitionRepositoryPort;

pub(super) fn wire() -> Arc<dyn CeremonyDefinitionRepositoryPort> {
    Arc::new(InMemoryCeremonyDefinitionRepository::new())
}

/// Reading a definition is asking two places in one fixed order: what
/// a run was pinned to, then what is mounted. Every caller needs the
/// same order, so it is composed once.
pub(super) fn resolver(
    definitions: &Arc<dyn CeremonyDefinitionRepositoryPort>,
    publications: &Arc<dyn made_core::ports::CeremonyDefinitionPublicationPort>,
) -> Arc<made_app::usecases::ResolveCeremonyDefinitionUseCase> {
    Arc::new(made_app::usecases::ResolveCeremonyDefinitionUseCase::new(
        definitions.clone(),
        publications.clone(),
    ))
}
