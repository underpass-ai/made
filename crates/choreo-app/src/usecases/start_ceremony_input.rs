use choreo_core::value_objects::{CeremonyContext, CeremonyId, CeremonyName, CeremonyVersion};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StartCeremonyInput {
    pub(crate) id: CeremonyId,
    pub(crate) definition_name: CeremonyName,
    pub(crate) definition_version: CeremonyVersion,
    pub(crate) context: CeremonyContext,
}

impl StartCeremonyInput {
    #[must_use]
    pub fn new(
        id: CeremonyId,
        definition_name: CeremonyName,
        definition_version: CeremonyVersion,
        context: CeremonyContext,
    ) -> Self {
        Self {
            id,
            definition_name,
            definition_version,
            context,
        }
    }
}
