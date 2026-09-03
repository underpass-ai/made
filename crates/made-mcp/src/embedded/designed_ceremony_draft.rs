use made_core::entities::CeremonyDefinitionDraft;

/// The result of turning structured authoring intent into an analysable draft.
#[derive(Debug)]
pub(super) struct DesignedCeremonyDraft {
    pub(super) definition_yaml: String,
    pub(super) draft: CeremonyDefinitionDraft,
    pub(super) stage_count: usize,
    pub(super) participant_count: usize,
    pub(super) final_approval_required: bool,
}

impl DesignedCeremonyDraft {
    pub(super) fn definition_yaml(&self) -> &str {
        &self.definition_yaml
    }

    pub(super) fn draft(&self) -> &CeremonyDefinitionDraft {
        &self.draft
    }

    pub(super) const fn stage_count(&self) -> usize {
        self.stage_count
    }

    pub(super) const fn participant_count(&self) -> usize {
        self.participant_count
    }

    pub(super) const fn final_approval_required(&self) -> bool {
        self.final_approval_required
    }
}
