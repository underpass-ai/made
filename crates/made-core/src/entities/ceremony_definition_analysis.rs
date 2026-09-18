//! Shared structural analysis of a ceremony state machine.
//!
//! The same checks serve a published [`CeremonyDefinition`] and a
//! [`CeremonyDefinitionDraft`] that may not be publishable at all, so
//! they operate on borrowed parts instead of on either aggregate.
//!
//! [`CeremonyDefinition`]: super::CeremonyDefinition
//! [`CeremonyDefinitionDraft`]: super::CeremonyDefinitionDraft

use std::collections::{BTreeMap, BTreeSet};

use crate::error::DomainError;
use crate::value_objects::{
    CeremonyGuard, CeremonyRole, CeremonyState, CeremonyStep, CeremonyTransition,
    CeremonyValidationFinding, CeremonyValidationLocus, GuardCondition, GuardName, MaxBounces,
    MaxTransitions, RoleAction, RoleId, StateExecution, StateId, StepId,
};

mod cycles;

use cycles::cyclic_components;

/// The assembled parts of a ceremony state machine, borrowed for
/// analysis.
pub(super) struct CeremonyDefinitionParts<'a> {
    pub(super) states: &'a BTreeMap<StateId, CeremonyState>,
    pub(super) transitions: &'a [CeremonyTransition],
    pub(super) steps: &'a BTreeMap<StepId, CeremonyStep>,
    pub(super) guards: &'a BTreeMap<GuardName, CeremonyGuard>,
    pub(super) roles: &'a BTreeMap<RoleId, CeremonyRole>,
    pub(super) max_transitions: Option<MaxTransitions>,
    pub(super) max_bounces: Option<MaxBounces>,
}

impl CeremonyDefinitionParts<'_> {
    /// Append every structural finding, in check order.
    ///
    /// Order matters: the first blocking finding must be the error
    /// fail-fast construction raises.
    pub(super) fn collect_findings(&self, findings: &mut Vec<CeremonyValidationFinding>) {
        self.collect_initial_state_findings(findings);
        self.collect_transition_graph_findings(findings);
        self.collect_cycle_findings(findings);
        self.collect_step_findings(findings);
        self.collect_state_repeat_findings(findings);
        self.collect_guard_findings(findings);
        self.collect_role_findings(findings);
        self.collect_concurrent_state_findings(findings);
        self.collect_reachability_findings(findings);
    }

    fn collect_cycle_findings(&self, findings: &mut Vec<CeremonyValidationFinding>) {
        if !self.transition_endpoints_resolve() {
            return;
        }
        let components = cyclic_components(self.states, self.transitions);
        if components.is_empty() {
            return;
        }
        if self.max_transitions.is_none() && self.max_bounces.is_none() {
            findings.push(CeremonyValidationFinding::error(
                CeremonyValidationLocus::Definition,
                DomainError::InvariantViolated {
                    reason: "cyclic ceremony definition requires max_transitions or max_bounces",
                },
            ));
        }
        if !self.transition_guards_resolve() {
            return;
        }
        for component in components {
            if self.component_has_human_control(&component) {
                continue;
            }
            let first = component
                .first()
                .expect("a cyclic component contains at least one state")
                .clone();
            findings.push(CeremonyValidationFinding::warning(
                CeremonyValidationLocus::state(first),
                DomainError::InvariantViolated {
                    reason: "cyclic ceremony component has no human-controlled state",
                },
            ));
        }
    }

    fn transition_guards_resolve(&self) -> bool {
        self.transitions.iter().all(|transition| {
            transition
                .required_guards()
                .iter()
                .all(|name| self.guards.contains_key(name))
        })
    }

    fn component_has_human_control(&self, component: &BTreeSet<StateId>) -> bool {
        self.transitions
            .iter()
            .filter(|transition| component.contains(transition.from()))
            .any(|transition| {
                transition.required_guards().iter().any(|name| {
                    self.guards.get(name).is_some_and(|guard| {
                        matches!(guard.condition(), GuardCondition::HumanApproval)
                    })
                })
            })
    }

    fn collect_state_repeat_findings(&self, findings: &mut Vec<CeremonyValidationFinding>) {
        for state in self.states.values() {
            let Some(policy) = state.repeat_policy() else {
                continue;
            };
            let step_id = policy.until().step_id();
            match self.steps.get(step_id) {
                None => findings.push(CeremonyValidationFinding::error(
                    CeremonyValidationLocus::state(state.id().clone()),
                    DomainError::NotFound {
                        what: "ceremony_state.repeat.until.step",
                    },
                )),
                Some(step) if step.state_id() != state.id() => {
                    findings.push(CeremonyValidationFinding::error(
                        CeremonyValidationLocus::state(state.id().clone()),
                        DomainError::InvariantViolated {
                            reason: "state repeat condition must reference a step in that state",
                        },
                    ));
                }
                Some(_) => {}
            }
        }
    }

    fn collect_initial_state_findings(&self, findings: &mut Vec<CeremonyValidationFinding>) {
        if self.states.is_empty() {
            findings.push(CeremonyValidationFinding::error(
                CeremonyValidationLocus::Definition,
                DomainError::EmptyCollection {
                    field: "ceremony_definition.states",
                },
            ));
            return;
        }

        let initial_count = self
            .states
            .values()
            .filter(|state| state.is_initial())
            .count();
        if initial_count != 1 {
            findings.push(CeremonyValidationFinding::error(
                CeremonyValidationLocus::Definition,
                DomainError::InvariantViolated {
                    reason: "ceremony definition must have exactly one initial state",
                },
            ));
        }
    }

    fn collect_transition_graph_findings(&self, findings: &mut Vec<CeremonyValidationFinding>) {
        let mut state_trigger_pairs = BTreeSet::new();
        for transition in self.transitions {
            let locus = CeremonyValidationLocus::transition(
                transition.from().clone(),
                transition.trigger().clone(),
            );
            let Some(from) = self.states.get(transition.from()) else {
                findings.push(CeremonyValidationFinding::error(
                    locus.clone(),
                    DomainError::NotFound {
                        what: "ceremony_transition.from_state",
                    },
                ));
                continue;
            };
            if !self.states.contains_key(transition.to()) {
                findings.push(CeremonyValidationFinding::error(
                    locus.clone(),
                    DomainError::NotFound {
                        what: "ceremony_transition.to_state",
                    },
                ));
                continue;
            }
            if from.is_terminal() {
                findings.push(CeremonyValidationFinding::error(
                    locus,
                    DomainError::InvariantViolated {
                        reason: "terminal ceremony states cannot have outgoing transitions",
                    },
                ));
                continue;
            }
            if !state_trigger_pairs
                .insert((transition.from().clone(), transition.trigger().clone()))
            {
                findings.push(CeremonyValidationFinding::error(
                    locus,
                    DomainError::AlreadyExists {
                        what: "ceremony_transition.state_trigger",
                    },
                ));
                continue;
            }
            for guard_name in transition.required_guards() {
                let Some(guard) = self.guards.get(guard_name) else {
                    findings.push(CeremonyValidationFinding::error(
                        locus.clone(),
                        DomainError::NotFound {
                            what: "ceremony_transition.guard",
                        },
                    ));
                    continue;
                };
                let GuardCondition::StepRepeatExhausted(condition) = guard.condition() else {
                    continue;
                };
                let Some(step) = self.steps.get(condition.step_id()) else {
                    continue;
                };
                if step.state_id() != transition.from() {
                    findings.push(CeremonyValidationFinding::error(
                        locus.clone(),
                        DomainError::InvariantViolated {
                            reason: "exhausted-repeat guard must reference a step in the transition source state",
                        },
                    ));
                }
            }
        }
    }

    fn collect_step_findings(&self, findings: &mut Vec<CeremonyValidationFinding>) {
        for (step_id, step) in self.steps {
            let locus = CeremonyValidationLocus::step(step_id.clone());
            let Some(state) = self.states.get(step.state_id()) else {
                findings.push(CeremonyValidationFinding::error(
                    locus,
                    DomainError::NotFound {
                        what: "ceremony_step.state",
                    },
                ));
                continue;
            };
            if state.is_terminal() {
                findings.push(CeremonyValidationFinding::error(
                    locus.clone(),
                    DomainError::InvariantViolated {
                        reason: "terminal ceremony states cannot own executable steps",
                    },
                ));
            }
            if let Some(binding) = step.dynamic_role_binding() {
                if binding.allowed_roles().is_empty() {
                    findings.push(CeremonyValidationFinding::error(
                        locus.clone(),
                        DomainError::EmptyCollection {
                            field: "dynamic_role_binding.allowed_roles",
                        },
                    ));
                }
                for role_id in binding.allowed_roles() {
                    match self.roles.get(role_id) {
                        None => findings.push(CeremonyValidationFinding::error(
                            locus.clone(),
                            DomainError::NotFound {
                                what: "ceremony_step.dynamic_role",
                            },
                        )),
                        Some(role) if !role.allows(&RoleAction::step(step_id.clone())) => {
                            findings.push(CeremonyValidationFinding::error(
                                locus.clone(),
                                DomainError::InvariantViolated {
                                    reason: "dynamic role must be authorised for the ceremony step",
                                },
                            ));
                        }
                        Some(_) => {}
                    }
                }
            }
        }
    }

    fn collect_guard_findings(&self, findings: &mut Vec<CeremonyValidationFinding>) {
        for (guard_name, guard) in self.guards {
            if let Some(step_id) = guard.condition().referenced_step_id() {
                let Some(step) = self.steps.get(step_id) else {
                    findings.push(CeremonyValidationFinding::error(
                        CeremonyValidationLocus::guard(guard_name.clone()),
                        DomainError::NotFound {
                            what: "ceremony_guard.step",
                        },
                    ));
                    continue;
                };
                if matches!(guard.condition(), GuardCondition::StepRepeatExhausted(_))
                    && step.repeat_policy().is_none()
                {
                    findings.push(CeremonyValidationFinding::error(
                        CeremonyValidationLocus::guard(guard_name.clone()),
                        DomainError::InvariantViolated {
                            reason: "exhausted-repeat guard must reference a repeating step",
                        },
                    ));
                }
            }
        }
    }

    fn collect_role_findings(&self, findings: &mut Vec<CeremonyValidationFinding>) {
        let transition_triggers = self
            .transitions
            .iter()
            .map(|transition| transition.trigger().clone())
            .collect::<BTreeSet<_>>();

        for (role_id, role) in self.roles {
            for action in role.allowed_actions() {
                if let Some(step_id) = action.step_id() {
                    if !self.steps.contains_key(step_id) {
                        findings.push(CeremonyValidationFinding::error(
                            CeremonyValidationLocus::role(role_id.clone()),
                            DomainError::NotFound {
                                what: "ceremony_role.step_action",
                            },
                        ));
                    }
                }
                if let Some(trigger) = action.transition_trigger() {
                    if !transition_triggers.contains(trigger) {
                        findings.push(CeremonyValidationFinding::error(
                            CeremonyValidationLocus::role(role_id.clone()),
                            DomainError::NotFound {
                                what: "ceremony_role.transition_action",
                            },
                        ));
                    }
                }
            }
        }
    }

    fn collect_concurrent_state_findings(&self, findings: &mut Vec<CeremonyValidationFinding>) {
        for state in self
            .states
            .values()
            .filter(|state| state.execution() == StateExecution::Concurrent)
        {
            let steps = self
                .steps
                .values()
                .filter(|step| step.state_id() == state.id())
                .collect::<Vec<_>>();
            let mut static_owners = BTreeSet::new();
            let mut possible_roles = BTreeSet::new();
            let mut duplicate_owner = false;
            for step in &steps {
                if let Some(binding) = step.dynamic_role_binding() {
                    possible_roles.extend(binding.allowed_roles().iter().cloned());
                    continue;
                }
                let owner = self
                    .roles
                    .values()
                    .find(|role| role.allows(&RoleAction::step(step.id().clone())));
                if let Some(owner) = owner {
                    possible_roles.insert(owner.id().clone());
                    duplicate_owner |= !static_owners.insert(owner.id().clone());
                }
            }
            if duplicate_owner {
                findings.push(CeremonyValidationFinding::error(
                    CeremonyValidationLocus::state(state.id().clone()),
                    DomainError::InvariantViolated {
                        reason: "concurrent ceremony steps must have distinct role owners",
                    },
                ));
            }
            if possible_roles.len() > 3 {
                findings.push(CeremonyValidationFinding::warning(
                    CeremonyValidationLocus::state(state.id().clone()),
                    DomainError::InvariantViolated {
                        reason: "concurrent ceremony state has more than three role owners",
                    },
                ));
            }
            let outgoing = self
                .transitions
                .iter()
                .filter(|transition| transition.from() == state.id())
                .collect::<Vec<_>>();
            if !outgoing
                .iter()
                .any(|transition| self.transition_has_join(transition, &steps))
            {
                findings.push(CeremonyValidationFinding::error(
                    CeremonyValidationLocus::state(state.id().clone()),
                    DomainError::InvariantViolated {
                        reason: "concurrent ceremony state requires an outgoing join guard",
                    },
                ));
            }
            for transition in outgoing {
                for guard_name in transition.required_guards() {
                    let Some(guard) = self.guards.get(guard_name) else {
                        continue;
                    };
                    if let GuardCondition::StepsCompleted(count) = guard.condition() {
                        if count.get() as usize > steps.len() {
                            findings.push(CeremonyValidationFinding::error(
                                CeremonyValidationLocus::guard(guard_name.clone()),
                                DomainError::OutOfRange {
                                    field: "join_step_count",
                                    value: f64::from(count.get()),
                                    min: 1.0,
                                    max: steps.len() as f64,
                                },
                            ));
                        }
                    }
                }
            }
        }
    }

    fn transition_has_join(
        &self,
        transition: &CeremonyTransition,
        steps: &[&CeremonyStep],
    ) -> bool {
        let required = transition
            .required_guards()
            .iter()
            .filter_map(|name| self.guards.get(name))
            .collect::<Vec<_>>();
        if required.iter().any(|guard| {
            matches!(
                guard.condition(),
                GuardCondition::AllStepsCompleted
                    | GuardCondition::AnyStepCompleted
                    | GuardCondition::StepsCompleted(_)
            )
        }) {
            return true;
        }
        steps.iter().all(|step| {
            required.iter().any(|guard| {
                matches!(
                    guard.condition(),
                    GuardCondition::StepStatus { step_id, status }
                        if step_id == step.id() && status.is_success()
                )
            })
        })
    }

    /// Reachability defects are reported as warnings.
    ///
    /// They describe a ceremony that can stall rather than one that is
    /// structurally impossible, and promoting them to errors would
    /// reject definitions that construct today. Whether any of them
    /// graduates to an error is a separate, deliberate decision.
    ///
    /// Analysis is skipped when the graph is not sound enough to walk:
    /// structural errors are reported first and reachability noise on
    /// top of them helps nobody.
    fn collect_reachability_findings(&self, findings: &mut Vec<CeremonyValidationFinding>) {
        let Some(initial) = self.sole_initial_state_id() else {
            return;
        };
        if !self.transition_endpoints_resolve() {
            return;
        }

        if !self.states.values().any(CeremonyState::is_terminal) {
            findings.push(CeremonyValidationFinding::warning(
                CeremonyValidationLocus::Definition,
                DomainError::InvariantViolated {
                    reason: "ceremony definition has no terminal state",
                },
            ));
            return;
        }

        let reachable = self.states_reachable_from(initial);
        let can_finish = self.states_that_reach_a_terminal();
        for state_id in self.states.keys() {
            if !reachable.contains(state_id) {
                findings.push(CeremonyValidationFinding::warning(
                    CeremonyValidationLocus::state(state_id.clone()),
                    DomainError::InvariantViolated {
                        reason: "ceremony state is unreachable from the initial state",
                    },
                ));
            } else if !can_finish.contains(state_id) {
                findings.push(CeremonyValidationFinding::warning(
                    CeremonyValidationLocus::state(state_id.clone()),
                    DomainError::InvariantViolated {
                        reason: "no terminal state is reachable from this ceremony state",
                    },
                ));
            }
        }
    }

    fn sole_initial_state_id(&self) -> Option<&StateId> {
        let mut initial_states = self.states.iter().filter(|(_, state)| state.is_initial());
        let (state_id, _) = initial_states.next()?;
        match initial_states.next() {
            Some(_) => None,
            None => Some(state_id),
        }
    }

    fn transition_endpoints_resolve(&self) -> bool {
        self.transitions.iter().all(|transition| {
            self.states.contains_key(transition.from()) && self.states.contains_key(transition.to())
        })
    }

    fn states_reachable_from(&self, initial: &StateId) -> BTreeSet<StateId> {
        let mut reached = BTreeSet::new();
        let mut pending = vec![initial.clone()];
        while let Some(current) = pending.pop() {
            if !reached.insert(current.clone()) {
                continue;
            }
            for transition in self
                .transitions
                .iter()
                .filter(|transition| transition.from() == &current)
            {
                pending.push(transition.to().clone());
            }
        }
        reached
    }

    fn states_that_reach_a_terminal(&self) -> BTreeSet<StateId> {
        let mut can_finish = self
            .states
            .iter()
            .filter(|(_, state)| state.is_terminal())
            .map(|(state_id, _)| state_id.clone())
            .collect::<BTreeSet<_>>();

        let mut grew = true;
        while grew {
            grew = false;
            for transition in self.transitions {
                if can_finish.contains(transition.to()) && !can_finish.contains(transition.from()) {
                    can_finish.insert(transition.from().clone());
                    grew = true;
                }
            }
        }
        can_finish
    }
}
