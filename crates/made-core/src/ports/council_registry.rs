//! [`CouncilRegistryPort`] — registry of councils keyed by specialty.

use async_trait::async_trait;

use crate::entities::Council;
use crate::error::DomainError;
use crate::value_objects::{AuthorizationEvidence, Specialty};

#[async_trait]
pub trait CouncilRegistryPort: Send + Sync {
    /// Store a freshly created council. Fails if a council for the
    /// same specialty already exists.
    async fn register(&self, council: Council) -> Result<(), DomainError>;
    async fn register_authorized(
        &self,
        council: Council,
        authorization: Option<AuthorizationEvidence>,
    ) -> Result<(), DomainError> {
        reject_unsupported(authorization.as_ref())?;
        self.register(council).await
    }

    /// Replace an existing council for a specialty. Fails if no
    /// council exists for the specialty.
    async fn replace(&self, council: Council) -> Result<(), DomainError>;
    async fn replace_authorized(
        &self,
        council: Council,
        authorization: Option<AuthorizationEvidence>,
    ) -> Result<(), DomainError> {
        reject_unsupported(authorization.as_ref())?;
        self.replace(council).await
    }

    /// Fetch the council for a specialty. Returns
    /// [`DomainError::NotFound`] when absent.
    async fn get(&self, specialty: &Specialty) -> Result<Council, DomainError>;

    /// Enumerate every registered council.
    async fn list(&self) -> Result<Vec<Council>, DomainError>;

    /// Remove the council for a specialty. Returns
    /// [`DomainError::NotFound`] when absent.
    async fn delete(&self, specialty: &Specialty) -> Result<(), DomainError>;
    async fn delete_authorized(
        &self,
        specialty: &Specialty,
        authorization: Option<AuthorizationEvidence>,
    ) -> Result<(), DomainError> {
        reject_unsupported(authorization.as_ref())?;
        self.delete(specialty).await
    }

    /// Cheap existence check.
    async fn contains(&self, specialty: &Specialty) -> Result<bool, DomainError>;
}

fn reject_unsupported(authorization: Option<&AuthorizationEvidence>) -> Result<(), DomainError> {
    if authorization.is_some() {
        return Err(DomainError::InvariantViolated {
            reason: "council registry adapter cannot persist authorization evidence",
        });
    }
    Ok(())
}
