use made_core::value_objects::GuardName;

/// A transition guard and its current satisfaction state.
#[derive(Debug, Clone, Copy)]
pub struct CeremonyGuardView<'a> {
    name: &'a GuardName,
    human: bool,
    satisfied: bool,
}

impl<'a> CeremonyGuardView<'a> {
    pub(super) const fn new(name: &'a GuardName, human: bool, satisfied: bool) -> Self {
        Self {
            name,
            human,
            satisfied,
        }
    }

    #[must_use]
    pub fn name(&self) -> &'a GuardName {
        self.name
    }

    #[must_use]
    pub fn is_human(&self) -> bool {
        self.human
    }

    #[must_use]
    pub fn is_satisfied(&self) -> bool {
        self.satisfied
    }
}
