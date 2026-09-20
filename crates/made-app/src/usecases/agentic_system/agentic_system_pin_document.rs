use made_core::error::DomainError;
use made_core::value_objects::{
    CeremonyDefinitionDigest, CeremonyName, CeremonyVersion, DefinitionPin,
};
use serde::Deserialize;

/// Which published definition a composition means.
///
/// The digest is optional in the document and never optional in the
/// design. An author writing "compose review 1.0" has not yet said
/// which bytes they meant, so the design use case fills it in from
/// what is published and the design records what they agreed to. An
/// author who does write a digest is stating it, and it is kept
/// exactly as written — a stale one has to survive as far as
/// validation, which is the only place equipped to explain it.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AgenticSystemPinDocument {
    name: CeremonyName,
    version: CeremonyVersion,
    #[serde(default)]
    digest: Option<String>,
}

impl AgenticSystemPinDocument {
    #[must_use]
    pub const fn name(&self) -> &CeremonyName {
        &self.name
    }

    #[must_use]
    pub const fn version(&self) -> &CeremonyVersion {
        &self.version
    }

    /// Which bytes the author said they meant, if they said.
    pub fn stated_digest(&self) -> Result<Option<CeremonyDefinitionDigest>, DomainError> {
        self.digest
            .as_deref()
            .map(CeremonyDefinitionDigest::parse_hex)
            .transpose()
    }

    /// The pin, with `resolved` used only where the author stated
    /// nothing.
    pub fn to_pin(
        &self,
        resolved: Option<CeremonyDefinitionDigest>,
    ) -> Result<DefinitionPin, DomainError> {
        let digest = match self.stated_digest()? {
            Some(stated) => stated,
            None => resolved.ok_or(DomainError::NotFound {
                what: "published_ceremony_definition",
            })?,
        };
        Ok(DefinitionPin::new(
            self.name.clone(),
            self.version.clone(),
            digest,
        ))
    }
}
