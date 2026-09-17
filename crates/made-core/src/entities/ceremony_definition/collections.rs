use std::collections::BTreeMap;

use crate::error::DomainError;
use crate::value_objects::{
    CeremonyGuard, CeremonyInputDefinition, CeremonyOutputDefinition, CeremonyRole, CeremonyState,
    CeremonyStep, GuardName, InputName, OutputName, RoleId, StateId, StepId,
};

pub(super) fn collect_inputs(
    inputs: impl IntoIterator<Item = CeremonyInputDefinition>,
) -> Result<BTreeMap<InputName, CeremonyInputDefinition>, DomainError> {
    let mut map = BTreeMap::new();
    for input in inputs {
        if map.insert(input.name().clone(), input).is_some() {
            return Err(DomainError::AlreadyExists {
                what: "ceremony_input",
            });
        }
    }
    Ok(map)
}

pub(super) fn collect_outputs(
    outputs: impl IntoIterator<Item = CeremonyOutputDefinition>,
) -> Result<BTreeMap<OutputName, CeremonyOutputDefinition>, DomainError> {
    let mut map = BTreeMap::new();
    for output in outputs {
        if map.insert(output.name().clone(), output).is_some() {
            return Err(DomainError::AlreadyExists {
                what: "ceremony_output",
            });
        }
    }
    Ok(map)
}

pub(super) fn collect_states(
    states: impl IntoIterator<Item = CeremonyState>,
) -> Result<BTreeMap<StateId, CeremonyState>, DomainError> {
    let mut map = BTreeMap::new();
    for state in states {
        if map.insert(state.id().clone(), state).is_some() {
            return Err(DomainError::AlreadyExists {
                what: "ceremony_state",
            });
        }
    }
    Ok(map)
}

pub(super) fn collect_steps(
    steps: impl IntoIterator<Item = CeremonyStep>,
) -> Result<(BTreeMap<StepId, CeremonyStep>, Vec<StepId>), DomainError> {
    let mut map = BTreeMap::new();
    let mut order = Vec::new();
    for step in steps {
        let step_id = step.id().clone();
        if map.insert(step_id.clone(), step).is_some() {
            return Err(DomainError::AlreadyExists {
                what: "ceremony_step",
            });
        }
        order.push(step_id);
    }
    Ok((map, order))
}

pub(super) fn collect_guards(
    guards: impl IntoIterator<Item = CeremonyGuard>,
) -> Result<BTreeMap<GuardName, CeremonyGuard>, DomainError> {
    let mut map = BTreeMap::new();
    for guard in guards {
        if map.insert(guard.name().clone(), guard).is_some() {
            return Err(DomainError::AlreadyExists {
                what: "ceremony_guard",
            });
        }
    }
    Ok(map)
}

pub(super) fn collect_roles(
    roles: impl IntoIterator<Item = CeremonyRole>,
) -> Result<BTreeMap<RoleId, CeremonyRole>, DomainError> {
    let mut map = BTreeMap::new();
    for role in roles {
        if map.insert(role.id().clone(), role).is_some() {
            return Err(DomainError::AlreadyExists {
                what: "ceremony_role",
            });
        }
    }
    Ok(map)
}
