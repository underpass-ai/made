use crate::InProcessProtocolDefinitionSource;
use made_app::usecases::{
    DesignProtocolUseCase, DesignedProtocol, DiffProtocolDefinitionsUseCase,
    GetProtocolDefinitionUseCase, GetPublishedProtocolDefinitionUseCase,
    ListProtocolDefinitionsUseCase, ListPublishedProtocolDefinitionsUseCase,
    MountProtocolDefinitionsOutput, MountProtocolDefinitionsUseCase, ProtocolDefinitionSource,
    ProtocolDesignDocument, PublishProtocolDefinitionUseCase, PublishedProtocolDefinitionPage,
    PublishedProtocolDefinitionQuery, ResolveProtocolDefinitionUseCase,
};
use made_core::entities::{
    ProtocolDefinition, ProtocolInstance, PublicationOutcome, PublishedProtocolDefinition,
};
use made_core::error::DomainError;
use made_core::value_objects::{
    AuthorizationAction, ProtocolDefinitionDiff, ProtocolName, ProtocolVersion,
};
use std::sync::Arc;

use super::EmbeddedMade;

impl EmbeddedMade {
    pub async fn mount_definition(
        &self,
        definition: ProtocolDefinition,
    ) -> Result<MountProtocolDefinitionsOutput, DomainError> {
        self.require_authorized_definition_action(
            AuthorizationAction::MountDefinition,
            definition.name(),
            Some(definition.version()),
        )?;
        self.mount_definitions([definition]).await
    }

    pub async fn mount_definitions(
        &self,
        definitions: impl IntoIterator<Item = ProtocolDefinition>,
    ) -> Result<MountProtocolDefinitionsOutput, DomainError> {
        self.require_authorized_action(AuthorizationAction::MountDefinition)?;
        let definitions = definitions.into_iter().collect::<Vec<_>>();
        for definition in &definitions {
            self.require_authorized_definition_action(
                AuthorizationAction::MountDefinition,
                definition.name(),
                Some(definition.version()),
            )?;
        }
        let source = Arc::new(InProcessProtocolDefinitionSource::new(definitions));
        MountProtocolDefinitionsUseCase::new(source, self.definitions.clone())
            .execute()
            .await
    }

    pub async fn mount_yaml(
        &self,
        raw: &str,
    ) -> Result<MountProtocolDefinitionsOutput, DomainError> {
        self.require_any_authorized_action(&[
            AuthorizationAction::MountDefinition,
            AuthorizationAction::RunProtocol,
            AuthorizationAction::StartProtocol,
        ])?;
        let source = Arc::new(InProcessProtocolDefinitionSource::from_yaml(raw)?);
        for definition in source.definitions() {
            self.require_definition_scope_when_mounting(definition.name(), definition.version())?;
        }
        MountProtocolDefinitionsUseCase::new(source, self.definitions.clone())
            .execute()
            .await
    }

    pub async fn definition(
        &self,
        name: &ProtocolName,
        version: &ProtocolVersion,
    ) -> Result<ProtocolDefinition, DomainError> {
        self.require_authorized_definition_action(
            AuthorizationAction::GetProtocolDefinition,
            name,
            Some(version),
        )?;
        GetProtocolDefinitionUseCase::new(self.definitions.clone())
            .execute(name, version)
            .await
    }

    pub async fn definitions(&self) -> Result<Vec<ProtocolDefinition>, DomainError> {
        self.require_authorized_global_action(AuthorizationAction::ListProtocolDefinitions)?;
        ListProtocolDefinitionsUseCase::new(self.definitions.clone())
            .execute()
            .await
    }

    /// Fix a definition to an immutable version.
    pub async fn publish_definition(
        &self,
        definition: ProtocolDefinition,
    ) -> Result<PublicationOutcome, DomainError> {
        self.require_authorized_definition_action(
            AuthorizationAction::PublishProtocolDefinition,
            definition.name(),
            Some(definition.version()),
        )?;
        PublishProtocolDefinitionUseCase::new(self.publications.clone())
            .execute(definition)
            .await
    }

    /// The published definition under a name and version, if any.
    pub async fn published_definition(
        &self,
        name: &ProtocolName,
        version: &ProtocolVersion,
    ) -> Result<Option<PublishedProtocolDefinition>, DomainError> {
        self.require_authorized_definition_action(
            AuthorizationAction::GetProtocolDefinition,
            name,
            Some(version),
        )?;
        self.publications.published(name, version).await
    }

    /// Every published definition.
    pub async fn published_definitions(
        &self,
    ) -> Result<Vec<PublishedProtocolDefinition>, DomainError> {
        self.require_authorized_global_action(AuthorizationAction::ListProtocolDefinitions)?;
        self.publications.catalogue().await
    }

    /// One page of the published catalogue, in name-then-version order.
    ///
    /// The paged, filtered reading of [`Self::published_definitions`],
    /// and the one both MCP backends serve: a caller that published a
    /// protocol can find it again without the document it sent.
    pub async fn list_definitions(
        &self,
        query: &PublishedProtocolDefinitionQuery,
    ) -> Result<PublishedProtocolDefinitionPage, DomainError> {
        self.require_authorized_global_action(AuthorizationAction::ListProtocolDefinitions)?;
        ListPublishedProtocolDefinitionsUseCase::new(self.publications.clone())
            .execute(query)
            .await
    }

    /// The version published under this name, or `NotFound`.
    ///
    /// [`Self::published_definition`] answers "is there one"; this
    /// answers "give me the one I published", where absence is the
    /// caller's mistake rather than an answer.
    pub async fn get_definition(
        &self,
        name: &ProtocolName,
        version: &ProtocolVersion,
    ) -> Result<PublishedProtocolDefinition, DomainError> {
        self.require_authorized_definition_action(
            AuthorizationAction::GetProtocolDefinition,
            name,
            Some(version),
        )?;
        GetPublishedProtocolDefinitionUseCase::new(self.publications.clone())
            .execute(name, version)
            .await
    }

    /// The definition an instance actually runs, binding included.
    ///
    /// Delegates to the shared use case rather than holding the rule,
    /// so the embedded and deployable distributions cannot drift apart
    /// on what a bound instance means.
    pub async fn definition_for(
        &self,
        instance: &ProtocolInstance,
    ) -> Result<ProtocolDefinition, DomainError> {
        self.require_authorized_definition_action(
            AuthorizationAction::GetProtocolDefinition,
            instance.definition_name(),
            Some(instance.definition_version()),
        )?;
        self.resolve_definition().execute(instance).await
    }

    /// How every verb that advances a session finds what it is running.
    /// A bound session resolves from the catalogue and is checked
    /// against the digest it recorded; an unbound one has only the
    /// repository. Handing this to the use cases is what lets a
    /// published session be advanced at all.
    pub(super) fn resolve_definition(&self) -> Arc<ResolveProtocolDefinitionUseCase> {
        Arc::new(ResolveProtocolDefinitionUseCase::new(
            self.definitions.clone(),
            self.publications.clone(),
        ))
    }

    /// Turn authoring intent into a protocol document.
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
        document: &ProtocolDesignDocument,
    ) -> Result<DesignedProtocol, DomainError> {
        self.require_authorized_global_action(AuthorizationAction::DesignProtocol)?;
        DesignProtocolUseCase::new().execute(document)
    }

    /// Compare two definitions, either side published or supplied.
    pub async fn diff_definitions(
        &self,
        before: ProtocolDefinitionSource,
        after: ProtocolDefinitionSource,
    ) -> Result<ProtocolDefinitionDiff, DomainError> {
        self.require_authorized_global_action(AuthorizationAction::DiffProtocolDefinitions)?;
        DiffProtocolDefinitionsUseCase::new(self.publications.clone())
            .execute(before, after)
            .await
    }
}
