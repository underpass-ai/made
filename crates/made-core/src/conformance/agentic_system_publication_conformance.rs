//! Conformance suite for [`AgenticSystemPublicationPort`].
//!
//! The property under test is that a sealed revision never changes.
//! A run pins the digest of the design it composed; if the bytes under
//! that revision could be replaced, the pin would prove nothing and
//! the run's evidence would be a claim about a document that no longer
//! exists.

use crate::ports::AgenticSystemPublicationPort;
use crate::value_objects::AgenticSystemRevision;

use super::agentic_system_fixtures::{call, failure, sealed, system_id};
use super::ConformanceFailure;

/// Every property an [`AgenticSystemPublicationPort`] implementation
/// must satisfy.
#[derive(Debug)]
pub struct AgenticSystemPublicationConformance;

impl AgenticSystemPublicationConformance {
    pub async fn run(
        publications: &dyn AgenticSystemPublicationPort,
    ) -> Result<Vec<&'static str>, ConformanceFailure> {
        let mut passed = Vec::new();
        Self::an_unpublished_revision_is_absent(publications).await?;
        passed.push("an_unpublished_revision_is_absent");
        Self::publishing_stores_the_design_and_its_digest(publications).await?;
        passed.push("publishing_stores_the_design_and_its_digest");
        Self::republishing_identical_content_is_idempotent(publications).await?;
        passed.push("republishing_identical_content_is_idempotent");
        Self::a_taken_revision_is_never_overwritten(publications).await?;
        passed.push("a_taken_revision_is_never_overwritten");
        Self::the_catalogue_holds_what_was_published(publications).await?;
        passed.push("the_catalogue_holds_what_was_published");
        Ok(passed)
    }

    async fn an_unpublished_revision_is_absent(
        publications: &dyn AgenticSystemPublicationPort,
    ) -> Result<(), ConformanceFailure> {
        const PROPERTY: &str = "an_unpublished_revision_is_absent";
        let id = system_id(PROPERTY)?;

        let found = call(
            PROPERTY,
            publications
                .published(&id, AgenticSystemRevision::INITIAL)
                .await,
        )?;
        if found.is_some() {
            return Err(failure(
                PROPERTY,
                "a revision that was never published came back",
            ));
        }
        Ok(())
    }

    async fn publishing_stores_the_design_and_its_digest(
        publications: &dyn AgenticSystemPublicationPort,
    ) -> Result<(), ConformanceFailure> {
        const PROPERTY: &str = "publishing_stores_the_design_and_its_digest";
        let id = system_id(PROPERTY)?;
        let design = sealed(PROPERTY, &id, "as published")?;
        let digest = design.digest();

        let outcome = call(PROPERTY, publications.publish(design).await)?;
        if !outcome.is_new() {
            return Err(failure(
                PROPERTY,
                format!("a first publication answered `{}`", outcome.as_str()),
            ));
        }
        let stored = call(
            PROPERTY,
            publications
                .published(&id, AgenticSystemRevision::INITIAL)
                .await,
        )?
        .ok_or_else(|| failure(PROPERTY, "a published revision could not be read back"))?;
        if stored.digest() != digest {
            return Err(failure(
                PROPERTY,
                "the stored digest is not the one that was sealed",
            ));
        }
        Ok(())
    }

    async fn republishing_identical_content_is_idempotent(
        publications: &dyn AgenticSystemPublicationPort,
    ) -> Result<(), ConformanceFailure> {
        const PROPERTY: &str = "republishing_identical_content_is_idempotent";
        let id = system_id(PROPERTY)?;
        call(
            PROPERTY,
            publications
                .publish(sealed(PROPERTY, &id, "the same words")?)
                .await,
        )?;

        let again = call(
            PROPERTY,
            publications
                .publish(sealed(PROPERTY, &id, "the same words")?)
                .await,
        )?;
        if again.is_new() || again.is_conflict() {
            return Err(failure(
                PROPERTY,
                format!(
                    "republishing identical content answered `{}` rather than already_published",
                    again.as_str()
                ),
            ));
        }
        Ok(())
    }

    async fn a_taken_revision_is_never_overwritten(
        publications: &dyn AgenticSystemPublicationPort,
    ) -> Result<(), ConformanceFailure> {
        const PROPERTY: &str = "a_taken_revision_is_never_overwritten";
        let id = system_id(PROPERTY)?;
        let first = sealed(PROPERTY, &id, "what the revision says")?;
        let digest = first.digest();
        call(PROPERTY, publications.publish(first).await)?;

        let different = sealed(PROPERTY, &id, "what somebody else wanted it to say")?;
        let offered = different.digest();
        let outcome = call(PROPERTY, publications.publish(different).await)?;
        if !outcome.is_conflict() {
            return Err(failure(
                PROPERTY,
                "different content was accepted under a revision already sealed",
            ));
        }
        let stored = call(
            PROPERTY,
            publications
                .published(&id, AgenticSystemRevision::INITIAL)
                .await,
        )?
        .ok_or_else(|| failure(PROPERTY, "the sealed revision disappeared"))?;
        if stored.digest() != digest || digest == offered {
            return Err(failure(
                PROPERTY,
                "the sealed revision changed under a refused publication",
            ));
        }
        Ok(())
    }

    async fn the_catalogue_holds_what_was_published(
        publications: &dyn AgenticSystemPublicationPort,
    ) -> Result<(), ConformanceFailure> {
        const PROPERTY: &str = "the_catalogue_holds_what_was_published";
        let id = system_id(PROPERTY)?;
        call(
            PROPERTY,
            publications
                .publish(sealed(PROPERTY, &id, "catalogued")?)
                .await,
        )?;

        let catalogue = call(PROPERTY, publications.catalogue().await)?;
        if !catalogue.iter().any(|entry| entry.id() == &id) {
            return Err(failure(
                PROPERTY,
                "a published design is missing from the catalogue",
            ));
        }
        Ok(())
    }
}
