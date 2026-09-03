use std::sync::Arc;

use made_core::entities::AuditFact;
use tokio::sync::RwLock;

use super::InstanceRepositoryFake;

#[derive(Debug)]
pub(in crate::usecases) struct UnitOfWorkFake {
    pub(super) instances: Arc<InstanceRepositoryFake>,
    pub(super) facts: RwLock<Vec<AuditFact>>,
}
