use made_core::entities::CeremonyDefinitionDraft;
use made_core::value_objects::{GuardCondition, StepTimeout};

/// The definition designed from authoring intent, ready for analysis and encoding.
#[derive(Debug, Clone, PartialEq)]
pub struct DesignedCeremony {
    definition: CeremonyDefinitionDraft,
    /// The ceremony-wide step timeout the designer resolved. Every
    /// step without a timeout of its own carries exactly this value;
    /// an encoder needs it to tell a stage's own timeout from the
    /// default without guessing from whichever step comes first.
    default_step_timeout: StepTimeout,
}

impl DesignedCeremony {
    #[must_use]
    pub(crate) const fn new(
        definition: CeremonyDefinitionDraft,
        default_step_timeout: StepTimeout,
    ) -> Self {
        Self {
            definition,
            default_step_timeout,
        }
    }

    #[must_use]
    pub const fn default_step_timeout(&self) -> StepTimeout {
        self.default_step_timeout
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
