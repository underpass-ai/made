use std::collections::BTreeMap;

use made_core::entities::CeremonyInstance;
use made_core::value_objects::{CeremonyId, CeremonyRevision};
use tokio::sync::RwLock;

#[derive(Debug, Default)]
pub(in crate::usecases) struct InstanceRepositoryFake {
    pub(super) inner: RwLock<BTreeMap<CeremonyId, CeremonyInstance>>,
    pub(super) revisions: RwLock<BTreeMap<CeremonyId, CeremonyRevision>>,
}
