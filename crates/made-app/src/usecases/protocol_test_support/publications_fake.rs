use std::collections::BTreeMap;

use made_core::entities::PublishedProtocolDefinition;
use tokio::sync::RwLock;

#[derive(Debug, Default)]
pub(in crate::usecases) struct PublicationsFake {
    pub(super) published: RwLock<BTreeMap<(String, String), PublishedProtocolDefinition>>,
}
