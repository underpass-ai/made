//! [`ProtocolDefinitionDraft`] — a protocol definition under authoring.
//!
//! A [`ProtocolDefinition`] is always valid: it cannot be constructed
//! otherwise. That is the right guarantee for execution and the wrong
//! one for authoring, because the definition an author most needs
//! feedback about is precisely the one that does not construct.
//!
//! The draft holds the same declarations without the invariants, so it
//! can be analysed, corrected and only then published.

use std::collections::BTreeMap;

use crate::error::DomainError;
use crate::value_objects::{
    MaxBounces, MaxParallel, MaxTransitions, ProtocolDescription, ProtocolGuard,
    ProtocolInputDefinition, ProtocolName, ProtocolOutputDefinition, ProtocolRole, ProtocolState,
    ProtocolStep, ProtocolTimeout, ProtocolTransition, ProtocolValidationFinding,
    ProtocolValidationLocus, ProtocolValidationReport, ProtocolVersion, StateTimeout,
};

use super::protocol_definition_analysis::ProtocolDefinitionParts;
use super::ProtocolDefinition;

#[derive(Debug, Clone, PartialEq)]
pub struct ProtocolDefinitionDraft {
    name: ProtocolName,
    version: ProtocolVersion,
    description: Option<ProtocolDescription>,
    inputs: Vec<ProtocolInputDefinition>,
    outputs: Vec<ProtocolOutputDefinition>,
    states: Vec<ProtocolState>,
    transitions: Vec<ProtocolTransition>,
    steps: Vec<ProtocolStep>,
    guards: Vec<ProtocolGuard>,
    roles: Vec<ProtocolRole>,
    max_parallel: MaxParallel,
    max_transitions: Option<MaxTransitions>,
    max_bounces: Option<MaxBounces>,
    protocol_timeout: Option<ProtocolTimeout>,
    state_timeout: Option<StateTimeout>,
}

impl ProtocolDefinitionDraft {
    /// Accept the declarations as given.
    ///
    /// Construction never fails: a draft exists in order to describe
    /// what is wrong with it.
    #[allow(clippy::too_many_arguments)]
    #[must_use]
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
    ) -> Self {
        Self {
            name,
            version,
            description,
            inputs: inputs.into_iter().collect(),
            outputs: outputs.into_iter().collect(),
            states: states.into_iter().collect(),
            transitions: transitions.into_iter().collect(),
            steps: steps.into_iter().collect(),
            guards: guards.into_iter().collect(),
            roles: roles.into_iter().collect(),
            max_parallel: MaxParallel::default(),
            max_transitions: None,
            max_bounces: None,
            protocol_timeout: None,
            state_timeout: None,
        }
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
    pub const fn with_max_transitions(mut self, max_transitions: MaxTransitions) -> Self {
        self.max_transitions = Some(max_transitions);
        self
    }

    #[must_use]
    pub const fn with_max_bounces(mut self, max_bounces: MaxBounces) -> Self {
        self.max_bounces = Some(max_bounces);
        self
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
    pub fn inputs(&self) -> &[ProtocolInputDefinition] {
        &self.inputs
    }

    #[must_use]
    pub fn outputs(&self) -> &[ProtocolOutputDefinition] {
        &self.outputs
    }

    #[must_use]
    pub fn states(&self) -> &[ProtocolState] {
        &self.states
    }

    #[must_use]
    pub fn transitions(&self) -> &[ProtocolTransition] {
        &self.transitions
    }

    #[must_use]
    pub fn steps(&self) -> &[ProtocolStep] {
        &self.steps
    }

    #[must_use]
    pub fn guards(&self) -> &[ProtocolGuard] {
        &self.guards
    }

    #[must_use]
    pub fn roles(&self) -> &[ProtocolRole] {
        &self.roles
    }

    /// Report every defect that would prevent publication.
    ///
    /// Duplicate declarations are reported first, in the order
    /// [`ProtocolDefinition::new`] assembles them, followed by the
    /// structural analysis of the resulting state machine. Duplicates
    /// do not stop the structural pass: an author fixing a draft wants
    /// the whole picture, not the first obstacle.
    #[must_use]
    pub fn analyze(&self) -> ProtocolValidationReport {
        let mut findings = Vec::new();

        let (_, duplicate_inputs) = index(&self.inputs, ProtocolInputDefinition::name);
        push_duplicates(
            &mut findings,
            duplicate_inputs,
            "protocol_input",
            ProtocolValidationLocus::input,
        );

        let (_, duplicate_outputs) = index(&self.outputs, ProtocolOutputDefinition::name);
        push_duplicates(
            &mut findings,
            duplicate_outputs,
            "protocol_output",
            ProtocolValidationLocus::output,
        );

        let (states, duplicate_states) = index(&self.states, ProtocolState::id);
        push_duplicates(
            &mut findings,
            duplicate_states,
            "protocol_state",
            ProtocolValidationLocus::state,
        );

        let (steps, duplicate_steps) = index(&self.steps, ProtocolStep::id);
        let step_order = self
            .steps
            .iter()
            .map(|step| step.id().clone())
            .collect::<Vec<_>>();
        push_duplicates(
            &mut findings,
            duplicate_steps,
            "protocol_step",
            ProtocolValidationLocus::step,
        );

        let (guards, duplicate_guards) = index(&self.guards, ProtocolGuard::name);
        push_duplicates(
            &mut findings,
            duplicate_guards,
            "protocol_guard",
            ProtocolValidationLocus::guard,
        );

        let (roles, duplicate_roles) = index(&self.roles, ProtocolRole::id);
        push_duplicates(
            &mut findings,
            duplicate_roles,
            "protocol_role",
            ProtocolValidationLocus::role,
        );

        ProtocolDefinitionParts {
            states: &states,
            transitions: &self.transitions,
            steps: &steps,
            step_order: &step_order,
            guards: &guards,
            roles: &roles,
            max_transitions: self.max_transitions,
            max_bounces: self.max_bounces,
        }
        .collect_findings(&mut findings);

        ProtocolValidationReport::new(findings)
    }

    /// Promote the draft into an always-valid definition.
    ///
    /// Publication goes through [`ProtocolDefinition::new`] so there is
    /// exactly one place where the invariants are enforced.
    pub fn publish(self) -> Result<ProtocolDefinition, DomainError> {
        ProtocolDefinition::new_with_transition_budgets(
            self.name,
            self.version,
            self.description,
            self.inputs,
            self.outputs,
            self.states,
            self.transitions,
            self.steps,
            self.guards,
            self.roles,
            self.max_transitions,
            self.max_bounces,
        )
        .map(|definition| {
            let mut definition = definition.with_max_parallel(self.max_parallel);
            if let Some(timeout) = self.protocol_timeout {
                definition = definition.with_protocol_timeout(timeout);
            }
            if let Some(timeout) = self.state_timeout {
                definition = definition.with_state_timeout(timeout);
            }
            definition
        })
    }
}

/// Index declarations by identity, keeping the first occurrence and
/// reporting every later one as a duplicate.
fn index<'a, T, K>(items: &'a [T], key: impl Fn(&'a T) -> &'a K) -> (BTreeMap<K, T>, Vec<K>)
where
    T: Clone,
    K: Clone + Ord + 'a,
{
    let mut indexed = BTreeMap::new();
    let mut duplicates = Vec::new();
    for item in items {
        let item_key = key(item).clone();
        if indexed.contains_key(&item_key) {
            duplicates.push(item_key);
            continue;
        }
        indexed.insert(item_key, item.clone());
    }
    (indexed, duplicates)
}

fn push_duplicates<K>(
    findings: &mut Vec<ProtocolValidationFinding>,
    duplicates: Vec<K>,
    what: &'static str,
    locus: impl Fn(K) -> ProtocolValidationLocus,
) {
    for key in duplicates {
        findings.push(ProtocolValidationFinding::error(
            locus(key),
            DomainError::AlreadyExists { what },
        ));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::value_objects::{
        GuardCondition, RetryPolicy, RoleAction, RoleId, StateId, StepHandlerConfig,
        StepHandlerKind, StepId, StepStatus, TransitionTrigger,
    };

    fn state_id(raw: &str) -> StateId {
        StateId::new(raw).unwrap()
    }

    fn step_id(raw: &str) -> StepId {
        StepId::new(raw).unwrap()
    }

    fn trigger(raw: &str) -> TransitionTrigger {
        TransitionTrigger::new(raw).unwrap()
    }

    fn step(raw_step_id: &str, raw_state_id: &str) -> ProtocolStep {
        ProtocolStep::new(
            step_id(raw_step_id),
            state_id(raw_state_id),
            StepHandlerKind::new("manual_review").unwrap(),
            StepHandlerConfig::empty(),
            RetryPolicy::single_attempt(),
            None,
        )
    }

    fn draft(
        states: Vec<ProtocolState>,
        transitions: Vec<ProtocolTransition>,
        steps: Vec<ProtocolStep>,
        guards: Vec<ProtocolGuard>,
        roles: Vec<ProtocolRole>,
    ) -> ProtocolDefinitionDraft {
        ProtocolDefinitionDraft::new(
            ProtocolName::new("planning_protocol").unwrap(),
            ProtocolVersion::v1(),
            None,
            Vec::new(),
            Vec::new(),
            states,
            transitions,
            steps,
            guards,
            roles,
        )
    }

    fn three_defect_draft() -> ProtocolDefinitionDraft {
        draft(
            vec![
                ProtocolState::initial(state_id("drafting")),
                ProtocolState::terminal(state_id("done")),
            ],
            vec![ProtocolTransition::new(
                state_id("drafting"),
                state_id("nowhere"),
                trigger("finish"),
                Vec::new(),
            )
            .unwrap()],
            Vec::new(),
            vec![ProtocolGuard::new(
                crate::value_objects::GuardName::new("plan_done").unwrap(),
                GuardCondition::StepStatus {
                    step_id: step_id("missing"),
                    status: StepStatus::Completed,
                },
            )],
            vec![ProtocolRole::new(
                RoleId::new("facilitator").unwrap(),
                vec![RoleAction::step(step_id("missing"))],
            )
            .unwrap()],
        )
    }

    #[test]
    fn a_draft_reports_every_defect_at_once() {
        let report = three_defect_draft().analyze();
        let errors = report.errors().collect::<Vec<_>>();

        assert!(!report.is_valid());
        assert_eq!(errors.len(), 3, "found: {errors:?}");
        assert_eq!(
            errors
                .iter()
                .map(|finding| finding.defect().clone())
                .collect::<Vec<_>>(),
            vec![
                DomainError::NotFound {
                    what: "protocol_transition.to_state"
                },
                DomainError::NotFound {
                    what: "protocol_guard.step"
                },
                DomainError::NotFound {
                    what: "protocol_role.step_action"
                },
            ]
        );
    }

    #[test]
    fn duplicate_declarations_are_reported_instead_of_aborting_the_analysis() {
        let report = draft(
            vec![
                ProtocolState::initial(state_id("drafting")),
                ProtocolState::terminal(state_id("done")),
            ],
            vec![ProtocolTransition::new(
                state_id("drafting"),
                state_id("done"),
                trigger("finish"),
                Vec::new(),
            )
            .unwrap()],
            vec![step("plan", "drafting"), step("plan", "drafting")],
            Vec::new(),
            Vec::new(),
        )
        .analyze();
        let errors = report.errors().collect::<Vec<_>>();

        assert_eq!(errors.len(), 1);
        assert_eq!(
            errors[0].defect(),
            &DomainError::AlreadyExists {
                what: "protocol_step"
            }
        );
        assert_eq!(
            errors[0].locus(),
            &ProtocolValidationLocus::step(step_id("plan"))
        );
    }

    #[test]
    fn publishing_fails_with_exactly_the_first_blocking_finding() {
        let draft = three_defect_draft();
        let expected = draft
            .analyze()
            .first_error()
            .expect("a blocking finding")
            .defect()
            .clone();

        let error = draft.publish().unwrap_err();

        assert_eq!(error, expected);
    }

    #[test]
    fn a_clean_draft_publishes() {
        let draft = draft(
            vec![
                ProtocolState::initial(state_id("drafting")),
                ProtocolState::terminal(state_id("done")),
            ],
            vec![ProtocolTransition::new(
                state_id("drafting"),
                state_id("done"),
                trigger("finish"),
                Vec::new(),
            )
            .unwrap()],
            vec![step("plan", "drafting")],
            Vec::new(),
            Vec::new(),
        );

        assert!(draft.analyze().is_valid());

        let definition = draft.publish().expect("a clean draft must publish");

        assert_eq!(definition.initial_state_id(), &state_id("drafting"));
        assert!(definition.analyze().findings().is_empty());
    }
}
