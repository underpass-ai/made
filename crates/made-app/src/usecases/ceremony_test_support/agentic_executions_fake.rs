use async_trait::async_trait;
use made_core::entities::AgenticSystemExecution;
use made_core::error::DomainError;
use made_core::ports::{
    AgenticSystemExecutionCreation, AgenticSystemExecutionStorePort, AgenticSystemExecutionUpdate,
};
use made_core::value_objects::{AgenticSystemExecutionId, AgenticSystemId};
use time::OffsetDateTime;

/// A deployment with no composed systems in it.
///
/// The bindings in these tests are scoped to a ceremony, so the
/// resolver never asks: anything that did ask would be a projection
/// that had started reading a run it was not told about.
pub(in crate::usecases) struct AgenticExecutionsFake;

#[async_trait]
impl AgenticSystemExecutionStorePort for AgenticExecutionsFake {
    async fn create(
        &self,
        _execution: AgenticSystemExecution,
    ) -> Result<AgenticSystemExecutionCreation, DomainError> {
        unimplemented!("a projection only reads")
    }

    async fn get(
        &self,
        _id: &AgenticSystemExecutionId,
    ) -> Result<Option<AgenticSystemExecution>, DomainError> {
        unimplemented!("these bindings are scoped to a ceremony")
    }

    async fn update(
        &self,
        _execution: AgenticSystemExecution,
        _expected_updated_at: OffsetDateTime,
    ) -> Result<AgenticSystemExecutionUpdate, DomainError> {
        unimplemented!("a projection only reads")
    }

    async fn list_by_system(
        &self,
        _id: &AgenticSystemId,
    ) -> Result<Vec<AgenticSystemExecution>, DomainError> {
        unimplemented!("the resolver asks for one run")
    }
}
