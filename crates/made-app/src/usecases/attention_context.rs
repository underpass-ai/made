//! The little a host is told about the ceremony it was woken for.

use made_core::value_objects::{CeremonyId, CeremonyLifecyclePhase, GuardName, StateId, StepId};
use serde::Serialize;

use crate::usecases::CeremonyInstanceView;

/// Enough context to decide what to do next, and not enough to act on
/// blindly.
///
/// Deliberately thin. Whatever travels with an attention item was true
/// when the batch was built and may not be by the time the host acts,
/// so the documented sequence has the integrator revalidate the
/// instance before doing anything. Carrying a fuller snapshot here
/// would only make that staleness harder to notice.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct AttentionContext {
    ceremony_id: CeremonyId,
    lifecycle: CeremonyLifecyclePhase,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    current_state: Option<StateId>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    claimable_step_ids: Vec<StepId>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    waiting_for_human: Vec<GuardName>,
}

impl AttentionContext {
    #[must_use]
    pub fn of(ceremony_id: CeremonyId, view: &CeremonyInstanceView<'_>) -> Self {
        Self {
            ceremony_id,
            lifecycle: view.instance().lifecycle().phase(),
            current_state: Some(view.instance().current_state().clone()),
            claimable_step_ids: view
                .claimable_step_ids()
                .iter()
                .map(|step| (*step).clone())
                .collect(),
            waiting_for_human: view
                .waiting_for_human()
                .iter()
                .map(|guard| (*guard).clone())
                .collect(),
        }
    }

    #[must_use]
    pub const fn ceremony_id(&self) -> &CeremonyId {
        &self.ceremony_id
    }

    #[must_use]
    pub const fn lifecycle(&self) -> CeremonyLifecyclePhase {
        self.lifecycle
    }

    #[must_use]
    pub const fn current_state(&self) -> Option<&StateId> {
        self.current_state.as_ref()
    }

    #[must_use]
    pub fn claimable_step_ids(&self) -> &[StepId] {
        &self.claimable_step_ids
    }

    #[must_use]
    pub fn waiting_for_human(&self) -> &[GuardName] {
        &self.waiting_for_human
    }
}
