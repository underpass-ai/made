//! The little a host is told about the protocol it was woken for.

use made_core::value_objects::{GuardName, ProtocolId, ProtocolLifecyclePhase, StateId, StepId};
use serde::Serialize;

use crate::usecases::ProtocolInstanceView;

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
    protocol_id: ProtocolId,
    lifecycle: ProtocolLifecyclePhase,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    current_state: Option<StateId>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    claimable_step_ids: Vec<StepId>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    waiting_for_human: Vec<GuardName>,
}

impl AttentionContext {
    #[must_use]
    pub fn of(protocol_id: ProtocolId, view: &ProtocolInstanceView<'_>) -> Self {
        Self {
            protocol_id,
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
    pub const fn protocol_id(&self) -> &ProtocolId {
        &self.protocol_id
    }

    #[must_use]
    pub const fn lifecycle(&self) -> ProtocolLifecyclePhase {
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
