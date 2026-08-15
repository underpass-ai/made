use made_core::entities::{CeremonyDefinition, CeremonyInstance, PublishedCeremonyDefinition};
use made_core::error::DomainError;
use made_core::value_objects::{CeremonyDefinitionDigest, CeremonyDefinitionDigestMigration};

/// One verified mapping from the pre-rename digest scheme to MADE's scheme.
#[derive(Debug, Clone)]
pub(super) struct LegacyDefinitionBinding {
    migration: CeremonyDefinitionDigestMigration,
    published: PublishedCeremonyDefinition,
    publication_requires_migration: bool,
}

impl LegacyDefinitionBinding {
    pub(super) fn verify(
        definition: CeremonyDefinition,
        stored_digest: CeremonyDefinitionDigest,
    ) -> Result<Self, DomainError> {
        let migration = definition.choreographer_v1_digest_migration()?;
        let published = PublishedCeremonyDefinition::seal(definition)?;
        if stored_digest != migration.source() && stored_digest != migration.destination() {
            return Err(DomainError::InvariantViolated {
                reason:
                    "a legacy publication matches neither the Choreographer nor MADE digest scheme",
            });
        }

        Ok(Self {
            publication_requires_migration: stored_digest == migration.source(),
            migration,
            published,
        })
    }

    pub(super) fn matches(&self, instance: &CeremonyInstance) -> bool {
        instance.definition_name() == self.migration.definition_name()
            && instance.definition_version() == self.migration.definition_version()
    }

    pub(super) fn migrate_instance(
        &self,
        instance: &mut CeremonyInstance,
    ) -> Result<bool, DomainError> {
        instance.migrate_definition_binding(&self.migration)
    }

    pub(super) fn published(&self) -> &PublishedCeremonyDefinition {
        &self.published
    }

    pub(super) fn publication_requires_migration(&self) -> bool {
        self.publication_requires_migration
    }

    #[cfg(test)]
    pub(super) fn legacy_digest(&self) -> CeremonyDefinitionDigest {
        self.migration.source()
    }
}

#[cfg(test)]
mod tests {
    use made_core::entities::CeremonyDefinition;
    use made_core::value_objects::{CeremonyName, CeremonyState, CeremonyVersion, StateId};

    use super::*;

    fn definition() -> CeremonyDefinition {
        CeremonyDefinition::new(
            CeremonyName::new("digest_migration").unwrap(),
            CeremonyVersion::v1(),
            None,
            [],
            [],
            [CeremonyState::initial(StateId::new("OPEN").unwrap())],
            [],
            [],
            [],
            [],
        )
        .unwrap()
    }

    #[test]
    fn the_legacy_domain_separator_is_not_the_made_identity() {
        let definition = definition();
        let migration = definition.choreographer_v1_digest_migration().unwrap();

        assert_ne!(migration.source(), migration.destination());
    }

    #[test]
    fn unrelated_digests_are_rejected() {
        let error = LegacyDefinitionBinding::verify(
            definition(),
            CeremonyDefinitionDigest::from_bytes([0x5a; 32]),
        )
        .unwrap_err();

        assert!(matches!(error, DomainError::InvariantViolated { .. }));
    }
}
