use std::collections::{BTreeMap, BTreeSet};

use crate::entities::ceremony_events::CeremonyInstanceStarted;
use crate::entities::CeremonyInstance;
use crate::value_objects::StepExecutionRecord;

impl CeremonyInstance {
    /// Open the session the event describes: its initial state, one
    /// pending record per step the definition declared, and nothing
    /// else yet. Needs no definition, because the event carries what
    /// starting derived from one.
    #[must_use]
    pub fn from_started(started: &CeremonyInstanceStarted) -> Self {
        Self {
            id: started.ceremony_id.clone(),
            definition_name: started.definition_name.clone(),
            definition_version: started.definition_version.clone(),
            current_state: started.initial_state.clone(),
            step_records: started
                .step_ids
                .iter()
                .map(|step_id| (step_id.clone(), StepExecutionRecord::pending()))
                .collect(),
            step_record_history: BTreeMap::new(),
            interventions: Vec::new(),
            guard_deferrals: Vec::new(),
            guard_approvals: Vec::new(),
            transitions: Vec::new(),
            reasons: Vec::new(),
            participant_bindings: BTreeMap::new(),
            context: started.context.clone(),
            idempotency_keys: BTreeSet::new(),
            created_at: started.created_at,
            updated_at: started.created_at,
            completed_at: None,
            bound_definition: started.bound_definition,
        }
    }

    /// A stream opens once. Applying the opening to a session that
    /// already exists is a programming error in the caller, and the
    /// session is left exactly as it was: a fold that panicked over
    /// it would take a host down over one bad stream, and one that
    /// reopened the session would silently discard everything after
    /// the first opening.
    pub(super) fn apply_started(&mut self, _started: &CeremonyInstanceStarted) {}
}
