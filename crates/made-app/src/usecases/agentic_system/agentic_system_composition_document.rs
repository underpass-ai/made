use std::collections::BTreeMap;

use made_core::error::DomainError;
use made_core::value_objects::{
    CeremonyActivation, CeremonyComposition, CeremonyDefinitionDigest, CeremonyOutputRef,
    InputName, ParticipantId, RoleId, SystemCeremonyId, SystemPurpose,
};
use serde::Deserialize;

use super::AgenticSystemPinDocument;

/// One ceremony an author composes into their system.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AgenticSystemCompositionDocument {
    id: SystemCeremonyId,
    pin: AgenticSystemPinDocument,
    purpose: SystemPurpose,
    #[serde(default)]
    depends_on: Vec<SystemCeremonyId>,
    #[serde(default = "manual")]
    activation: CeremonyActivation,
    #[serde(default)]
    role_bindings: BTreeMap<RoleId, ParticipantId>,
    #[serde(default)]
    inputs_from: BTreeMap<InputName, CeremonyOutputRef>,
}

fn manual() -> CeremonyActivation {
    CeremonyActivation::Manual
}

impl AgenticSystemCompositionDocument {
    #[must_use]
    pub const fn id(&self) -> &SystemCeremonyId {
        &self.id
    }

    #[must_use]
    pub const fn pin(&self) -> &AgenticSystemPinDocument {
        &self.pin
    }

    /// The composition this document describes, with `resolved` used
    /// only where the author stated no digest.
    pub fn to_composition(
        &self,
        resolved: Option<CeremonyDefinitionDigest>,
    ) -> Result<CeremonyComposition, DomainError> {
        Ok(CeremonyComposition::new(
            self.id.clone(),
            self.pin.to_pin(resolved)?,
            self.purpose.clone(),
            self.depends_on.iter().cloned(),
            self.activation.clone(),
            self.role_bindings.clone(),
            self.inputs_from.clone(),
        ))
    }
}
