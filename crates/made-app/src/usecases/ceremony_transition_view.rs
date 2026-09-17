use made_core::value_objects::CeremonyTransition;

use super::ceremony_guard_view::CeremonyGuardView;

/// A transition leaving the current state with its derived readiness.
#[derive(Debug, Clone)]
pub struct CeremonyTransitionView<'a> {
    transition: &'a CeremonyTransition,
    enabled: bool,
    automated_preconditions_satisfied: bool,
    guards: Vec<CeremonyGuardView<'a>>,
}

impl<'a> CeremonyTransitionView<'a> {
    pub(super) fn new(
        transition: &'a CeremonyTransition,
        enabled: bool,
        automated_preconditions_satisfied: bool,
        guards: Vec<CeremonyGuardView<'a>>,
    ) -> Self {
        Self {
            transition,
            enabled,
            automated_preconditions_satisfied,
            guards,
        }
    }

    #[must_use]
    pub fn transition(&self) -> &'a CeremonyTransition {
        self.transition
    }

    #[must_use]
    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    #[must_use]
    pub fn guards(&self) -> &[CeremonyGuardView<'a>] {
        &self.guards
    }

    #[must_use]
    pub(super) fn waits_only_on_people(&self) -> bool {
        self.automated_preconditions_satisfied
            && self
                .guards
                .iter()
                .filter(|guard| !guard.is_human())
                .all(CeremonyGuardView::is_satisfied)
    }
}
