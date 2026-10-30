use std::collections::BTreeMap;

use made_core::entities::ProtocolDefinitionDraft;
use made_core::error::DomainError;
use made_core::value_objects::{
    MaxBounces, MaxParallel, MaxTransitions, OutputName, ProtocolDescription, ProtocolName,
    ProtocolOutputDefinition, ProtocolVersion,
};
use serde::Deserialize;
use serde_json::Value;

use super::protocol_guard_document::ProtocolGuardDocument;
use super::protocol_inputs_document::ProtocolInputsDocument;
use super::protocol_role_document::ProtocolRoleDocument;
use super::protocol_state_document::ProtocolStateDocument;
use super::protocol_step_document::ProtocolStepDocument;
use super::protocol_timeouts_document::ProtocolTimeoutsDocument;
use super::protocol_transition_document::ProtocolTransitionDocument;
use super::retry_policies_document::RetryPoliciesDocument;

#[derive(Debug, Clone, Deserialize)]
pub(super) struct ProtocolDefinitionDocument {
    version: String,
    name: String,
    #[serde(default)]
    description: Option<String>,
    #[serde(default)]
    inputs: ProtocolInputsDocument,
    #[serde(default)]
    outputs: BTreeMap<String, Value>,
    #[serde(default)]
    states: Vec<ProtocolStateDocument>,
    #[serde(default)]
    transitions: Vec<ProtocolTransitionDocument>,
    #[serde(default)]
    steps: Vec<ProtocolStepDocument>,
    #[serde(default)]
    guards: BTreeMap<String, ProtocolGuardDocument>,
    #[serde(default)]
    roles: Vec<ProtocolRoleDocument>,
    #[serde(default)]
    timeouts: ProtocolTimeoutsDocument,
    #[serde(default)]
    retry_policies: RetryPoliciesDocument,
    #[serde(default = "default_max_parallel")]
    max_parallel: u8,
    #[serde(default)]
    max_transitions: Option<u32>,
    #[serde(default)]
    max_bounces: Option<u32>,
}

impl ProtocolDefinitionDocument {
    /// Build the authoring draft without enforcing the state-machine
    /// invariants.
    ///
    /// Failures here are parse failures — a malformed identifier, an
    /// unusable retry policy — not structural defects. Structural
    /// defects belong in the draft's analysis report, where they can be
    /// reported all at once.
    pub(super) fn into_draft(self) -> Result<ProtocolDefinitionDraft, DomainError> {
        let retry_policy = self.retry_policies.default_policy()?;
        let timeout = self.timeouts.default_step_timeout()?;
        let protocol_timeout = self.timeouts.protocol_timeout()?;
        let state_timeout = self.timeouts.state_timeout()?;
        let steps = self
            .steps
            .into_iter()
            .map(|step| step.into_domain(retry_policy, timeout))
            .collect::<Result<Vec<_>, _>>()?;
        let step_ids = steps.iter().map(|step| step.id().clone()).collect();

        let transitions = self
            .transitions
            .into_iter()
            .map(ProtocolTransitionDocument::into_domain)
            .collect::<Result<Vec<_>, _>>()?;
        let transition_triggers = transitions
            .iter()
            .map(|transition| transition.trigger().clone())
            .collect();

        let guards = self
            .guards
            .into_iter()
            .map(|(name, guard)| guard.into_domain(name))
            .collect::<Result<Vec<_>, _>>()?;
        let roles = self
            .roles
            .into_iter()
            .map(|role| role.into_domain(&step_ids, &transition_triggers))
            .collect::<Result<Vec<_>, _>>()?;

        let mut draft = ProtocolDefinitionDraft::new(
            ProtocolName::new(self.name)?,
            ProtocolVersion::new(self.version)?,
            self.description.map(ProtocolDescription::new).transpose()?,
            self.inputs.into_domain()?,
            self.outputs
                .into_keys()
                .map(|name| OutputName::new(name).map(ProtocolOutputDefinition::new))
                .collect::<Result<Vec<_>, _>>()?,
            self.states
                .into_iter()
                .map(ProtocolStateDocument::into_domain)
                .collect::<Result<Vec<_>, _>>()?,
            transitions,
            steps,
            guards,
            roles,
        )
        .with_max_parallel(MaxParallel::new(self.max_parallel)?);
        if let Some(limit) = self.max_transitions {
            draft = draft.with_max_transitions(MaxTransitions::new(limit)?);
        }
        if let Some(limit) = self.max_bounces {
            draft = draft.with_max_bounces(MaxBounces::new(limit)?);
        }
        if let Some(timeout) = protocol_timeout {
            draft = draft.with_protocol_timeout(timeout);
        }
        if let Some(timeout) = state_timeout {
            draft = draft.with_state_timeout(timeout);
        }
        Ok(draft)
    }
}

const fn default_max_parallel() -> u8 {
    3
}
