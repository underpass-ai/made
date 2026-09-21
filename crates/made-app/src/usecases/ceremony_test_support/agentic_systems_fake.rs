use async_trait::async_trait;
use made_core::entities::AgenticSystem;
use made_core::error::DomainError;
use made_core::ports::{
    AgenticSystemPage, AgenticSystemQuery, AgenticSystemRepositoryPort, AgenticSystemSaveOutcome,
};
use made_core::value_objects::{AgenticSystemId, AgenticSystemRevision};

/// The design side of the same absent composition.
pub(in crate::usecases) struct AgenticSystemsFake;

#[async_trait]
impl AgenticSystemRepositoryPort for AgenticSystemsFake {
    async fn save(
        &self,
        _system: AgenticSystem,
        _expected: Option<AgenticSystemRevision>,
    ) -> Result<AgenticSystemSaveOutcome, DomainError> {
        unimplemented!("a projection only reads")
    }

    async fn get(
        &self,
        _id: &AgenticSystemId,
        _revision: Option<AgenticSystemRevision>,
    ) -> Result<Option<AgenticSystem>, DomainError> {
        unimplemented!("these bindings are scoped to a ceremony")
    }

    async fn list(&self, _query: &AgenticSystemQuery) -> Result<AgenticSystemPage, DomainError> {
        unimplemented!("the resolver asks for one design")
    }
}
