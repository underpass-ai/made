use std::collections::{BTreeMap, BTreeSet};

use crate::entities::ceremony_events::CeremonyInstanceStarted;
use crate::entities::CeremonyInstance;
use crate::value_objects::StepExecutionRecord;
use crate::value_objects::{StateIteration, StateVisit};

impl CeremonyInstance {
    /// Open the session the event describes: its initial state, one
    /// pending record per step the definition declared, and nothing
    /// else yet. Needs no definition, because the event carries what
    /// starting derived from one.
    #[must_use]
    pub fn from_started(started: &CeremonyInstanceStarted) -> Self {
        Self {
            lease_renewals: BTreeMap::new(),
            host_handoffs: BTreeMap::new(),
            id: started.ceremony_id.clone(),
            definition_name: started.definition_name.clone(),
            definition_version: started.definition_version.clone(),
            current_state: started.initial_state.clone(),
            current_state_iteration: StateIteration::FIRST,
            current_state_visit: StateVisit::FIRST,
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
            recollection: None,
            idempotency_keys: BTreeSet::new(),
            created_at: started.created_at,
            updated_at: started.created_at,
            completed_at: None,
            bound_definition: started.bound_definition,
            lineage: started.lineage.clone(),
            budget_account_id: started.budget_account_id.clone(),
            child_groups: BTreeMap::new(),
            lifecycle: crate::value_objects::CeremonyLifecycle::default(),
            ceremony_deadline: started.ceremony_deadline,
            state_deadline: started.state_deadline.clone(),
            step_deadlines: BTreeMap::new(),
            retired_deadline_claims: BTreeMap::new(),
            late_step_results: BTreeMap::new(),
            execution_receipt_links: BTreeMap::new(),
            execution_receipt_adoptions: BTreeMap::new(),
        }
    }
}
