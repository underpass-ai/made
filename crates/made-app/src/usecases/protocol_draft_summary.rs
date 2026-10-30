use super::ProtocolDraftElementCount;

/// Counts of each element declared by a protocol draft.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProtocolDraftSummary {
    states: ProtocolDraftElementCount,
    initial_states: ProtocolDraftElementCount,
    terminal_states: ProtocolDraftElementCount,
    transitions: ProtocolDraftElementCount,
    steps: ProtocolDraftElementCount,
    guards: ProtocolDraftElementCount,
    roles: ProtocolDraftElementCount,
}

impl ProtocolDraftSummary {
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
            states: ProtocolDraftElementCount::new(states),
            initial_states: ProtocolDraftElementCount::new(initial_states),
            terminal_states: ProtocolDraftElementCount::new(terminal_states),
            transitions: ProtocolDraftElementCount::new(transitions),
            steps: ProtocolDraftElementCount::new(steps),
            guards: ProtocolDraftElementCount::new(guards),
            roles: ProtocolDraftElementCount::new(roles),
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
