/// What a design answers with: the document an author can publish, and
/// what the design did with their intent.
///
/// The counts are not derivable from the YAML without reading it the
/// way the designer read the intent, and the topology is the designer's
/// own word for what it built. Both arms read them from here, so the
/// two cannot describe the same design differently.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DesignedCeremony {
    definition_yaml: String,
    stage_count: usize,
    participant_count: usize,
    final_approval_required: bool,
}

impl DesignedCeremony {
    /// The only shape this designer builds: every stage follows the
    /// one before it, and completion is the end of the line.
    const TOPOLOGY: &'static str = "linear";

    #[must_use]
    pub fn new(
        definition_yaml: impl Into<String>,
        stage_count: usize,
        participant_count: usize,
        final_approval_required: bool,
    ) -> Self {
        Self {
            definition_yaml: definition_yaml.into(),
            stage_count,
            participant_count,
            final_approval_required,
        }
    }

    #[must_use]
    pub fn definition_yaml(&self) -> &str {
        &self.definition_yaml
    }

    #[must_use]
    pub const fn topology(&self) -> &'static str {
        Self::TOPOLOGY
    }

    #[must_use]
    pub const fn stage_count(&self) -> usize {
        self.stage_count
    }

    #[must_use]
    pub const fn participant_count(&self) -> usize {
        self.participant_count
    }

    #[must_use]
    pub const fn final_approval_required(&self) -> bool {
        self.final_approval_required
    }
}
