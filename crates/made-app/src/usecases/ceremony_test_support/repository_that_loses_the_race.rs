use std::sync::Arc;

use super::{InstanceRepositoryFake, UnitOfWorkFake};

#[derive(Debug)]
pub(in crate::usecases) struct ARepositoryThatLosesTheRace {
    pub(super) instances: Arc<InstanceRepositoryFake>,
    pub(super) unit_of_work: Arc<UnitOfWorkFake>,
}
