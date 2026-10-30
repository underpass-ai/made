use std::collections::BTreeMap;

use crate::error::DomainError;
use crate::value_objects::{
    GuardName, InputName, OutputName, ProtocolGuard, ProtocolInputDefinition,
    ProtocolOutputDefinition, ProtocolRole, ProtocolState, ProtocolStep, RoleId, StateId, StepId,
};

pub(super) fn collect_inputs(
    inputs: impl IntoIterator<Item = ProtocolInputDefinition>,
) -> Result<BTreeMap<InputName, ProtocolInputDefinition>, DomainError> {
    let mut map = BTreeMap::new();
    for input in inputs {
        if map.insert(input.name().clone(), input).is_some() {
            return Err(DomainError::AlreadyExists {
                what: "protocol_input",
            });
        }
    }
    Ok(map)
}

pub(super) fn collect_outputs(
    outputs: impl IntoIterator<Item = ProtocolOutputDefinition>,
) -> Result<BTreeMap<OutputName, ProtocolOutputDefinition>, DomainError> {
    let mut map = BTreeMap::new();
    for output in outputs {
        if map.insert(output.name().clone(), output).is_some() {
            return Err(DomainError::AlreadyExists {
                what: "protocol_output",
            });
        }
    }
    Ok(map)
}

pub(super) fn collect_states(
    states: impl IntoIterator<Item = ProtocolState>,
) -> Result<BTreeMap<StateId, ProtocolState>, DomainError> {
    let mut map = BTreeMap::new();
    for state in states {
        if map.insert(state.id().clone(), state).is_some() {
            return Err(DomainError::AlreadyExists {
                what: "protocol_state",
            });
        }
    }
    Ok(map)
}

pub(super) fn collect_steps(
    steps: impl IntoIterator<Item = ProtocolStep>,
) -> Result<(BTreeMap<StepId, ProtocolStep>, Vec<StepId>), DomainError> {
    let mut map = BTreeMap::new();
    let mut order = Vec::new();
    for step in steps {
        let step_id = step.id().clone();
        if map.insert(step_id.clone(), step).is_some() {
            return Err(DomainError::AlreadyExists {
                what: "protocol_step",
            });
        }
        order.push(step_id);
    }
    Ok((map, order))
}

pub(super) fn collect_guards(
    guards: impl IntoIterator<Item = ProtocolGuard>,
) -> Result<BTreeMap<GuardName, ProtocolGuard>, DomainError> {
    let mut map = BTreeMap::new();
    for guard in guards {
        if map.insert(guard.name().clone(), guard).is_some() {
            return Err(DomainError::AlreadyExists {
                what: "protocol_guard",
            });
        }
    }
    Ok(map)
}

pub(super) fn collect_roles(
    roles: impl IntoIterator<Item = ProtocolRole>,
) -> Result<BTreeMap<RoleId, ProtocolRole>, DomainError> {
    let mut map = BTreeMap::new();
    for role in roles {
        if map.insert(role.id().clone(), role).is_some() {
            return Err(DomainError::AlreadyExists {
                what: "protocol_role",
            });
        }
    }
    Ok(map)
}
