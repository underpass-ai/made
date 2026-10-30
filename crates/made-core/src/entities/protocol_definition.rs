//! [`ProtocolDefinition`] aggregate.
//!
//! A protocol definition is the declarative state machine extracted
//! from the original laboratory protocol engine. It is intentionally
//! pure domain: no YAML, no transport, no handler registry.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::error::DomainError;
use crate::value_objects::{
    GuardName, InputName, MaxBounces, MaxParallel, MaxTransitions, OutputName, ProtocolContext,
    ProtocolDefinitionDigest, ProtocolDefinitionDigestScheme, ProtocolDescription, ProtocolGuard,
    ProtocolInputDefinition, ProtocolName, ProtocolOutputDefinition, ProtocolRole, ProtocolState,
    ProtocolStep, ProtocolTimeout, ProtocolTransition, ProtocolValidationReport, ProtocolVersion,
    RoleAction, RoleId, StateId, StateTimeout, StepExecutionRecord, StepId, TransitionTrigger,
};

use super::protocol_definition_analysis::ProtocolDefinitionParts;

mod collections;
mod guards;

use collections::{
    collect_guards, collect_inputs, collect_outputs, collect_roles, collect_states, collect_steps,
};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProtocolDefinition {
    name: ProtocolName,
    version: ProtocolVersion,
    description: Option<ProtocolDescription>,
    inputs: BTreeMap<InputName, ProtocolInputDefinition>,
    outputs: BTreeMap<OutputName, ProtocolOutputDefinition>,
    states: BTreeMap<StateId, ProtocolState>,
    transitions: Vec<ProtocolTransition>,
    steps: BTreeMap<StepId, ProtocolStep>,
    step_order: Vec<StepId>,
    guards: BTreeMap<GuardName, ProtocolGuard>,
    roles: BTreeMap<RoleId, ProtocolRole>,
    #[serde(default, skip_serializing_if = "MaxParallel::is_default")]
    max_parallel: MaxParallel,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    max_transitions: Option<MaxTransitions>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    max_bounces: Option<MaxBounces>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    protocol_timeout: Option<ProtocolTimeout>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    state_timeout: Option<StateTimeout>,
}

impl ProtocolDefinition {
    pub fn new(
        name: ProtocolName,
        version: ProtocolVersion,
        description: Option<ProtocolDescription>,
        inputs: impl IntoIterator<Item = ProtocolInputDefinition>,
        outputs: impl IntoIterator<Item = ProtocolOutputDefinition>,
        states: impl IntoIterator<Item = ProtocolState>,
        transitions: impl IntoIterator<Item = ProtocolTransition>,
        steps: impl IntoIterator<Item = ProtocolStep>,
        guards: impl IntoIterator<Item = ProtocolGuard>,
        roles: impl IntoIterator<Item = ProtocolRole>,
    ) -> Result<Self, DomainError> {
        Self::new_with_transition_budgets(
            name,
            version,
            description,
            inputs,
            outputs,
            states,
            transitions,
            steps,
            guards,
            roles,
            None,
            None,
        )
    }

    /// Construct with transition budgets installed before structural validation.
    ///
    /// A cyclic graph cannot be constructed uncapped and repaired afterward:
    /// the missing bound is itself a blocking definition defect.
    #[allow(clippy::too_many_arguments)]
    pub fn new_with_transition_budgets(
        name: ProtocolName,
        version: ProtocolVersion,
        description: Option<ProtocolDescription>,
        inputs: impl IntoIterator<Item = ProtocolInputDefinition>,
        outputs: impl IntoIterator<Item = ProtocolOutputDefinition>,
        states: impl IntoIterator<Item = ProtocolState>,
        transitions: impl IntoIterator<Item = ProtocolTransition>,
        steps: impl IntoIterator<Item = ProtocolStep>,
        guards: impl IntoIterator<Item = ProtocolGuard>,
        roles: impl IntoIterator<Item = ProtocolRole>,
        max_transitions: Option<MaxTransitions>,
        max_bounces: Option<MaxBounces>,
    ) -> Result<Self, DomainError> {
        let inputs = collect_inputs(inputs)?;
        let outputs = collect_outputs(outputs)?;
        let states = collect_states(states)?;
        let transitions = transitions.into_iter().collect::<Vec<_>>();
        let (steps, step_order) = collect_steps(steps)?;
        let guards = collect_guards(guards)?;
        let roles = collect_roles(roles)?;

        let definition = Self {
            name,
            version,
            description,
            inputs,
            outputs,
            states,
            transitions,
            steps,
            step_order,
            guards,
            roles,
            max_parallel: MaxParallel::default(),
            max_transitions,
            max_bounces,
            protocol_timeout: None,
            state_timeout: None,
        };
        definition.validate()?;
        Ok(definition)
    }

    #[must_use]
    pub fn name(&self) -> &ProtocolName {
        &self.name
    }

    #[must_use]
    pub fn version(&self) -> &ProtocolVersion {
        &self.version
    }

    #[must_use]
    pub fn description(&self) -> Option<&ProtocolDescription> {
        self.description.as_ref()
    }

    #[must_use]
    pub fn inputs(&self) -> &BTreeMap<InputName, ProtocolInputDefinition> {
        &self.inputs
    }

    #[must_use]
    pub fn outputs(&self) -> &BTreeMap<OutputName, ProtocolOutputDefinition> {
        &self.outputs
    }

    #[must_use]
    pub fn states(&self) -> &BTreeMap<StateId, ProtocolState> {
        &self.states
    }

    #[must_use]
    pub fn transitions(&self) -> &[ProtocolTransition] {
        &self.transitions
    }

    #[must_use]
    pub fn steps(&self) -> &BTreeMap<StepId, ProtocolStep> {
        &self.steps
    }

    /// Iterate over every step in its declaration order.
    ///
    /// Step identifiers remain indexed separately for efficient lookup;
    /// execution order is an explicit part of the protocol definition.
    pub fn steps_in_declaration_order(&self) -> impl Iterator<Item = &ProtocolStep> + '_ {
        self.step_order.iter().map(|step_id| {
            self.steps
                .get(step_id)
                .expect("protocol step order must reference an indexed step")
        })
    }

    #[must_use]
    pub fn guards(&self) -> &BTreeMap<GuardName, ProtocolGuard> {
        &self.guards
    }

    #[must_use]
    pub fn roles(&self) -> &BTreeMap<RoleId, ProtocolRole> {
        &self.roles
    }

    #[must_use]
    pub fn with_max_parallel(mut self, max_parallel: MaxParallel) -> Self {
        self.max_parallel = max_parallel;
        self
    }

    #[must_use]
    pub fn max_parallel(&self) -> MaxParallel {
        self.max_parallel
    }

    #[must_use]
    pub const fn max_transitions(&self) -> Option<MaxTransitions> {
        self.max_transitions
    }

    #[must_use]
    pub const fn max_bounces(&self) -> Option<MaxBounces> {
        self.max_bounces
    }

    #[must_use]
    pub const fn with_protocol_timeout(mut self, timeout: ProtocolTimeout) -> Self {
        self.protocol_timeout = Some(timeout);
        self
    }
    #[must_use]
    pub const fn with_state_timeout(mut self, timeout: StateTimeout) -> Self {
        self.state_timeout = Some(timeout);
        self
    }
    #[must_use]
    pub const fn protocol_timeout(&self) -> Option<ProtocolTimeout> {
        self.protocol_timeout
    }
    #[must_use]
    pub const fn state_timeout(&self) -> Option<StateTimeout> {
        self.state_timeout
    }

    #[must_use]
    pub fn initial_state_id(&self) -> &StateId {
        self.states
            .values()
            .find(|state| state.is_initial())
            .map(ProtocolState::id)
            .expect("protocol definition invariant requires one initial state")
    }

    #[must_use]
    pub fn state(&self, state_id: &StateId) -> Option<&ProtocolState> {
        self.states.get(state_id)
    }

    #[must_use]
    pub fn step(&self, step_id: &StepId) -> Option<&ProtocolStep> {
        self.steps.get(step_id)
    }

    #[must_use]
    pub fn role(&self, role_id: &RoleId) -> Option<&ProtocolRole> {
        self.roles.get(role_id)
    }

    pub fn steps_for_state(&self, state_id: &StateId) -> impl Iterator<Item = &ProtocolStep> + '_ {
        let state_id = state_id.clone();
        self.steps_in_declaration_order()
            .filter(move |step| step.state_id() == &state_id)
    }

    #[must_use]
    pub fn is_terminal_state(&self, state_id: &StateId) -> bool {
        self.states
            .get(state_id)
            .is_some_and(ProtocolState::is_terminal)
    }

    #[must_use]
    pub fn transition_for_trigger(
        &self,
        state_id: &StateId,
        trigger: &TransitionTrigger,
    ) -> Option<&ProtocolTransition> {
        self.transitions
            .iter()
            .find(|transition| transition.from() == state_id && transition.trigger() == trigger)
    }

    pub fn available_transitions(
        &self,
        state_id: &StateId,
    ) -> impl Iterator<Item = &ProtocolTransition> + '_ {
        let state_id = state_id.clone();
        self.transitions
            .iter()
            .filter(move |transition| transition.from() == &state_id)
    }

    #[must_use]
    pub fn role_allows(&self, role_id: &RoleId, action: &RoleAction) -> bool {
        self.roles
            .get(role_id)
            .is_some_and(|role| role.allows(action))
    }

    /// Find the role authorised to perform `action`, if any.
    ///
    /// Roles are scanned in id order and the first whose action set
    /// permits `action` is returned. Yields `None` when no declared role
    /// is allowed to perform it.
    #[must_use]
    pub fn role_for_action(&self, action: &RoleAction) -> Option<&ProtocolRole> {
        self.roles.values().find(|role| role.allows(action))
    }

    /// Resolve the role authorised to execute `step_id`.
    ///
    /// Fails fast with [`DomainError::InvariantViolated`] when no role is
    /// allowed to run the step — a protocol cannot execute a step nobody
    /// owns.
    pub fn role_id_for_step(&self, step_id: &StepId) -> Result<RoleId, DomainError> {
        self.role_for_action(&RoleAction::step(step_id.clone()))
            .map(|role| role.id().clone())
            .ok_or(DomainError::InvariantViolated {
                reason: "no protocol role can execute step",
            })
    }

    /// Resolve the role authorised to apply the transition fired by
    /// `trigger`.
    ///
    /// Fails fast with [`DomainError::InvariantViolated`] when no role is
    /// allowed to apply it — a protocol cannot advance through a
    /// transition nobody owns.
    pub fn role_id_for_transition(
        &self,
        trigger: &TransitionTrigger,
    ) -> Result<RoleId, DomainError> {
        self.role_for_action(&RoleAction::transition(trigger.clone()))
            .map(|role| role.id().clone())
            .ok_or(DomainError::InvariantViolated {
                reason: "no protocol role can apply transition",
            })
    }

    /// Select the next transition out of `state_id` whose guards are all
    /// currently satisfied by `records` and `context`.
    ///
    /// Outgoing transitions are evaluated in declaration order and the
    /// first one that is fully enabled is returned. Yields `None` when
    /// the state has no outgoing transition whose guards hold — either
    /// because the state is terminal or because the protocol is not yet
    /// ready to advance.
    #[must_use]
    pub fn next_satisfied_transition(
        &self,
        state_id: &StateId,
        records: &BTreeMap<StepId, StepExecutionRecord>,
        context: &ProtocolContext,
    ) -> Option<&ProtocolTransition> {
        self.available_transitions(state_id)
            .find(|transition| self.guards_are_satisfied(transition, records, context))
    }

    /// The identity of this definition's content.
    ///
    /// Computed over canonical JSON of the whole aggregate rather than
    /// over the document it arrived in: two YAML files differing in
    /// whitespace, key order or comments describe the same working
    /// session and must agree, while any material difference must not.
    ///
    /// Encoding the aggregate through `serde` rather than by hand is
    /// deliberate. A hand-written encoder that forgets a field produces
    /// two materially different definitions with one digest, and
    /// nothing would report it; here a field cannot be left out, and a
    /// field added later changes the digest, which is correct because
    /// it is material.
    ///
    /// Canonical because `serde_json` maps are ordered — see
    /// `serde_json_emits_sorted_keys` for the guard that keeps that
    /// assumption from being silently withdrawn.
    pub fn digest(&self) -> Result<ProtocolDefinitionDigest, DomainError> {
        self.digest_under(ProtocolDefinitionDigestScheme::CURRENT)
    }

    /// The digest of this content under a given scheme.
    ///
    /// Only verification of an existing publication needs a scheme other
    /// than the current one; new publications are sealed with
    /// [`Self::digest`].
    pub fn digest_under(
        &self,
        scheme: ProtocolDefinitionDigestScheme,
    ) -> Result<ProtocolDefinitionDigest, DomainError> {
        let canonical = self.canonical_form()?;
        Ok(ProtocolDefinitionDigest::of_canonical_form(
            scheme, &canonical,
        ))
    }

    fn canonical_form(&self) -> Result<Vec<u8>, DomainError> {
        serde_json::to_vec(self).map_err(|_| DomainError::InvariantViolated {
            reason: "protocol definition cannot be rendered canonically",
        })
    }

    /// Collect every defect in the definition instead of stopping at
    /// the first one.
    ///
    /// A single error is enough to reject a definition but not enough
    /// to correct one. An author — human or agent — needs the full set
    /// to fix a draft in one pass.
    ///
    /// Findings are emitted in check order, so the first blocking one
    /// is exactly the error [`Self::new`] raises.
    #[must_use]
    pub fn analyze(&self) -> ProtocolValidationReport {
        let mut findings = Vec::new();
        self.parts().collect_findings(&mut findings);
        ProtocolValidationReport::new(findings)
    }

    fn parts(&self) -> ProtocolDefinitionParts<'_> {
        ProtocolDefinitionParts {
            states: &self.states,
            transitions: &self.transitions,
            steps: &self.steps,
            step_order: &self.step_order,
            guards: &self.guards,
            roles: &self.roles,
            max_transitions: self.max_transitions,
            max_bounces: self.max_bounces,
        }
    }

    fn validate(&self) -> Result<(), DomainError> {
        match self.analyze().first_error() {
            Some(finding) => Err(finding.defect().clone()),
            None => Ok(()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::value_objects::{
        GuardCondition, ProtocolStateKind, ProtocolValidationLocus, RetryPolicy, StepHandlerConfig,
        StepHandlerKind, StepStatus,
    };

    fn name() -> ProtocolName {
        ProtocolName::new("planning_protocol").unwrap()
    }

    fn state_id(raw: &str) -> StateId {
        StateId::new(raw).unwrap()
    }

    fn step_id(raw: &str) -> StepId {
        StepId::new(raw).unwrap()
    }

    fn guard_name(raw: &str) -> GuardName {
        GuardName::new(raw).unwrap()
    }

    fn trigger(raw: &str) -> TransitionTrigger {
        TransitionTrigger::new(raw).unwrap()
    }

    fn handler_kind() -> StepHandlerKind {
        StepHandlerKind::new("manual_review").unwrap()
    }

    fn step(raw_step_id: &str, raw_state_id: &str) -> ProtocolStep {
        ProtocolStep::new(
            step_id(raw_step_id),
            state_id(raw_state_id),
            handler_kind(),
            StepHandlerConfig::empty(),
            RetryPolicy::single_attempt(),
            None,
        )
    }

    fn role(actions: Vec<RoleAction>) -> ProtocolRole {
        ProtocolRole::new(RoleId::new("facilitator").unwrap(), actions).unwrap()
    }

    fn definition(
        states: Vec<ProtocolState>,
        transitions: Vec<ProtocolTransition>,
        steps: Vec<ProtocolStep>,
        guards: Vec<ProtocolGuard>,
        roles: Vec<ProtocolRole>,
    ) -> Result<ProtocolDefinition, DomainError> {
        definition_with_budgets(states, transitions, steps, guards, roles, None, None)
    }

    fn definition_with_budgets(
        states: Vec<ProtocolState>,
        transitions: Vec<ProtocolTransition>,
        steps: Vec<ProtocolStep>,
        guards: Vec<ProtocolGuard>,
        roles: Vec<ProtocolRole>,
        max_transitions: Option<MaxTransitions>,
        max_bounces: Option<MaxBounces>,
    ) -> Result<ProtocolDefinition, DomainError> {
        ProtocolDefinition::new_with_transition_budgets(
            name(),
            ProtocolVersion::v1(),
            None,
            Vec::new(),
            Vec::new(),
            states,
            transitions,
            steps,
            guards,
            roles,
            max_transitions,
            max_bounces,
        )
    }

    fn valid_definition() -> ProtocolDefinition {
        let plan_step = step("plan", "drafting");
        let guard = ProtocolGuard::new(
            guard_name("plan_done"),
            GuardCondition::StepStatus {
                step_id: plan_step.id().clone(),
                status: StepStatus::Completed,
            },
        );
        let transition = ProtocolTransition::new(
            state_id("drafting"),
            state_id("done"),
            trigger("finish"),
            vec![guard.name().clone()],
        )
        .unwrap();
        let role = role(vec![
            RoleAction::step(plan_step.id().clone()),
            RoleAction::transition(transition.trigger().clone()),
        ]);

        definition(
            vec![
                ProtocolState::initial(state_id("drafting")),
                ProtocolState::terminal(state_id("done")),
            ],
            vec![transition],
            vec![plan_step],
            vec![guard],
            vec![role],
        )
        .unwrap()
    }

    #[test]
    fn accepts_valid_declarative_state_machine() {
        let definition = valid_definition();

        assert_eq!(definition.initial_state_id(), &state_id("drafting"));
        assert_eq!(definition.steps_for_state(&state_id("drafting")).count(), 1);
        assert!(definition.role_allows(
            &RoleId::new("facilitator").unwrap(),
            &RoleAction::transition(trigger("finish"))
        ));
    }

    #[test]
    fn preserves_step_declaration_order_within_a_state() {
        let definition = definition(
            vec![
                ProtocolState::initial(state_id("drafting")),
                ProtocolState::terminal(state_id("done")),
            ],
            Vec::new(),
            vec![
                step("write_plan", "drafting"),
                step("challenge_plan", "drafting"),
                step("archive_plan", "drafting"),
            ],
            Vec::new(),
            Vec::new(),
        )
        .unwrap();

        let step_ids = definition
            .steps_for_state(&state_id("drafting"))
            .map(ProtocolStep::id)
            .cloned()
            .collect::<Vec<_>>();

        assert_eq!(
            step_ids,
            vec![
                step_id("write_plan"),
                step_id("challenge_plan"),
                step_id("archive_plan"),
            ]
        );
    }

    #[test]
    fn rejects_definitions_without_exactly_one_initial_state() {
        let err = definition(
            vec![
                ProtocolState::new(state_id("one"), ProtocolStateKind::Initial),
                ProtocolState::new(state_id("two"), ProtocolStateKind::Initial),
                ProtocolState::terminal(state_id("done")),
            ],
            Vec::new(),
            Vec::new(),
            Vec::new(),
            Vec::new(),
        )
        .unwrap_err();

        assert!(matches!(err, DomainError::InvariantViolated { .. }));
    }

    #[test]
    fn rejects_terminal_states_with_outgoing_transitions() {
        let transition = ProtocolTransition::new(
            state_id("done"),
            state_id("drafting"),
            trigger("restart"),
            Vec::new(),
        )
        .unwrap();

        let err = definition(
            vec![
                ProtocolState::initial(state_id("drafting")),
                ProtocolState::terminal(state_id("done")),
            ],
            vec![transition],
            Vec::new(),
            Vec::new(),
            Vec::new(),
        )
        .unwrap_err();

        assert!(matches!(err, DomainError::InvariantViolated { .. }));
    }

    #[test]
    fn rejects_roles_that_reference_unknown_steps() {
        let role = role(vec![RoleAction::step(step_id("missing"))]);

        let err = definition(
            vec![
                ProtocolState::initial(state_id("drafting")),
                ProtocolState::terminal(state_id("done")),
            ],
            Vec::new(),
            Vec::new(),
            Vec::new(),
            vec![role],
        )
        .unwrap_err();

        assert!(matches!(
            err,
            DomainError::NotFound {
                what: "protocol_role.step_action"
            }
        ));
    }

    #[test]
    fn rejects_empty_states_collection() {
        let err =
            definition(Vec::new(), Vec::new(), Vec::new(), Vec::new(), Vec::new()).unwrap_err();

        assert!(matches!(
            err,
            DomainError::EmptyCollection {
                field: "protocol_definition.states"
            }
        ));
    }

    #[test]
    fn rejects_transition_referencing_unknown_from_state() {
        let transition = ProtocolTransition::new(
            state_id("ghost"),
            state_id("done"),
            trigger("finish"),
            Vec::new(),
        )
        .unwrap();

        let err = definition(
            vec![
                ProtocolState::initial(state_id("drafting")),
                ProtocolState::terminal(state_id("done")),
            ],
            vec![transition],
            Vec::new(),
            Vec::new(),
            Vec::new(),
        )
        .unwrap_err();

        assert!(matches!(
            err,
            DomainError::NotFound {
                what: "protocol_transition.from_state"
            }
        ));
    }

    #[test]
    fn rejects_transition_referencing_unknown_to_state() {
        let transition = ProtocolTransition::new(
            state_id("drafting"),
            state_id("ghost"),
            trigger("finish"),
            Vec::new(),
        )
        .unwrap();

        let err = definition(
            vec![
                ProtocolState::initial(state_id("drafting")),
                ProtocolState::terminal(state_id("done")),
            ],
            vec![transition],
            Vec::new(),
            Vec::new(),
            Vec::new(),
        )
        .unwrap_err();

        assert!(matches!(
            err,
            DomainError::NotFound {
                what: "protocol_transition.to_state"
            }
        ));
    }

    #[test]
    fn rejects_duplicate_state_trigger_pairs() {
        let first = ProtocolTransition::new(
            state_id("drafting"),
            state_id("done"),
            trigger("finish"),
            Vec::new(),
        )
        .unwrap();
        let duplicate = ProtocolTransition::new(
            state_id("drafting"),
            state_id("done"),
            trigger("finish"),
            Vec::new(),
        )
        .unwrap();

        let err = definition(
            vec![
                ProtocolState::initial(state_id("drafting")),
                ProtocolState::terminal(state_id("done")),
            ],
            vec![first, duplicate],
            Vec::new(),
            Vec::new(),
            Vec::new(),
        )
        .unwrap_err();

        assert!(matches!(
            err,
            DomainError::AlreadyExists {
                what: "protocol_transition.state_trigger"
            }
        ));
    }

    #[test]
    fn rejects_transition_referencing_unknown_guard() {
        let transition = ProtocolTransition::new(
            state_id("drafting"),
            state_id("done"),
            trigger("finish"),
            vec![guard_name("absent")],
        )
        .unwrap();

        let err = definition(
            vec![
                ProtocolState::initial(state_id("drafting")),
                ProtocolState::terminal(state_id("done")),
            ],
            vec![transition],
            Vec::new(),
            Vec::new(),
            Vec::new(),
        )
        .unwrap_err();

        assert!(matches!(
            err,
            DomainError::NotFound {
                what: "protocol_transition.guard"
            }
        ));
    }

    #[test]
    fn rejects_step_referencing_unknown_state() {
        let err = definition(
            vec![
                ProtocolState::initial(state_id("drafting")),
                ProtocolState::terminal(state_id("done")),
            ],
            Vec::new(),
            vec![step("plan", "ghost")],
            Vec::new(),
            Vec::new(),
        )
        .unwrap_err();

        assert!(matches!(
            err,
            DomainError::NotFound {
                what: "protocol_step.state"
            }
        ));
    }

    #[test]
    fn rejects_role_that_references_unknown_transition_trigger() {
        let role = role(vec![RoleAction::transition(trigger("ghost"))]);

        let err = definition(
            vec![
                ProtocolState::initial(state_id("drafting")),
                ProtocolState::terminal(state_id("done")),
            ],
            Vec::new(),
            Vec::new(),
            Vec::new(),
            vec![role],
        )
        .unwrap_err();

        assert!(matches!(
            err,
            DomainError::NotFound {
                what: "protocol_role.transition_action"
            }
        ));
    }

    #[test]
    fn resolves_role_authorised_for_a_step() {
        let definition = valid_definition();

        assert_eq!(
            definition.role_id_for_step(&step_id("plan")).unwrap(),
            RoleId::new("facilitator").unwrap()
        );
    }

    #[test]
    fn rejects_step_with_no_authorised_role() {
        let definition = valid_definition();

        let err = definition
            .role_id_for_step(&step_id("unowned"))
            .unwrap_err();

        assert!(matches!(
            err,
            DomainError::InvariantViolated {
                reason: "no protocol role can execute step"
            }
        ));
    }

    #[test]
    fn resolves_role_authorised_for_a_transition() {
        let definition = valid_definition();

        assert_eq!(
            definition
                .role_id_for_transition(&trigger("finish"))
                .unwrap(),
            RoleId::new("facilitator").unwrap()
        );
    }

    #[test]
    fn rejects_transition_with_no_authorised_role() {
        let definition = valid_definition();

        let err = definition
            .role_id_for_transition(&trigger("unowned"))
            .unwrap_err();

        assert!(matches!(
            err,
            DomainError::InvariantViolated {
                reason: "no protocol role can apply transition"
            }
        ));
    }

    #[test]
    fn selects_guardless_transition_as_immediately_enabled() {
        let transition = ProtocolTransition::new(
            state_id("open"),
            state_id("closed"),
            trigger("go"),
            Vec::new(),
        )
        .unwrap();
        let definition = definition_with_budgets(
            vec![
                ProtocolState::initial(state_id("open")),
                ProtocolState::terminal(state_id("closed")),
            ],
            vec![transition],
            Vec::new(),
            Vec::new(),
            Vec::new(),
            None,
            Some(MaxBounces::new(1).unwrap()),
        )
        .unwrap();

        let selected = definition
            .next_satisfied_transition(
                &state_id("open"),
                &BTreeMap::new(),
                &ProtocolContext::empty(),
            )
            .expect("guardless transition is always enabled");

        assert_eq!(selected.trigger(), &trigger("go"));
    }

    #[test]
    fn skips_transition_whose_guards_are_unsatisfied() {
        let definition = valid_definition();

        assert!(definition
            .next_satisfied_transition(
                &state_id("drafting"),
                &BTreeMap::new(),
                &ProtocolContext::empty(),
            )
            .is_none());
    }

    #[test]
    fn yields_no_transition_out_of_a_terminal_state() {
        let definition = valid_definition();

        assert!(definition
            .next_satisfied_transition(
                &state_id("done"),
                &BTreeMap::new(),
                &ProtocolContext::empty(),
            )
            .is_none());
    }

    #[test]
    fn rejects_guards_that_reference_unknown_steps() {
        let guard = ProtocolGuard::new(
            guard_name("unknown_step_done"),
            GuardCondition::StepStatus {
                step_id: step_id("missing"),
                status: StepStatus::Completed,
            },
        );

        let err = definition(
            vec![
                ProtocolState::initial(state_id("drafting")),
                ProtocolState::terminal(state_id("done")),
            ],
            Vec::new(),
            Vec::new(),
            vec![guard],
            Vec::new(),
        )
        .unwrap_err();

        assert!(matches!(
            err,
            DomainError::NotFound {
                what: "protocol_guard.step"
            }
        ));
    }

    /// The digest is canonical only because `serde_json` orders object
    /// keys. Enabling `preserve_order` anywhere in the dependency graph
    /// — including through feature unification by a crate nobody here
    /// chose — would withdraw that silently and change every digest.
    /// This is what makes it loud instead.
    #[test]
    fn serde_json_emits_sorted_keys() {
        let mut out_of_order = serde_json::Map::new();
        out_of_order.insert("zulu".to_owned(), serde_json::Value::from(1));
        out_of_order.insert("alpha".to_owned(), serde_json::Value::from(2));

        assert_eq!(
            serde_json::to_string(&serde_json::Value::Object(out_of_order)).unwrap(),
            r#"{"alpha":2,"zulu":1}"#,
            "serde_json is no longer emitting sorted keys, so the definition digest is not canonical"
        );
    }

    #[test]
    fn the_same_definition_always_digests_the_same() {
        assert_eq!(
            valid_definition().digest().unwrap(),
            valid_definition().digest().unwrap()
        );
    }

    #[test]
    fn a_material_difference_changes_the_digest() {
        let baseline = valid_definition().digest().unwrap();
        let renamed = definition(
            vec![
                ProtocolState::initial(state_id("drafting")),
                ProtocolState::terminal(state_id("finished")),
            ],
            vec![ProtocolTransition::new(
                state_id("drafting"),
                state_id("finished"),
                trigger("finish"),
                Vec::new(),
            )
            .unwrap()],
            Vec::new(),
            Vec::new(),
            Vec::new(),
        )
        .unwrap()
        .digest()
        .unwrap();

        assert_ne!(baseline, renamed);
    }

    #[test]
    fn transition_order_is_material_to_the_digest() {
        // Declaration order decides which transition fires first when
        // several are enabled, so two definitions that differ only in
        // that order are different working sessions.
        let states = || {
            vec![
                ProtocolState::initial(state_id("drafting")),
                ProtocolState::terminal(state_id("done")),
                ProtocolState::terminal(state_id("cancelled")),
            ]
        };
        let finish = || {
            ProtocolTransition::new(
                state_id("drafting"),
                state_id("done"),
                trigger("finish"),
                Vec::new(),
            )
            .unwrap()
        };
        let cancel = || {
            ProtocolTransition::new(
                state_id("drafting"),
                state_id("cancelled"),
                trigger("cancel"),
                Vec::new(),
            )
            .unwrap()
        };

        let first = definition(
            states(),
            vec![finish(), cancel()],
            Vec::new(),
            Vec::new(),
            Vec::new(),
        )
        .unwrap();
        let swapped = definition(
            states(),
            vec![cancel(), finish()],
            Vec::new(),
            Vec::new(),
            Vec::new(),
        )
        .unwrap();

        assert_ne!(first.digest().unwrap(), swapped.digest().unwrap());
    }

    #[test]
    fn a_valid_definition_reports_no_findings_at_all() {
        let report = valid_definition().analyze();

        assert!(report.is_valid());
        assert!(report.findings().is_empty());
    }

    #[test]
    fn an_unreachable_state_is_warned_about_without_blocking_construction() {
        let definition = definition(
            vec![
                ProtocolState::initial(state_id("drafting")),
                ProtocolState::intermediate(state_id("orphan")),
                ProtocolState::terminal(state_id("done")),
            ],
            vec![
                ProtocolTransition::new(
                    state_id("drafting"),
                    state_id("done"),
                    trigger("finish"),
                    Vec::new(),
                )
                .unwrap(),
                ProtocolTransition::new(
                    state_id("orphan"),
                    state_id("done"),
                    trigger("rescue"),
                    Vec::new(),
                )
                .unwrap(),
            ],
            Vec::new(),
            Vec::new(),
            Vec::new(),
        )
        .unwrap();

        let report = definition.analyze();
        let warnings = report.warnings().collect::<Vec<_>>();

        assert!(report.is_valid());
        assert_eq!(warnings.len(), 1);
        assert_eq!(
            warnings[0].locus(),
            &ProtocolValidationLocus::state(state_id("orphan"))
        );
    }

    #[test]
    fn a_state_that_cannot_reach_a_terminal_is_warned_about() {
        let definition = definition_with_budgets(
            vec![
                ProtocolState::initial(state_id("drafting")),
                ProtocolState::intermediate(state_id("stuck")),
                ProtocolState::terminal(state_id("done")),
            ],
            vec![
                ProtocolTransition::new(
                    state_id("drafting"),
                    state_id("done"),
                    trigger("finish"),
                    Vec::new(),
                )
                .unwrap(),
                ProtocolTransition::new(
                    state_id("drafting"),
                    state_id("stuck"),
                    trigger("stall"),
                    Vec::new(),
                )
                .unwrap(),
                ProtocolTransition::new(
                    state_id("stuck"),
                    state_id("stuck"),
                    trigger("spin"),
                    Vec::new(),
                )
                .unwrap(),
            ],
            Vec::new(),
            Vec::new(),
            Vec::new(),
            None,
            Some(MaxBounces::new(1).unwrap()),
        )
        .unwrap();

        let report = definition.analyze();
        let warnings = report.warnings().collect::<Vec<_>>();

        assert!(warnings.iter().any(|warning| {
            warning.locus() == &ProtocolValidationLocus::state(state_id("stuck"))
                && matches!(
                    warning.defect(),
                    DomainError::InvariantViolated {
                        reason: "no terminal state is reachable from this protocol state"
                    }
                )
        }));
    }

    #[test]
    fn a_definition_without_any_terminal_state_is_warned_about() {
        let definition = definition(
            vec![ProtocolState::initial(state_id("drafting"))],
            Vec::new(),
            Vec::new(),
            Vec::new(),
            Vec::new(),
        )
        .unwrap();

        let report = definition.analyze();
        let warnings = report.warnings().collect::<Vec<_>>();

        assert_eq!(warnings.len(), 1);
        assert_eq!(warnings[0].locus(), &ProtocolValidationLocus::Definition);
    }

    #[test]
    fn structural_errors_suppress_reachability_noise() {
        let report = definition(
            vec![
                ProtocolState::initial(state_id("drafting")),
                ProtocolState::initial(state_id("also_drafting")),
                ProtocolState::terminal(state_id("done")),
            ],
            Vec::new(),
            Vec::new(),
            Vec::new(),
            Vec::new(),
        )
        .unwrap_err();

        assert!(matches!(
            report,
            DomainError::InvariantViolated {
                reason: "protocol definition must have exactly one initial state"
            }
        ));
    }
}
