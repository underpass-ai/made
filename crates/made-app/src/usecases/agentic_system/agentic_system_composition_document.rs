use std::collections::BTreeMap;

use made_core::error::DomainError;
use made_core::value_objects::{
    InputName, ParticipantId, ProtocolActivation, ProtocolComposition, ProtocolDefinitionDigest,
    ProtocolOutputRef, RoleId, SystemProtocolId, SystemPurpose,
};
use serde::Deserialize;

use super::AgenticSystemPinDocument;

/// One protocol an author composes into their system.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AgenticSystemCompositionDocument {
    id: SystemProtocolId,
    pin: AgenticSystemPinDocument,
    purpose: SystemPurpose,
    #[serde(default)]
    depends_on: Vec<SystemProtocolId>,
    #[serde(default = "manual")]
    activation: ProtocolActivation,
    #[serde(default)]
    role_bindings: BTreeMap<RoleId, ParticipantId>,
    #[serde(default)]
    inputs_from: BTreeMap<InputName, ProtocolOutputRef>,
}

fn manual() -> ProtocolActivation {
    ProtocolActivation::Manual
}

impl AgenticSystemCompositionDocument {
    #[must_use]
    pub const fn id(&self) -> &SystemProtocolId {
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
        resolved: Option<ProtocolDefinitionDigest>,
    ) -> Result<ProtocolComposition, DomainError> {
        Ok(ProtocolComposition::new(
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
