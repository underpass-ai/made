use made_core::entities::CeremonyDefinitionDraft;
use made_core::value_objects::GuardCondition;

/// The definition designed from authoring intent, ready for analysis and encoding.
#[derive(Debug, Clone, PartialEq)]
pub struct DesignedCeremony {
    definition: CeremonyDefinitionDraft,
}

impl DesignedCeremony {
    #[must_use]
    pub(crate) const fn new(definition: CeremonyDefinitionDraft) -> Self {
        Self { definition }
    }

    #[must_use]
    pub const fn definition(&self) -> &CeremonyDefinitionDraft {
        &self.definition
    }

    #[must_use]
    pub const fn topology(&self) -> &'static str {
        "linear"
    }

    #[must_use]
    pub fn stage_count(&self) -> usize {
        self.definition.steps().len()
    }

    #[must_use]
    pub fn participant_count(&self) -> usize {
        self.definition.roles().len()
    }

    #[must_use]
    pub fn final_approval_required(&self) -> bool {
        self.definition
            .guards()
            .iter()
            .any(|guard| matches!(guard.condition(), GuardCondition::HumanApproval))
    }
}
