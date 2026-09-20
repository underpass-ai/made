use made_core::value_objects::{
    CeremonyId, CeremonyLifecycle, DefinitionPin, LinkStatus, LoopRound, StateId, SystemCeremonyId,
    UnavailabilityReason,
};

/// One composed ceremony, as the design intended it and as it
/// actually stands.
///
/// Both, side by side and labelled. A view that merged them would let
/// a run that skipped a ceremony read like a design that never had
/// one, which is the exact confusion an execution view exists to
/// prevent.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgenticSystemCeremonyView {
    ceremony: SystemCeremonyId,
    pin: DefinitionPin,
    planned: LinkStatus,
    round: LoopRound,
    instance_id: Option<CeremonyId>,
    observed_lifecycle: Option<CeremonyLifecycle>,
    observed_state: Option<StateId>,
    skipped_because: Option<UnavailabilityReason>,
}

impl AgenticSystemCeremonyView {
    #[must_use]
    pub const fn new(
        ceremony: SystemCeremonyId,
        pin: DefinitionPin,
        planned: LinkStatus,
        round: LoopRound,
        instance_id: Option<CeremonyId>,
        skipped_because: Option<UnavailabilityReason>,
    ) -> Self {
        Self {
            ceremony,
            pin,
            planned,
            round,
            instance_id,
            observed_lifecycle: None,
            observed_state: None,
            skipped_because,
        }
    }

    /// The same view with what the instance itself says folded in.
    #[must_use]
    pub fn observing(self, lifecycle: CeremonyLifecycle, state: StateId) -> Self {
        Self {
            observed_lifecycle: Some(lifecycle),
            observed_state: Some(state),
            ..self
        }
    }

    #[must_use]
    pub const fn ceremony(&self) -> &SystemCeremonyId {
        &self.ceremony
    }

    #[must_use]
    pub const fn pin(&self) -> &DefinitionPin {
        &self.pin
    }

    /// What the run recorded about this composition.
    #[must_use]
    pub const fn planned(&self) -> LinkStatus {
        self.planned
    }

    #[must_use]
    pub const fn round(&self) -> LoopRound {
        self.round
    }

    #[must_use]
    pub const fn instance_id(&self) -> Option<&CeremonyId> {
        self.instance_id.as_ref()
    }

    /// What the instance says about itself, when there is one to ask.
    #[must_use]
    pub const fn observed_lifecycle(&self) -> Option<&CeremonyLifecycle> {
        self.observed_lifecycle.as_ref()
    }

    #[must_use]
    pub const fn observed_state(&self) -> Option<&StateId> {
        self.observed_state.as_ref()
    }

    #[must_use]
    pub const fn skipped_because(&self) -> Option<&UnavailabilityReason> {
        self.skipped_because.as_ref()
    }
}
