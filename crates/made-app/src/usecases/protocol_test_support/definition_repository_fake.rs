use std::collections::BTreeMap;

use made_core::entities::ProtocolDefinition;
use made_core::value_objects::{ProtocolName, ProtocolVersion};
use tokio::sync::RwLock;

#[derive(Debug, Default)]
pub(in crate::usecases) struct DefinitionRepositoryFake {
    pub(super) inner: RwLock<BTreeMap<(ProtocolName, ProtocolVersion), ProtocolDefinition>>,
}
