/// Counts of each element declared by a ceremony draft.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CeremonyDraftSummary {
    pub states: usize,
    pub initial_states: usize,
    pub terminal_states: usize,
    pub transitions: usize,
    pub steps: usize,
    pub guards: usize,
    pub roles: usize,
}
