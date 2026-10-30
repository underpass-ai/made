//! Conformance suite for [`ProtocolDefinitionPublicationPort`].
//!
//! The property under test is immutability, and it is easy to fail by
//! accident: a store built on the ordinary repository's `save` accepts
//! everything and overwrites silently, which looks like it works until
//! an instance is bound to a version whose content has since changed
//! underneath it.

use crate::entities::{ProtocolCatalogueEntry, ProtocolDefinition, PublishedProtocolDefinition};
use crate::error::DomainError;
use crate::ports::ProtocolDefinitionPublicationPort;
use crate::value_objects::{
    ProtocolName, ProtocolState, ProtocolTransition, ProtocolVersion, StateId, TransitionTrigger,
};

use super::ConformanceFailure;

/// Every property a [`ProtocolDefinitionPublicationPort`]
/// implementation must satisfy.
#[derive(Debug)]
pub struct ProtocolDefinitionPublicationConformance;

impl ProtocolDefinitionPublicationConformance {
    pub async fn run(
        publications: &dyn ProtocolDefinitionPublicationPort,
    ) -> Result<Vec<&'static str>, ConformanceFailure> {
        let mut passed = Vec::new();
        Self::an_unpublished_version_is_absent(publications).await?;
        passed.push("an_unpublished_version_is_absent");
        Self::publishing_stores_the_definition_and_its_digest(publications).await?;
        passed.push("publishing_stores_the_definition_and_its_digest");
        Self::republishing_identical_content_is_idempotent(publications).await?;
        passed.push("republishing_identical_content_is_idempotent");
        Self::a_taken_version_is_never_overwritten(publications).await?;
        passed.push("a_taken_version_is_never_overwritten");
        Self::versions_of_one_protocol_coexist(publications).await?;
        passed.push("versions_of_one_protocol_coexist");
        Self::the_listing_reads_back_every_publication(publications).await?;
        passed.push("the_listing_reads_back_every_publication");
        Ok(passed)
    }

    async fn an_unpublished_version_is_absent(
        publications: &dyn ProtocolDefinitionPublicationPort,
    ) -> Result<(), ConformanceFailure> {
        const PROPERTY: &str = "an_unpublished_version_is_absent";
        let name = protocol_name(PROPERTY, "absent")?;

        let found = call(
            PROPERTY,
            publications.published(&name, &ProtocolVersion::v1()).await,
        )?;
        if found.is_some() {
            return Err(failure(
                PROPERTY,
                "a version that was never published came back",
            ));
        }
        Ok(())
    }

    async fn publishing_stores_the_definition_and_its_digest(
        publications: &dyn ProtocolDefinitionPublicationPort,
    ) -> Result<(), ConformanceFailure> {
        const PROPERTY: &str = "publishing_stores_the_definition_and_its_digest";
        let name = protocol_name(PROPERTY, "stored")?;
        let sealed = sealed(PROPERTY, &name, "done")?;
        let digest = sealed.digest();

        let outcome = call(PROPERTY, publications.publish(sealed).await)?;
        if !outcome.is_new() {
            return Err(failure(
                PROPERTY,
                "a first publication was not reported as new",
            ));
        }

        let stored = call(
            PROPERTY,
            publications.published(&name, &ProtocolVersion::v1()).await,
        )?
        .ok_or_else(|| failure(PROPERTY, "a published version is not readable"))?;
        if stored.digest() != digest {
            return Err(failure(
                PROPERTY,
                "the stored digest is not the one that was published",
            ));
        }
        Ok(())
    }

    /// A caller that loses the response must be able to retry. If
    /// republishing identical content failed, it would have no correct
    /// next move.
    async fn republishing_identical_content_is_idempotent(
        publications: &dyn ProtocolDefinitionPublicationPort,
    ) -> Result<(), ConformanceFailure> {
        const PROPERTY: &str = "republishing_identical_content_is_idempotent";
        let name = protocol_name(PROPERTY, "retried")?;

        call(
            PROPERTY,
            publications.publish(sealed(PROPERTY, &name, "done")?).await,
        )?;
        let again = call(
            PROPERTY,
            publications.publish(sealed(PROPERTY, &name, "done")?).await,
        )?;

        if again.is_conflict() {
            return Err(failure(
                PROPERTY,
                "republishing identical content was reported as a conflict",
            ));
        }
        if again.is_new() {
            return Err(failure(
                PROPERTY,
                "republishing identical content was reported as a first publication",
            ));
        }
        Ok(())
    }

    /// The property publication exists for.
    async fn a_taken_version_is_never_overwritten(
        publications: &dyn ProtocolDefinitionPublicationPort,
    ) -> Result<(), ConformanceFailure> {
        const PROPERTY: &str = "a_taken_version_is_never_overwritten";
        let name = protocol_name(PROPERTY, "immutable")?;
        let original = sealed(PROPERTY, &name, "done")?;
        let original_digest = original.digest();

        call(PROPERTY, publications.publish(original).await)?;
        let outcome = call(
            PROPERTY,
            publications
                .publish(sealed(PROPERTY, &name, "finished")?)
                .await,
        )?;

        if !outcome.is_conflict() {
            return Err(failure(
                PROPERTY,
                "different content was accepted under a version that was already taken",
            ));
        }

        let stored = call(
            PROPERTY,
            publications.published(&name, &ProtocolVersion::v1()).await,
        )?
        .ok_or_else(|| failure(PROPERTY, "the published version disappeared"))?;
        if stored.digest() != original_digest {
            return Err(failure(
                PROPERTY,
                "a rejected publication still replaced the stored definition",
            ));
        }
        Ok(())
    }

    async fn versions_of_one_protocol_coexist(
        publications: &dyn ProtocolDefinitionPublicationPort,
    ) -> Result<(), ConformanceFailure> {
        const PROPERTY: &str = "versions_of_one_protocol_coexist";
        let name = protocol_name(PROPERTY, "versioned")?;
        let second = ProtocolVersion::new("2.0")
            .map_err(|error| failure(PROPERTY, format!("invalid version: {error}")))?;

        call(
            PROPERTY,
            publications.publish(sealed(PROPERTY, &name, "done")?).await,
        )?;
        let outcome = call(
            PROPERTY,
            publications
                .publish(sealed_at(PROPERTY, &name, second.clone(), "finished")?)
                .await,
        )?;
        if !outcome.is_new() {
            return Err(failure(
                PROPERTY,
                "a new version of an existing protocol was refused",
            ));
        }

        let first = call(
            PROPERTY,
            publications.published(&name, &ProtocolVersion::v1()).await,
        )?;
        if first.is_none() {
            return Err(failure(
                PROPERTY,
                "publishing a new version removed the previous one",
            ));
        }
        Ok(())
    }

    /// What a listing reads is the catalogue: every publication, each
    /// readable, none invented. Run last, over what the properties
    /// above published.
    async fn the_listing_reads_back_every_publication(
        publications: &dyn ProtocolDefinitionPublicationPort,
    ) -> Result<(), ConformanceFailure> {
        const PROPERTY: &str = "the_listing_reads_back_every_publication";
        let mut catalogue = call(PROPERTY, publications.catalogue().await)?;
        let mut listed = call(PROPERTY, publications.catalogue_entries().await)?
            .into_iter()
            .map(|entry| match entry {
                ProtocolCatalogueEntry::Readable(published) => Ok(*published),
                ProtocolCatalogueEntry::Unreadable(unreadable) => Err(failure(
                    PROPERTY,
                    format!(
                        "an intact publication was listed as unreadable: {}",
                        unreadable.defect()
                    ),
                )),
            })
            .collect::<Result<Vec<_>, _>>()?;
        if catalogue.is_empty() {
            return Err(failure(PROPERTY, "nothing published came back at all"));
        }
        let identity = |published: &PublishedProtocolDefinition| {
            (published.name().clone(), published.version().clone())
        };
        catalogue.sort_by_key(identity);
        listed.sort_by_key(identity);
        if catalogue != listed {
            return Err(failure(
                PROPERTY,
                "the listing and the catalogue disagree about what is published",
            ));
        }
        Ok(())
    }
}

fn call<T>(
    property: &'static str,
    outcome: Result<T, DomainError>,
) -> Result<T, ConformanceFailure> {
    outcome.map_err(|error| failure(property, format!("the adapter returned an error: {error}")))
}

fn failure(property: &'static str, detail: impl Into<String>) -> ConformanceFailure {
    ConformanceFailure::new(property, detail)
}

fn protocol_name(property: &'static str, suffix: &str) -> Result<ProtocolName, ConformanceFailure> {
    ProtocolName::new(format!("conformance_{property}_{suffix}"))
        .map_err(|error| failure(property, format!("invalid protocol name: {error}")))
}

fn sealed(
    property: &'static str,
    name: &ProtocolName,
    terminal: &str,
) -> Result<PublishedProtocolDefinition, ConformanceFailure> {
    sealed_at(property, name, ProtocolVersion::v1(), terminal)
}

/// `terminal` names the terminal state, which is what makes two
/// otherwise identical definitions differ materially.
fn sealed_at(
    property: &'static str,
    name: &ProtocolName,
    version: ProtocolVersion,
    terminal: &str,
) -> Result<PublishedProtocolDefinition, ConformanceFailure> {
    let build = || -> Result<PublishedProtocolDefinition, DomainError> {
        let definition = ProtocolDefinition::new(
            name.clone(),
            version,
            None,
            Vec::new(),
            Vec::new(),
            vec![
                ProtocolState::initial(StateId::new("OPEN")?),
                ProtocolState::terminal(StateId::new(terminal)?),
            ],
            vec![ProtocolTransition::new(
                StateId::new("OPEN")?,
                StateId::new(terminal)?,
                TransitionTrigger::new("finish")?,
                Vec::new(),
            )?],
            Vec::new(),
            Vec::new(),
            Vec::new(),
        )?;
        PublishedProtocolDefinition::seal(definition)
    };
    build().map_err(|error| {
        failure(
            property,
            format!("the suite built an invalid definition: {error}"),
        )
    })
}
