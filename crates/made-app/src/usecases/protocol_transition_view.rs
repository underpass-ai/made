use made_core::value_objects::ProtocolTransition;

use super::protocol_guard_view::ProtocolGuardView;

/// A transition leaving the current state with its derived readiness.
#[derive(Debug, Clone)]
pub struct ProtocolTransitionView<'a> {
    transition: &'a ProtocolTransition,
    enabled: bool,
    automated_preconditions_satisfied: bool,
    guards: Vec<ProtocolGuardView<'a>>,
}

impl<'a> ProtocolTransitionView<'a> {
    pub(super) fn new(
        transition: &'a ProtocolTransition,
        enabled: bool,
        automated_preconditions_satisfied: bool,
        guards: Vec<ProtocolGuardView<'a>>,
    ) -> Self {
        Self {
            transition,
            enabled,
            automated_preconditions_satisfied,
            guards,
        }
    }

    #[must_use]
    pub fn transition(&self) -> &'a ProtocolTransition {
        self.transition
    }

    #[must_use]
    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    #[must_use]
    pub fn guards(&self) -> &[ProtocolGuardView<'a>] {
        &self.guards
    }

    #[must_use]
    pub(super) fn waits_only_on_people(&self) -> bool {
        self.automated_preconditions_satisfied
            && self
                .guards
                .iter()
                .filter(|guard| !guard.is_human())
                .all(ProtocolGuardView::is_satisfied)
    }
}
