//! [`CeremonyDefinitionDraft`] — a ceremony definition under authoring.
//!
//! A [`CeremonyDefinition`] is always valid: it cannot be constructed
//! otherwise. That is the right guarantee for execution and the wrong
//! one for authoring, because the definition an author most needs
//! feedback about is precisely the one that does not construct.
//!
//! The draft holds the same declarations without the invariants, so it
//! can be analysed, corrected and only then published.

use std::collections::BTreeMap;

use crate::error::DomainError;
use crate::value_objects::{
    CeremonyDescription, CeremonyGuard, CeremonyInputDefinition, CeremonyName,
    CeremonyOutputDefinition, CeremonyRole, CeremonyState, CeremonyStep, CeremonyTimeout,
    CeremonyTransition, CeremonyValidationFinding, CeremonyValidationLocus,
    CeremonyValidationReport, CeremonyVersion, MaxBounces, MaxParallel, MaxTransitions,
    StateTimeout,
};

use super::ceremony_definition_analysis::CeremonyDefinitionParts;
use super::CeremonyDefinition;

#[derive(Debug, Clone, PartialEq)]
pub struct CeremonyDefinitionDraft {
    name: CeremonyName,
    version: CeremonyVersion,
    description: Option<CeremonyDescription>,
    inputs: Vec<CeremonyInputDefinition>,
    outputs: Vec<CeremonyOutputDefinition>,
    states: Vec<CeremonyState>,
    transitions: Vec<CeremonyTransition>,
    steps: Vec<CeremonyStep>,
    guards: Vec<CeremonyGuard>,
    roles: Vec<CeremonyRole>,
    max_parallel: MaxParallel,
    max_transitions: Option<MaxTransitions>,
    max_bounces: Option<MaxBounces>,
    ceremony_timeout: Option<CeremonyTimeout>,
    state_timeout: Option<StateTimeout>,
}

impl CeremonyDefinitionDraft {
    /// Accept the declarations as given.
    ///
    /// Construction never fails: a draft exists in order to describe
    /// what is wrong with it.
    #[allow(clippy::too_many_arguments)]
    #[must_use]
    pub fn new(
        name: CeremonyName,
        version: CeremonyVersion,
        description: Option<CeremonyDescription>,
        inputs: impl IntoIterator<Item = CeremonyInputDefinition>,
        outputs: impl IntoIterator<Item = CeremonyOutputDefinition>,
        states: impl IntoIterator<Item = CeremonyState>,
        transitions: impl IntoIterator<Item = CeremonyTransition>,
        steps: impl IntoIterator<Item = CeremonyStep>,
        guards: impl IntoIterator<Item = CeremonyGuard>,
        roles: impl IntoIterator<Item = CeremonyRole>,
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
            ceremony_timeout: None,
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
    pub const fn with_ceremony_timeout(mut self, timeout: CeremonyTimeout) -> Self {
        self.ceremony_timeout = Some(timeout);
        self
    }
    #[must_use]
    pub const fn with_state_timeout(mut self, timeout: StateTimeout) -> Self {
        self.state_timeout = Some(timeout);
        self
    }
    #[must_use]
    pub const fn ceremony_timeout(&self) -> Option<CeremonyTimeout> {
        self.ceremony_timeout
    }
    #[must_use]
    pub const fn state_timeout(&self) -> Option<StateTimeout> {
        self.state_timeout
    }

    #[must_use]
    pub fn name(&self) -> &CeremonyName {
        &self.name
    }

    #[must_use]
    pub fn version(&self) -> &CeremonyVersion {
        &self.version
    }

    #[must_use]
    pub fn description(&self) -> Option<&CeremonyDescription> {
        self.description.as_ref()
    }

    #[must_use]
    pub fn inputs(&self) -> &[CeremonyInputDefinition] {
        &self.inputs
    }

    #[must_use]
    pub fn outputs(&self) -> &[CeremonyOutputDefinition] {
        &self.outputs
    }

    #[must_use]
    pub fn states(&self) -> &[CeremonyState] {
        &self.states
    }

    #[must_use]
    pub fn transitions(&self) -> &[CeremonyTransition] {
        &self.transitions
    }

    #[must_use]
    pub fn steps(&self) -> &[CeremonyStep] {
        &self.steps
    }

    #[must_use]
    pub fn guards(&self) -> &[CeremonyGuard] {
        &self.guards
    }

    #[must_use]
    pub fn roles(&self) -> &[CeremonyRole] {
        &self.roles
    }

    /// Report every defect that would prevent publication.
    ///
    /// Duplicate declarations are reported first, in the order
    /// [`CeremonyDefinition::new`] assembles them, followed by the
    /// structural analysis of the resulting state machine. Duplicates
    /// do not stop the structural pass: an author fixing a draft wants
    /// the whole picture, not the first obstacle.
    #[must_use]
    pub fn analyze(&self) -> CeremonyValidationReport {
        let mut findings = Vec::new();

        let (_, duplicate_inputs) = index(&self.inputs, CeremonyInputDefinition::name);
        push_duplicates(
            &mut findings,
            duplicate_inputs,
            "ceremony_input",
            CeremonyValidationLocus::input,
        );

        let (_, duplicate_outputs) = index(&self.outputs, CeremonyOutputDefinition::name);
        push_duplicates(
            &mut findings,
            duplicate_outputs,
            "ceremony_output",
            CeremonyValidationLocus::output,
        );

        let (states, duplicate_states) = index(&self.states, CeremonyState::id);
        push_duplicates(
            &mut findings,
            duplicate_states,
            "ceremony_state",
            CeremonyValidationLocus::state,
        );

        let (steps, duplicate_steps) = index(&self.steps, CeremonyStep::id);
        let step_order = self
            .steps
            .iter()
            .map(|step| step.id().clone())
            .collect::<Vec<_>>();
        push_duplicates(
            &mut findings,
            duplicate_steps,
            "ceremony_step",
            CeremonyValidationLocus::step,
        );

        let (guards, duplicate_guards) = index(&self.guards, CeremonyGuard::name);
        push_duplicates(
            &mut findings,
            duplicate_guards,
            "ceremony_guard",
            CeremonyValidationLocus::guard,
        );

        let (roles, duplicate_roles) = index(&self.roles, CeremonyRole::id);
        push_duplicates(
            &mut findings,
            duplicate_roles,
            "ceremony_role",
            CeremonyValidationLocus::role,
        );

        CeremonyDefinitionParts {
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

        CeremonyValidationReport::new(findings)
    }

    /// Promote the draft into an always-valid definition.
    ///
    /// Publication goes through [`CeremonyDefinition::new`] so there is
    /// exactly one place where the invariants are enforced.
    pub fn publish(self) -> Result<CeremonyDefinition, DomainError> {
        CeremonyDefinition::new_with_transition_budgets(
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
            if let Some(timeout) = self.ceremony_timeout {
                definition = definition.with_ceremony_timeout(timeout);
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
    findings: &mut Vec<CeremonyValidationFinding>,
    duplicates: Vec<K>,
    what: &'static str,
    locus: impl Fn(K) -> CeremonyValidationLocus,
) {
    for key in duplicates {
        findings.push(CeremonyValidationFinding::error(
            locus(key),
            DomainError::AlreadyExists { what },
        ));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::value_objects::{
        ContextKey, DynamicRoleBinding, GuardCondition, GuardName, RepeatUntilCondition,
        RetryPolicy, RoleAction, RoleId, StateId, StepHandlerConfig, StepHandlerKind, StepId,
        StepIteration, StepOutputField, StepRepeatExhaustedGuardCondition, StepRepeatPolicy,
        StepStatus, TransitionTrigger,
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

    fn step(raw_step_id: &str, raw_state_id: &str) -> CeremonyStep {
        CeremonyStep::new(
            step_id(raw_step_id),
            state_id(raw_state_id),
            StepHandlerKind::new("manual_review").unwrap(),
            StepHandlerConfig::empty(),
            RetryPolicy::single_attempt(),
            None,
        )
    }

    fn draft(
        states: Vec<CeremonyState>,
        transitions: Vec<CeremonyTransition>,
        steps: Vec<CeremonyStep>,
        guards: Vec<CeremonyGuard>,
        roles: Vec<CeremonyRole>,
    ) -> CeremonyDefinitionDraft {
        CeremonyDefinitionDraft::new(
            CeremonyName::new("planning_ceremony").unwrap(),
            CeremonyVersion::v1(),
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

    fn three_defect_draft() -> CeremonyDefinitionDraft {
        draft(
            vec![
                CeremonyState::initial(state_id("drafting")),
                CeremonyState::terminal(state_id("done")),
            ],
            vec![CeremonyTransition::new(
                state_id("drafting"),
                state_id("nowhere"),
                trigger("finish"),
                Vec::new(),
            )
            .unwrap()],
            Vec::new(),
            vec![CeremonyGuard::new(
                crate::value_objects::GuardName::new("plan_done").unwrap(),
                GuardCondition::StepStatus {
                    step_id: step_id("missing"),
                    status: StepStatus::Completed,
                },
            )],
            vec![CeremonyRole::new(
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
                    what: "ceremony_transition.to_state"
                },
                DomainError::NotFound {
                    what: "ceremony_guard.step"
                },
                DomainError::NotFound {
                    what: "ceremony_role.step_action"
                },
            ]
        );
    }

    fn guard_name(raw: &str) -> GuardName {
        GuardName::new(raw).unwrap()
    }

    fn transition(from: &str, to: &str, raw_trigger: &str, guards: &[&str]) -> CeremonyTransition {
        CeremonyTransition::new(
            state_id(from),
            state_id(to),
            trigger(raw_trigger),
            guards.iter().map(|raw| guard_name(raw)).collect::<Vec<_>>(),
        )
        .unwrap()
    }

    fn drafting_and_done() -> Vec<CeremonyState> {
        vec![
            CeremonyState::initial(state_id("drafting")),
            CeremonyState::terminal(state_id("done")),
        ]
    }

    fn error_defects(draft: &CeremonyDefinitionDraft) -> Vec<DomainError> {
        draft
            .analyze()
            .errors()
            .map(|finding| finding.defect().clone())
            .collect()
    }

    /// A transition out of an undeclared state, to an undeclared state,
    /// requiring an undeclared guard and an exhausted-repeat guard on a
    /// step of `drafting`.
    fn missing_source_draft() -> CeremonyDefinitionDraft {
        let repeating = step("plan", "drafting").with_repeat_policy(StepRepeatPolicy::new(
            RepeatUntilCondition::output_field_equals(
                StepOutputField::new("ready").unwrap(),
                serde_json::json!(true),
            ),
            StepIteration::new(3).unwrap(),
        ));
        draft(
            drafting_and_done(),
            vec![transition(
                "ghost",
                "nowhere",
                "finish",
                &["absent", "drained"],
            )],
            vec![repeating],
            vec![CeremonyGuard::new(
                guard_name("drained"),
                GuardCondition::StepRepeatExhausted(StepRepeatExhaustedGuardCondition::new(
                    step_id("plan"),
                )),
            )],
            Vec::new(),
        )
    }

    /// One defect on a transition must not hide another on the same
    /// transition: the author would otherwise fix one, validate again
    /// and only then learn about the next.
    fn transition_defect_drafts() -> Vec<(&'static str, CeremonyDefinitionDraft, Vec<DomainError>)>
    {
        vec![
            (
                "missing target with an undefined guard",
                draft(
                    drafting_and_done(),
                    vec![transition("drafting", "nowhere", "finish", &["absent"])],
                    Vec::new(),
                    Vec::new(),
                    Vec::new(),
                ),
                vec![
                    DomainError::NotFound {
                        what: "ceremony_transition.to_state",
                    },
                    DomainError::NotFound {
                        what: "ceremony_transition.guard",
                    },
                ],
            ),
            (
                // The source-state checks wait for the source; the
                // exhausted-repeat guard names a step in `drafting`, so
                // comparing it with an undeclared source would invent a
                // second defect.
                "missing source and target with an undefined guard",
                missing_source_draft(),
                vec![
                    DomainError::NotFound {
                        what: "ceremony_transition.from_state",
                    },
                    DomainError::NotFound {
                        what: "ceremony_transition.to_state",
                    },
                    DomainError::NotFound {
                        what: "ceremony_transition.guard",
                    },
                ],
            ),
            (
                "terminal source with an undefined guard",
                draft(
                    vec![
                        CeremonyState::initial(state_id("drafting")),
                        CeremonyState::terminal(state_id("done")),
                        CeremonyState::terminal(state_id("archived")),
                    ],
                    vec![
                        transition("drafting", "done", "finish", &[]),
                        transition("done", "archived", "archive", &["absent"]),
                    ],
                    Vec::new(),
                    Vec::new(),
                    Vec::new(),
                ),
                vec![
                    DomainError::InvariantViolated {
                        reason: "terminal ceremony states cannot have outgoing transitions",
                    },
                    DomainError::NotFound {
                        what: "ceremony_transition.guard",
                    },
                ],
            ),
            (
                // The first transition is broken, yet it still claims
                // its state and trigger: fixing its target must not be
                // what reveals that the second one repeats them.
                "duplicate state and trigger after a broken transition, with an undefined guard",
                draft(
                    drafting_and_done(),
                    vec![
                        transition("drafting", "nowhere", "finish", &[]),
                        transition("drafting", "done", "finish", &["absent"]),
                    ],
                    Vec::new(),
                    Vec::new(),
                    Vec::new(),
                ),
                vec![
                    DomainError::NotFound {
                        what: "ceremony_transition.to_state",
                    },
                    DomainError::AlreadyExists {
                        what: "ceremony_transition.state_trigger",
                    },
                    DomainError::NotFound {
                        what: "ceremony_transition.guard",
                    },
                ],
            ),
        ]
    }

    #[test]
    fn every_defect_of_one_transition_is_reported_at_once() {
        for (case, draft, expected) in transition_defect_drafts() {
            assert_eq!(error_defects(&draft), expected, "{case}");
        }
    }

    #[test]
    fn a_step_in_an_undeclared_state_still_reports_its_other_defects() {
        let draft = draft(
            drafting_and_done(),
            vec![transition("drafting", "done", "finish", &[])],
            vec![step("plan", "ghost").with_dynamic_role_binding(
                DynamicRoleBinding::new(
                    ContextKey::new("next_role").unwrap(),
                    vec![RoleId::new("reviewer").unwrap()],
                )
                .unwrap(),
            )],
            Vec::new(),
            Vec::new(),
        );

        assert_eq!(
            error_defects(&draft),
            vec![
                DomainError::NotFound {
                    what: "ceremony_step.state",
                },
                DomainError::NotFound {
                    what: "ceremony_step.dynamic_role",
                },
            ]
        );
    }

    #[test]
    fn reporting_every_transition_defect_keeps_the_publication_error() {
        for (case, draft, expected) in transition_defect_drafts() {
            let error = draft.publish().unwrap_err();

            assert_eq!(error, expected[0], "{case}");
        }
    }

    #[test]
    fn duplicate_declarations_are_reported_instead_of_aborting_the_analysis() {
        let report = draft(
            vec![
                CeremonyState::initial(state_id("drafting")),
                CeremonyState::terminal(state_id("done")),
            ],
            vec![CeremonyTransition::new(
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
                what: "ceremony_step"
            }
        );
        assert_eq!(
            errors[0].locus(),
            &CeremonyValidationLocus::step(step_id("plan"))
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
                CeremonyState::initial(state_id("drafting")),
                CeremonyState::terminal(state_id("done")),
            ],
            vec![CeremonyTransition::new(
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
