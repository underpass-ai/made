use super::CeremonyDraftElementCount;

/// Counts of each element declared by a ceremony draft.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CeremonyDraftSummary {
    states: CeremonyDraftElementCount,
    initial_states: CeremonyDraftElementCount,
    terminal_states: CeremonyDraftElementCount,
    transitions: CeremonyDraftElementCount,
    steps: CeremonyDraftElementCount,
    guards: CeremonyDraftElementCount,
    roles: CeremonyDraftElementCount,
}

impl CeremonyDraftSummary {
    #[must_use]
    pub const fn new(
        states: usize,
        initial_states: usize,
        terminal_states: usize,
        transitions: usize,
        steps: usize,
        guards: usize,
        roles: usize,
    ) -> Self {
        Self {
            states: CeremonyDraftElementCount::new(states),
            initial_states: CeremonyDraftElementCount::new(initial_states),
            terminal_states: CeremonyDraftElementCount::new(terminal_states),
            transitions: CeremonyDraftElementCount::new(transitions),
            steps: CeremonyDraftElementCount::new(steps),
            guards: CeremonyDraftElementCount::new(guards),
            roles: CeremonyDraftElementCount::new(roles),
        }
    }

    #[must_use]
    pub const fn states(self) -> usize {
        self.states.get()
    }
    #[must_use]
    pub const fn initial_states(self) -> usize {
        self.initial_states.get()
    }
    #[must_use]
    pub const fn terminal_states(self) -> usize {
        self.terminal_states.get()
    }
    #[must_use]
    pub const fn transitions(self) -> usize {
        self.transitions.get()
    }
    #[must_use]
    pub const fn steps(self) -> usize {
        self.steps.get()
    }
    #[must_use]
    pub const fn guards(self) -> usize {
        self.guards.get()
    }
    #[must_use]
    pub const fn roles(self) -> usize {
        self.roles.get()
    }
}
