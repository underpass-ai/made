use std::collections::BTreeMap;

use made_core::entities::CeremonyDefinition;
use made_core::value_objects::{CeremonyName, CeremonyVersion};
use tokio::sync::RwLock;

#[derive(Debug, Default)]
pub(in crate::usecases) struct DefinitionRepositoryFake {
    pub(super) inner: RwLock<BTreeMap<(CeremonyName, CeremonyVersion), CeremonyDefinition>>,
}
