use std::collections::BTreeSet;

use crate::entities::{CeremonyDefinition, CeremonyInstance};
use crate::error::DomainError;
use crate::value_objects::{
    RoleAction, RoleId, StateExecution, StepExecutionRecord, StepId, StepStatus,
};

impl CeremonyInstance {
    pub fn resolved_step_role(
        &self,
        definition: &CeremonyDefinition,
        step_id: &StepId,
    ) -> Result<RoleId, DomainError> {
        self.resolve_step_role(definition, step_id, None)
            .map(|(role, _)| role)
    }

    pub(super) fn resolve_step_role(
        &self,
        definition: &CeremonyDefinition,
        step_id: &StepId,
        requested_role: Option<&RoleId>,
    ) -> Result<(RoleId, bool), DomainError> {
        let step = definition.step(step_id).ok_or(DomainError::NotFound {
            what: "ceremony_instance.step",
        })?;
        let Some(binding) = step.dynamic_role_binding() else {
            let role = requested_role
                .cloned()
                .map_or_else(|| definition.role_id_for_step(step_id), Ok)?;
            self.require_role(definition, &role, &RoleAction::step(step_id.clone()))?;
            self.require_role_is_available(definition, step_id, &role)?;
            return Ok((role, false));
        };

        let raw = self
            .context
            .get(binding.context_key())
            .ok_or(DomainError::NotFound {
                what: "ceremony_step.role_from.context_key",
            })?;
        let raw = raw.as_str().ok_or(DomainError::InvariantViolated {
            reason: "dynamic role context value must be a string",
        })?;
        let resolved = RoleId::new(raw)?;
        if !binding.allows(&resolved) {
            return Err(DomainError::InvariantViolated {
                reason: "dynamic role is outside the step allow-list",
            });
        }
        if requested_role.is_some_and(|requested| requested != &resolved) {
            return Err(DomainError::InvariantViolated {
                reason: "requested step role differs from the dynamic role",
            });
        }
        self.require_role(definition, &resolved, &RoleAction::step(step_id.clone()))?;
        self.require_role_is_available(definition, step_id, &resolved)?;
        Ok((resolved, true))
    }

    fn require_role_is_available(
        &self,
        definition: &CeremonyDefinition,
        step_id: &StepId,
        role: &RoleId,
    ) -> Result<(), DomainError> {
        let concurrent = definition
            .state(&self.current_state)
            .is_some_and(|state| state.execution() == StateExecution::Concurrent);
        if concurrent
            && self
                .roles_assigned_to_other_steps(definition, step_id)?
                .contains(role)
        {
            return Err(DomainError::InvariantViolated {
                reason: "role is already assigned to another step in this state iteration",
            });
        }
        Ok(())
    }

    fn roles_assigned_to_other_steps(
        &self,
        definition: &CeremonyDefinition,
        step_id: &StepId,
    ) -> Result<BTreeSet<RoleId>, DomainError> {
        let current = self.current_state_iteration;
        let mut roles = BTreeSet::new();
        for (other_id, record) in &self.step_records {
            if other_id != step_id
                && record.state_iteration() == current
                && definition
                    .step(other_id)
                    .is_some_and(|step| step.state_id() == &self.current_state)
            {
                roles.extend(reserved_role(definition, other_id, record)?);
            }
        }
        for (other_id, history) in &self.step_record_history {
            if other_id == step_id {
                continue;
            }
            for record in history {
                if record.state_iteration() == current
                    && definition
                        .step(other_id)
                        .is_some_and(|step| step.state_id() == &self.current_state)
                {
                    roles.extend(reserved_role(definition, other_id, record)?);
                }
            }
        }
        Ok(roles)
    }
}

fn reserved_role(
    definition: &CeremonyDefinition,
    step_id: &StepId,
    record: &StepExecutionRecord,
) -> Result<Option<RoleId>, DomainError> {
    if let Some(role) = record.claimed_role() {
        return Ok(Some(role.clone()));
    }
    let static_non_pending = record.status() != StepStatus::Pending
        && definition
            .step(step_id)
            .is_some_and(|step| step.dynamic_role_binding().is_none());
    if static_non_pending {
        definition.role_id_for_step(step_id).map(Some)
    } else {
        Ok(None)
    }
}
