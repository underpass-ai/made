//! [`GetPublishedCeremonyDefinitionUseCase`] — read one published
//! version back, with the digest it was fixed to.

use std::fmt;
use std::sync::Arc;

use made_core::entities::PublishedCeremonyDefinition;
use made_core::error::DomainError;
use made_core::ports::CeremonyDefinitionPublicationPort;
use made_core::value_objects::{CeremonyName, CeremonyVersion};

pub struct GetPublishedCeremonyDefinitionUseCase {
    publications: Arc<dyn CeremonyDefinitionPublicationPort>,
}

impl fmt::Debug for GetPublishedCeremonyDefinitionUseCase {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("GetPublishedCeremonyDefinitionUseCase")
            .finish()
    }
}

impl GetPublishedCeremonyDefinitionUseCase {
    #[must_use]
    pub fn new(publications: Arc<dyn CeremonyDefinitionPublicationPort>) -> Self {
        Self { publications }
    }

    /// Only the catalogue, never the repository of mounted definitions:
    /// "what was published under this version" answered from a mounted
    /// document would be a different question with the same name.
    #[tracing::instrument(
        name = "get_published_ceremony_definition",
        skip_all,
        fields(name = %name, version = %version)
    )]
    pub async fn execute(
        &self,
        name: &CeremonyName,
        version: &CeremonyVersion,
    ) -> Result<PublishedCeremonyDefinition, DomainError> {
        self.publications
            .published(name, version)
            .await?
            .ok_or(DomainError::NotFound {
                what: "published_ceremony_definition",
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::usecases::ceremony_test_support::{definition, PublicationsFake};

    #[tokio::test]
    async fn a_published_version_comes_back_with_its_digest() {
        let publications = Arc::new(PublicationsFake::default());
        let sealed = publications.seed(definition()).await;
        let usecase = GetPublishedCeremonyDefinitionUseCase::new(publications);

        let read = usecase
            .execute(sealed.name(), sealed.version())
            .await
            .unwrap();

        assert_eq!(read, sealed);
        assert_eq!(read.digest(), definition().digest().unwrap());
    }

    #[tokio::test]
    async fn an_unpublished_version_is_not_found() {
        let usecase =
            GetPublishedCeremonyDefinitionUseCase::new(Arc::new(PublicationsFake::default()));

        let error = usecase
            .execute(definition().name(), &CeremonyVersion::new("9.9").unwrap())
            .await
            .unwrap_err();

        assert!(matches!(error, DomainError::NotFound { .. }), "{error:?}");
    }
}
