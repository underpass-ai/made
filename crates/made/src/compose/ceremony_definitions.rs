use std::sync::Arc;

use made_adapters::memory::InMemoryCeremonyDefinitionRepository;
use made_core::ports::CeremonyDefinitionRepositoryPort;

pub(super) fn wire() -> Arc<dyn CeremonyDefinitionRepositoryPort> {
    Arc::new(InMemoryCeremonyDefinitionRepository::new())
}
