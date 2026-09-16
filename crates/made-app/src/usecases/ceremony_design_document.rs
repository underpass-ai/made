use made_core::value_objects::{
    CeremonyDescription, CeremonyName, CeremonyVersion, InputName, OutputName,
};

use super::ceremony_design_final_approval::CeremonyDesignFinalApproval;
use super::ceremony_design_participant::CeremonyDesignParticipant;
use super::ceremony_design_stage::CeremonyDesignStage;

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
    final_approval: Option<CeremonyDesignFinalApproval>,
    step_timeout_seconds: Option<u64>,
    max_attempts: Option<u32>,
    backoff_seconds: Option<u64>,
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
        step_timeout_seconds: Option<u64>,
        max_attempts: Option<u32>,
        backoff_seconds: Option<u64>,
    ) -> Self {
        Self {
            name,
            version,
            objective,
            required_inputs,
            optional_inputs,
            outputs,
            participants,
            stages,
            final_approval,
            step_timeout_seconds,
            max_attempts,
            backoff_seconds,
        }
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
    pub fn final_approval(&self) -> Option<&CeremonyDesignFinalApproval> {
        self.final_approval.as_ref()
    }

    #[must_use]
    pub const fn step_timeout_seconds(&self) -> Option<u64> {
        self.step_timeout_seconds
    }

    #[must_use]
    pub const fn max_attempts(&self) -> Option<u32> {
        self.max_attempts
    }

    #[must_use]
    pub const fn backoff_seconds(&self) -> Option<u64> {
        self.backoff_seconds
    }
}
