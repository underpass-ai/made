use crate::InProcessCeremonyDefinitionSource;
use made_app::usecases::{
    CeremonyDefinitionSource, CeremonyDesignDocument, DesignCeremonyUseCase, DesignedCeremony,
    DiffCeremonyDefinitionsUseCase, GetCeremonyDefinitionUseCase, ListCeremonyDefinitionsUseCase,
    MountCeremonyDefinitionsOutput, MountCeremonyDefinitionsUseCase,
    PublishCeremonyDefinitionUseCase, ResolveCeremonyDefinitionUseCase,
};
use made_core::entities::{
    CeremonyDefinition, CeremonyInstance, PublicationOutcome, PublishedCeremonyDefinition,
};
use made_core::error::DomainError;
use made_core::value_objects::{CeremonyDefinitionDiff, CeremonyName, CeremonyVersion};
use std::sync::Arc;

use super::EmbeddedMade;

impl EmbeddedMade {
    pub async fn mount_definition(
        &self,
        definition: CeremonyDefinition,
    ) -> Result<MountCeremonyDefinitionsOutput, DomainError> {
        self.mount_definitions([definition]).await
    }

    pub async fn mount_definitions(
        &self,
        definitions: impl IntoIterator<Item = CeremonyDefinition>,
    ) -> Result<MountCeremonyDefinitionsOutput, DomainError> {
        let source = Arc::new(InProcessCeremonyDefinitionSource::new(definitions));
        MountCeremonyDefinitionsUseCase::new(source, self.definitions.clone())
            .execute()
            .await
    }

    pub async fn mount_yaml(
        &self,
        raw: &str,
    ) -> Result<MountCeremonyDefinitionsOutput, DomainError> {
        let source = Arc::new(InProcessCeremonyDefinitionSource::from_yaml(raw)?);
        MountCeremonyDefinitionsUseCase::new(source, self.definitions.clone())
            .execute()
            .await
    }

    pub async fn definition(
        &self,
        name: &CeremonyName,
        version: &CeremonyVersion,
    ) -> Result<CeremonyDefinition, DomainError> {
        GetCeremonyDefinitionUseCase::new(self.definitions.clone())
            .execute(name, version)
            .await
    }

    pub async fn definitions(&self) -> Result<Vec<CeremonyDefinition>, DomainError> {
        ListCeremonyDefinitionsUseCase::new(self.definitions.clone())
            .execute()
            .await
    }

    /// Fix a definition to an immutable version.
    pub async fn publish_definition(
        &self,
        definition: CeremonyDefinition,
    ) -> Result<PublicationOutcome, DomainError> {
        PublishCeremonyDefinitionUseCase::new(self.publications.clone())
            .execute(definition)
            .await
    }

    /// The published definition under a name and version, if any.
    pub async fn published_definition(
        &self,
        name: &CeremonyName,
        version: &CeremonyVersion,
    ) -> Result<Option<PublishedCeremonyDefinition>, DomainError> {
        self.publications.published(name, version).await
    }

    /// Every published definition.
    pub async fn published_definitions(
        &self,
    ) -> Result<Vec<PublishedCeremonyDefinition>, DomainError> {
        self.publications.catalogue().await
    }

    /// The definition an instance actually runs, binding included.
    ///
    /// Delegates to the shared use case rather than holding the rule,
    /// so the embedded and deployable distributions cannot drift apart
    /// on what a bound instance means.
    pub async fn definition_for(
        &self,
        instance: &CeremonyInstance,
    ) -> Result<CeremonyDefinition, DomainError> {
        self.resolve_definition().execute(instance).await
    }

    /// How every verb that advances a session finds what it is running.
    /// A bound session resolves from the catalogue and is checked
    /// against the digest it recorded; an unbound one has only the
    /// repository. Handing this to the use cases is what lets a
    /// published session be advanced at all.
    pub(super) fn resolve_definition(&self) -> Arc<ResolveCeremonyDefinitionUseCase> {
        Arc::new(ResolveCeremonyDefinitionUseCase::new(
            self.definitions.clone(),
            self.publications.clone(),
        ))
    }

    /// Turn authoring intent into a ceremony document.
    ///
    /// Touches nothing, like validating and explaining a draft: it
    /// reads no store, writes no definition and starts no session.
    /// What it answers with is the document an author can then put
    /// through those three.
    // A method rather than an associated function: a host asks the
    // engine it holds, and designing is one of the things it asks.
    // That it needs nothing from the engine today is a fact about
    // designing, not about where the question belongs.
    #[allow(clippy::unused_self)]
    pub fn design(
        &self,
        document: &CeremonyDesignDocument,
    ) -> Result<DesignedCeremony, DomainError> {
        DesignCeremonyUseCase::new().execute(document)
    }

    /// Compare two definitions, either side published or supplied.
    pub async fn diff_definitions(
        &self,
        before: CeremonyDefinitionSource,
        after: CeremonyDefinitionSource,
    ) -> Result<CeremonyDefinitionDiff, DomainError> {
        DiffCeremonyDefinitionsUseCase::new(self.publications.clone())
            .execute(before, after)
            .await
    }
}
