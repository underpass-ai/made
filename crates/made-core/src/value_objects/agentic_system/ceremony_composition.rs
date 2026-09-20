use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

use crate::value_objects::{DefinitionPin, InputName, RoleId};

use super::{
    CeremonyActivation, CeremonyOutputRef, ParticipantId, SystemCeremonyId, SystemPurpose,
};

/// One ceremony this system composes, referenced and never owned.
///
/// The pin is the whole point: name and version say which definition,
/// the digest says which bytes, and a republished version is therefore
/// a different pin rather than a silent change of what the system does.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CeremonyComposition {
    id: SystemCeremonyId,
    pin: DefinitionPin,
    purpose: SystemPurpose,
    #[serde(default, skip_serializing_if = "BTreeSet::is_empty")]
    depends_on: BTreeSet<SystemCeremonyId>,
    activation: CeremonyActivation,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    role_bindings: BTreeMap<RoleId, ParticipantId>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    inputs_from: BTreeMap<InputName, CeremonyOutputRef>,
}

impl CeremonyComposition {
    #[must_use]
    pub fn new(
        id: SystemCeremonyId,
        pin: DefinitionPin,
        purpose: SystemPurpose,
        depends_on: impl IntoIterator<Item = SystemCeremonyId>,
        activation: CeremonyActivation,
        role_bindings: impl IntoIterator<Item = (RoleId, ParticipantId)>,
        inputs_from: impl IntoIterator<Item = (InputName, CeremonyOutputRef)>,
    ) -> Self {
        Self {
            id,
            pin,
            purpose,
            depends_on: depends_on.into_iter().collect(),
            activation,
            role_bindings: role_bindings.into_iter().collect(),
            inputs_from: inputs_from.into_iter().collect(),
        }
    }

    #[must_use]
    pub const fn id(&self) -> &SystemCeremonyId {
        &self.id
    }

    #[must_use]
    pub const fn pin(&self) -> &DefinitionPin {
        &self.pin
    }

    #[must_use]
    pub const fn purpose(&self) -> &SystemPurpose {
        &self.purpose
    }

    #[must_use]
    pub const fn depends_on(&self) -> &BTreeSet<SystemCeremonyId> {
        &self.depends_on
    }

    #[must_use]
    pub const fn activation(&self) -> &CeremonyActivation {
        &self.activation
    }

    /// Which logical participant sits in each seat the definition
    /// declares.
    #[must_use]
    pub const fn role_bindings(&self) -> &BTreeMap<RoleId, ParticipantId> {
        &self.role_bindings
    }

    #[must_use]
    pub const fn inputs_from(&self) -> &BTreeMap<InputName, CeremonyOutputRef> {
        &self.inputs_from
    }

    /// Every composition this one waits for: its declared dependencies
    /// plus the one its loop runs back to.
    #[must_use]
    pub fn predecessors(&self) -> BTreeSet<&SystemCeremonyId> {
        let mut every: BTreeSet<&SystemCeremonyId> = self.depends_on.iter().collect();
        every.extend(self.activation.loops_after());
        every
    }
}
