use std::sync::Arc;

use made_core::ports::{
    AgenticSystemDiagramPort, AgenticSystemExecutionStorePort, AgenticSystemPublicationPort,
    AgenticSystemRepositoryPort,
};

/// The agentic-system stores the service composed, held for the
/// operations that will use them.
///
/// One handle rather than four fields on the application, because the
/// four are never chosen separately: a durable revision log beside an
/// in-memory publication store would restart into a service that had
/// forgotten which revisions its runs are pinned to.
#[derive(Clone)]
pub struct AgenticSystemHandles {
    pub repository: Arc<dyn AgenticSystemRepositoryPort>,
    pub publications: Arc<dyn AgenticSystemPublicationPort>,
    pub executions: Arc<dyn AgenticSystemExecutionStorePort>,
    pub diagrams: Arc<dyn AgenticSystemDiagramPort>,
}

impl std::fmt::Debug for AgenticSystemHandles {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.debug_struct("AgenticSystemHandles").finish()
    }
}
