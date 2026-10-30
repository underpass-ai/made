use std::collections::BTreeMap;

use made_core::value_objects::{
    DurationMs, InputName, MaxBounces, MaxParallel, MaxTransitions, OutputName,
    ProtocolDescription, ProtocolName, ProtocolVersion, StepAttempt, StepTimeout,
};

use super::protocol_design_final_approval::ProtocolDesignFinalApproval;
use super::protocol_design_participant::ProtocolDesignParticipant;
use super::protocol_design_route::ProtocolDesignRoute;
use super::protocol_design_stage::ProtocolDesignStage;
use super::protocol_pattern_preset::ProtocolPatternPreset;
use super::ProtocolDesignStageEntry;
use super::ProtocolStagePatternKind;

/// What an author wants, before anything mechanical is decided.
///
/// The document says the meaning — the objective, who sits at the
/// table, what each stage is for — and nothing about states, triggers
/// or guards. Turning it into a protocol is
/// [`super::DesignProtocolUseCase`]'s work, which is why every field
/// a caller may leave out is carried as an `Option` and resolved
/// there: an omission means the same thing on every surface because
/// only one place decides what it means.
#[derive(Debug, Clone, PartialEq)]
pub struct ProtocolDesignDocument {
    name: ProtocolName,
    version: Option<ProtocolVersion>,
    objective: ProtocolDescription,
    required_inputs: Vec<InputName>,
    optional_inputs: Vec<InputName>,
    outputs: Vec<OutputName>,
    participants: Vec<ProtocolDesignParticipant>,
    stages: Vec<ProtocolDesignStage>,
    stage_entries: Vec<ProtocolDesignStageEntry>,
    final_approval: Option<ProtocolDesignFinalApproval>,
    step_timeout: Option<StepTimeout>,
    max_attempts: Option<StepAttempt>,
    retry_backoff: Option<DurationMs>,
    pattern: Option<ProtocolPatternPreset>,
    max_parallel: MaxParallel,
    max_transitions: Option<MaxTransitions>,
    max_bounces: Option<MaxBounces>,
    state_patterns: BTreeMap<made_core::value_objects::StepId, ProtocolStagePatternKind>,
    routes: Vec<ProtocolDesignRoute>,
}

impl ProtocolDesignDocument {
    #[must_use]
    pub fn new(
        name: ProtocolName,
        version: Option<ProtocolVersion>,
        objective: ProtocolDescription,
        required_inputs: Vec<InputName>,
        optional_inputs: Vec<InputName>,
        outputs: Vec<OutputName>,
        participants: Vec<ProtocolDesignParticipant>,
        stages: Vec<ProtocolDesignStage>,
        final_approval: Option<ProtocolDesignFinalApproval>,
        step_timeout: Option<StepTimeout>,
        max_attempts: Option<StepAttempt>,
        retry_backoff: Option<DurationMs>,
    ) -> Self {
        let stage_entries = stages
            .iter()
            .cloned()
            .map(ProtocolDesignStageEntry::Leaf)
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
    pub const fn with_pattern(mut self, pattern: ProtocolPatternPreset) -> Self {
        self.pattern = Some(pattern);
        self
    }

    #[must_use]
    pub const fn name(&self) -> &ProtocolName {
        &self.name
    }

    #[must_use]
    pub fn version(&self) -> Option<&ProtocolVersion> {
        self.version.as_ref()
    }

    #[must_use]
    pub const fn objective(&self) -> &ProtocolDescription {
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
    pub fn participants(&self) -> &[ProtocolDesignParticipant] {
        &self.participants
    }

    #[must_use]
    pub fn stages(&self) -> &[ProtocolDesignStage] {
        &self.stages
    }

    #[must_use]
    pub fn stage_entries(&self) -> &[ProtocolDesignStageEntry] {
        &self.stage_entries
    }

    #[must_use]
    pub fn with_stage_entries(mut self, entries: Vec<ProtocolDesignStageEntry>) -> Self {
        self.stages = entries
            .iter()
            .filter_map(|entry| match entry {
                ProtocolDesignStageEntry::Leaf(stage) => Some(stage.clone()),
                ProtocolDesignStageEntry::Group(_) | ProtocolDesignStageEntry::Pattern(_) => None,
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
    pub fn final_approval(&self) -> Option<&ProtocolDesignFinalApproval> {
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
    pub const fn pattern(&self) -> Option<ProtocolPatternPreset> {
        self.pattern
    }

    #[must_use]
    pub(crate) fn state_pattern(
        &self,
        entry_id: &made_core::value_objects::StepId,
    ) -> Option<ProtocolStagePatternKind> {
        self.state_patterns.get(entry_id).copied()
    }

    pub(crate) fn materialized_with_entries(
        &self,
        entries: Vec<ProtocolDesignStageEntry>,
        state_patterns: BTreeMap<made_core::value_objects::StepId, ProtocolStagePatternKind>,
        routes: Vec<ProtocolDesignRoute>,
    ) -> Self {
        let mut materialized = self.clone().with_stage_entries(entries);
        materialized.state_patterns = state_patterns;
        materialized.routes = routes;
        materialized
    }

    pub(crate) fn routes(&self) -> &[ProtocolDesignRoute] {
        &self.routes
    }

    pub(crate) fn materialized_with_stages(&self, stages: Vec<ProtocolDesignStage>) -> Self {
        let mut materialized = self.clone();
        materialized.stages = stages;
        materialized.stage_entries = materialized
            .stages
            .iter()
            .cloned()
            .map(ProtocolDesignStageEntry::Leaf)
            .collect();
        materialized.pattern = None;
        materialized.state_patterns.clear();
        materialized.routes.clear();
        materialized
    }
}
