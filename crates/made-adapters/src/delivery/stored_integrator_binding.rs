use made_core::ports::{BindOutcome, BindReplacement};
use made_core::value_objects::{IntegratorBinding, IntegratorBindingId};
use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

/// Every integrator ever bound to one scope, oldest first.
///
/// Revoked bindings stay: "who was driving this ceremony when it went
/// quiet" is a question an operator asks afterwards, and a store that
/// only kept the current binding could not answer it.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub(crate) struct StoredIntegratorBinding(Vec<IntegratorBinding>);

impl StoredIntegratorBinding {
    pub(crate) const fn new() -> Self {
        Self(Vec::new())
    }

    /// The binding in force, if one is.
    pub(crate) fn current(&self) -> Option<&IntegratorBinding> {
        self.0.iter().rev().find(|binding| binding.is_live())
    }

    pub(crate) fn bindings(&self) -> &[IntegratorBinding] {
        &self.0
    }

    /// Take an offered binding, or say who already has the scope.
    pub(crate) fn bind(
        &self,
        offered: IntegratorBinding,
        replacement: BindReplacement,
    ) -> (Option<Self>, BindOutcome) {
        let Some(incumbent) = self.current() else {
            let mut next = self.clone();
            next.0.push(offered.clone());
            return (Some(next), BindOutcome::Bound(offered));
        };
        if incumbent.id() == offered.id() && incumbent.incarnation() == offered.incarnation() {
            return (None, BindOutcome::AlreadyBound(incumbent.clone()));
        }
        if !replacement.replaces() {
            return (
                None,
                BindOutcome::AlreadyExists {
                    existing: Box::new(incumbent.clone()),
                },
            );
        }
        let previous = incumbent.clone();
        let current = previous.replacing(&offered);
        let mut next = self.clone();
        next.retire(previous.id(), current.bound_at());
        next.0.push(current.clone());
        (
            Some(next),
            BindOutcome::Replaced {
                previous: Box::new(previous),
                current: Box::new(current),
            },
        )
    }

    /// End one binding of this scope, if it is here and still live.
    pub(crate) fn revoke(
        &self,
        id: &IntegratorBindingId,
        now: OffsetDateTime,
    ) -> (Option<Self>, Option<IntegratorBinding>) {
        let Some(position) = self
            .0
            .iter()
            .position(|binding| binding.id() == id && binding.is_live())
        else {
            let found = self.0.iter().find(|binding| binding.id() == id).cloned();
            return (None, found);
        };
        let mut next = self.clone();
        let revoked = next.0[position].revoked(now);
        next.0[position] = revoked.clone();
        (Some(next), Some(revoked))
    }

    fn retire(&mut self, id: &IntegratorBindingId, at: OffsetDateTime) {
        for binding in &mut self.0 {
            if binding.id() == id && binding.is_live() {
                *binding = binding.revoked(at);
            }
        }
    }
}
