//! [`ListPublishedCeremonyDefinitionsUseCase`] — page through what has
//! been published.
//!
//! Publishing gave an author a digest and nothing to read it back with:
//! an agent that published a ceremony could not later find out what it
//! had published without holding the YAML it sent. This is the reading
//! half.
//!
//! The port hands back the whole catalogue on purpose — a published
//! catalogue that needs pagination at the store has a curation problem
//! before it has a query problem — so the page is cut here, over the
//! catalogue's public order, and the same cut serves every backend.

use std::fmt;
use std::sync::Arc;

use made_core::error::DomainError;
use made_core::ports::CeremonyDefinitionPublicationPort;
use made_core::value_objects::CeremonyDefinitionCursor;

use super::{PublishedCeremonyDefinitionPage, PublishedCeremonyDefinitionQuery};

pub struct ListPublishedCeremonyDefinitionsUseCase {
    publications: Arc<dyn CeremonyDefinitionPublicationPort>,
}

impl fmt::Debug for ListPublishedCeremonyDefinitionsUseCase {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ListPublishedCeremonyDefinitionsUseCase")
            .finish()
    }
}

impl ListPublishedCeremonyDefinitionsUseCase {
    #[must_use]
    pub fn new(publications: Arc<dyn CeremonyDefinitionPublicationPort>) -> Self {
        Self { publications }
    }

    #[tracing::instrument(name = "list_published_ceremony_definitions", skip_all)]
    pub async fn execute(
        &self,
        query: &PublishedCeremonyDefinitionQuery,
    ) -> Result<PublishedCeremonyDefinitionPage, DomainError> {
        let mut admitted = self
            .publications
            .catalogue()
            .await?
            .into_iter()
            .filter(|published| query.admits(published))
            .collect::<Vec<_>>();
        // No backend promises an order, so the order is imposed here
        // rather than inherited from whichever store answered.
        admitted.sort_by(|left, right| {
            (left.name(), left.version()).cmp(&(right.name(), right.version()))
        });

        let limit = query.limit().as_usize();
        let more = admitted.len() > limit;
        admitted.truncate(limit);
        let next_cursor = more
            .then(|| admitted.last())
            .flatten()
            .map(|last| CeremonyDefinitionCursor::new(last.name().clone(), last.version().clone()));
        Ok(PublishedCeremonyDefinitionPage::new(admitted, next_cursor))
    }
}

#[cfg(test)]
mod tests {
    use made_core::entities::PublishedCeremonyDefinition;
    use made_core::value_objects::{CeremonyDefinitionPageLimit, CeremonyName};

    use super::*;
    use crate::usecases::ceremony_test_support::{
        approval_definition, definition, review_child_definition, PublicationsFake,
    };

    async fn catalogue() -> Arc<PublicationsFake> {
        let publications = Arc::new(PublicationsFake::default());
        for published in [
            review_child_definition(),
            definition(),
            approval_definition(),
        ] {
            publications.seed(published).await;
        }
        publications
    }

    fn names(page: &PublishedCeremonyDefinitionPage) -> Vec<&str> {
        page.definitions()
            .iter()
            .map(|published| published.name().as_str())
            .collect()
    }

    #[tokio::test]
    async fn pages_follow_name_then_version_and_resume_after_the_cursor() {
        let usecase = ListPublishedCeremonyDefinitionsUseCase::new(catalogue().await);
        let two = CeremonyDefinitionPageLimit::new(2).unwrap();

        let first = usecase
            .execute(&PublishedCeremonyDefinitionQuery::new(None, two, None))
            .await
            .unwrap();
        assert_eq!(names(&first), ["approval_ceremony", "editorial_meeting"]);
        let cursor = first.next_cursor().cloned().unwrap();
        assert_eq!(cursor.to_string(), "editorial_meeting@1.0");

        let second = usecase
            .execute(&PublishedCeremonyDefinitionQuery::new(
                None,
                two,
                Some(cursor),
            ))
            .await
            .unwrap();
        assert_eq!(names(&second), ["review_child"]);
        assert!(second.next_cursor().is_none());
    }

    #[tokio::test]
    async fn a_name_narrows_the_catalogue_to_its_versions() {
        let usecase = ListPublishedCeremonyDefinitionsUseCase::new(catalogue().await);

        let page = usecase
            .execute(&PublishedCeremonyDefinitionQuery::new(
                Some(CeremonyName::new("review_child").unwrap()),
                CeremonyDefinitionPageLimit::default(),
                None,
            ))
            .await
            .unwrap();

        assert_eq!(names(&page), ["review_child"]);
        assert_eq!(
            page.definitions()[0].digest(),
            PublishedCeremonyDefinition::seal(review_child_definition())
                .unwrap()
                .digest()
        );
        assert!(page.next_cursor().is_none());
    }

    #[tokio::test]
    async fn an_empty_catalogue_is_an_empty_page() {
        let usecase =
            ListPublishedCeremonyDefinitionsUseCase::new(Arc::new(PublicationsFake::default()));

        let page = usecase
            .execute(&PublishedCeremonyDefinitionQuery::default())
            .await
            .unwrap();

        assert!(page.definitions().is_empty());
        assert!(page.next_cursor().is_none());
    }
}
