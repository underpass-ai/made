use std::collections::BTreeMap;

use made_core::value_objects::{CeremonyId, CeremonyStepContribution};
use tokio::sync::RwLock;

#[derive(Debug, Default)]
pub(in crate::usecases) struct ContextStoreFake {
    pub(super) inner: RwLock<BTreeMap<CeremonyId, Vec<CeremonyStepContribution>>>,
}
