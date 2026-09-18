use std::collections::BTreeMap;

use made_core::value_objects::{
    CeremonyDescription, CeremonyName, CeremonyVersion, DurationMs, InputName, MaxBounces,
    MaxParallel, MaxTransitions, OutputName, StepAttempt, StepTimeout,
};

use super::ceremony_design_final_approval::CeremonyDesignFinalApproval;
use super::ceremony_design_participant::CeremonyDesignParticipant;
use super::ceremony_design_route::CeremonyDesignRoute;
use super::ceremony_design_stage::CeremonyDesignStage;
use super::ceremony_pattern_preset::CeremonyPatternPreset;
use super::CeremonyDesignStageEntry;
use super::CeremonyStagePatternKind;

/// What an author wants, before anything mechanical is decided.
///
/// The document says the meaning — the objective, who sits at the
/// table, what each stage is for — and nothing about states, triggers
/// or guards. Turning it into a ceremony is
/// [`super::DesignCeremonyUseCase`]'s work, which is why every field
/// a caller may leave out is carried as an `Option` and resolved
/// there: an omission means the same thing on every surface because
/// only one place decides what it means.
#[derive(Debug, Clone, PartialEq)]
pub struct CeremonyDesignDocument {
    name: CeremonyName,
    version: Option<CeremonyVersion>,
    objective: CeremonyDescription,
    required_inputs: Vec<InputName>,
    optional_inputs: Vec<InputName>,
    outputs: Vec<OutputName>,
    participants: Vec<CeremonyDesignParticipant>,
    stages: Vec<CeremonyDesignStage>,
    stage_entries: Vec<CeremonyDesignStageEntry>,
    final_approval: Option<CeremonyDesignFinalApproval>,
    step_timeout: Option<StepTimeout>,
    max_attempts: Option<StepAttempt>,
    retry_backoff: Option<DurationMs>,
    pattern: Option<CeremonyPatternPreset>,
    max_parallel: MaxParallel,
    max_transitions: Option<MaxTransitions>,
    max_bounces: Option<MaxBounces>,
    state_patterns: BTreeMap<made_core::value_objects::StepId, CeremonyStagePatternKind>,
    routes: Vec<CeremonyDesignRoute>,
}

impl CeremonyDesignDocument {
    #[must_use]
    pub fn new(
        name: CeremonyName,
        version: Option<CeremonyVersion>,
        objective: CeremonyDescription,
        required_inputs: Vec<InputName>,
        optional_inputs: Vec<InputName>,
        outputs: Vec<OutputName>,
        participants: Vec<CeremonyDesignParticipant>,
        stages: Vec<CeremonyDesignStage>,
        final_approval: Option<CeremonyDesignFinalApproval>,
        step_timeout: Option<StepTimeout>,
        max_attempts: Option<StepAttempt>,
        retry_backoff: Option<DurationMs>,
    ) -> Self {
        let stage_entries = stages
            .iter()
            .cloned()
            .map(CeremonyDesignStageEntry::Leaf)
            .collect();
        Self {
            name,
            version,
            objective,
            required_inputs,
            optional_inputs,
            outputs,
            participants,
            stages,
            stage_entries,
            final_approval,
            step_timeout,
            max_attempts,
            retry_backoff,
            pattern: None,
            max_parallel: MaxParallel::default(),
            max_transitions: None,
            max_bounces: None,
            state_patterns: BTreeMap::new(),
            routes: Vec::new(),
        }
    }

    /// Select a shipped preset instead of supplying explicit stages.
    #[must_use]
    pub const fn with_pattern(mut self, pattern: CeremonyPatternPreset) -> Self {
        self.pattern = Some(pattern);
        self
    }

    #[must_use]
    pub const fn name(&self) -> &CeremonyName {
        &self.name
    }

    #[must_use]
    pub fn version(&self) -> Option<&CeremonyVersion> {
        self.version.as_ref()
    }

    #[must_use]
    pub const fn objective(&self) -> &CeremonyDescription {
        &self.objective
    }

    #[must_use]
    pub fn required_inputs(&self) -> &[InputName] {
        &self.required_inputs
    }

    #[must_use]
    pub fn optional_inputs(&self) -> &[InputName] {
        &self.optional_inputs
    }

    #[must_use]
    pub fn outputs(&self) -> &[OutputName] {
        &self.outputs
    }

    #[must_use]
    pub fn participants(&self) -> &[CeremonyDesignParticipant] {
        &self.participants
    }

    #[must_use]
    pub fn stages(&self) -> &[CeremonyDesignStage] {
        &self.stages
    }

    #[must_use]
    pub fn stage_entries(&self) -> &[CeremonyDesignStageEntry] {
        &self.stage_entries
    }

    #[must_use]
    pub fn with_stage_entries(mut self, entries: Vec<CeremonyDesignStageEntry>) -> Self {
        self.stages = entries
            .iter()
            .filter_map(|entry| match entry {
                CeremonyDesignStageEntry::Leaf(stage) => Some(stage.clone()),
                CeremonyDesignStageEntry::Group(_) | CeremonyDesignStageEntry::Pattern(_) => None,
            })
            .collect();
        self.stage_entries = entries;
        self
    }

    #[must_use]
    pub fn with_max_parallel(mut self, max_parallel: MaxParallel) -> Self {
        self.max_parallel = max_parallel;
        self
    }

    #[must_use]
    pub fn max_parallel(&self) -> MaxParallel {
        self.max_parallel
    }

    #[must_use]
    pub const fn with_max_transitions(mut self, max_transitions: MaxTransitions) -> Self {
        self.max_transitions = Some(max_transitions);
        self
    }

    #[must_use]
    pub const fn max_transitions(&self) -> Option<MaxTransitions> {
        self.max_transitions
    }

    #[must_use]
    pub const fn with_max_bounces(mut self, max_bounces: MaxBounces) -> Self {
        self.max_bounces = Some(max_bounces);
        self
    }

    #[must_use]
    pub const fn max_bounces(&self) -> Option<MaxBounces> {
        self.max_bounces
    }

    #[must_use]
    pub fn final_approval(&self) -> Option<&CeremonyDesignFinalApproval> {
        self.final_approval.as_ref()
    }

    #[must_use]
    pub const fn step_timeout(&self) -> Option<StepTimeout> {
        self.step_timeout
    }

    #[must_use]
    pub const fn max_attempts(&self) -> Option<StepAttempt> {
        self.max_attempts
    }

    #[must_use]
    pub const fn retry_backoff(&self) -> Option<DurationMs> {
        self.retry_backoff
    }

    #[must_use]
    pub const fn pattern(&self) -> Option<CeremonyPatternPreset> {
        self.pattern
    }

    #[must_use]
    pub(crate) fn state_pattern(
        &self,
        entry_id: &made_core::value_objects::StepId,
    ) -> Option<CeremonyStagePatternKind> {
        self.state_patterns.get(entry_id).copied()
    }

    pub(crate) fn materialized_with_entries(
        &self,
        entries: Vec<CeremonyDesignStageEntry>,
        state_patterns: BTreeMap<made_core::value_objects::StepId, CeremonyStagePatternKind>,
        routes: Vec<CeremonyDesignRoute>,
    ) -> Self {
        let mut materialized = self.clone().with_stage_entries(entries);
        materialized.state_patterns = state_patterns;
        materialized.routes = routes;
        materialized
    }

    pub(crate) fn routes(&self) -> &[CeremonyDesignRoute] {
        &self.routes
    }

    pub(crate) fn materialized_with_stages(&self, stages: Vec<CeremonyDesignStage>) -> Self {
        let mut materialized = self.clone();
        materialized.stages = stages;
        materialized.stage_entries = materialized
            .stages
            .iter()
            .cloned()
            .map(CeremonyDesignStageEntry::Leaf)
            .collect();
        materialized.pattern = None;
        materialized.state_patterns.clear();
        materialized.routes.clear();
        materialized
    }
}
