//! Shared structural analysis of a protocol state machine.
//!
//! The same checks serve a published [`ProtocolDefinition`] and a
//! [`ProtocolDefinitionDraft`] that may not be publishable at all, so
//! they operate on borrowed parts instead of on either aggregate.
//!
//! [`ProtocolDefinition`]: super::ProtocolDefinition
//! [`ProtocolDefinitionDraft`]: super::ProtocolDefinitionDraft

use std::collections::{BTreeMap, BTreeSet};

use crate::error::DomainError;
use crate::value_objects::{
    GuardCondition, GuardName, MaxBounces, MaxTransitions, ProtocolGuard, ProtocolRole,
    ProtocolState, ProtocolStep, ProtocolTransition, ProtocolValidationFinding,
    ProtocolValidationLocus, RoleAction, RoleId, StateExecution, StateId, StepId,
};

mod aggregation;
mod cycles;
mod global_completion;
mod reachability;
mod state_repeat_exhaustion;

use cycles::cyclic_components;

/// The assembled parts of a protocol state machine, borrowed for
/// analysis.
pub(super) struct ProtocolDefinitionParts<'a> {
    pub(super) states: &'a BTreeMap<StateId, ProtocolState>,
    pub(super) transitions: &'a [ProtocolTransition],
    pub(super) steps: &'a BTreeMap<StepId, ProtocolStep>,
    pub(super) step_order: &'a [StepId],
    pub(super) guards: &'a BTreeMap<GuardName, ProtocolGuard>,
    pub(super) roles: &'a BTreeMap<RoleId, ProtocolRole>,
    pub(super) max_transitions: Option<MaxTransitions>,
    pub(super) max_bounces: Option<MaxBounces>,
}

impl ProtocolDefinitionParts<'_> {
    /// Append every structural finding, in check order.
    ///
    /// Order matters: the first blocking finding must be the error
    /// fail-fast construction raises.
    pub(super) fn collect_findings(&self, findings: &mut Vec<ProtocolValidationFinding>) {
        self.collect_initial_state_findings(findings);
        self.collect_transition_graph_findings(findings);
        self.collect_cycle_findings(findings);
        self.collect_step_findings(findings);
        aggregation::collect(self, findings);
        self.collect_state_repeat_findings(findings);
        self.collect_guard_findings(findings);
        state_repeat_exhaustion::collect(self, findings);
        self.collect_role_findings(findings);
        self.collect_concurrent_state_findings(findings);
        self.collect_reachability_findings(findings);
        self.collect_global_completion_findings(findings);
    }

    fn collect_cycle_findings(&self, findings: &mut Vec<ProtocolValidationFinding>) {
        if !self.transition_endpoints_resolve() {
            return;
        }
        let components = cyclic_components(self.states, self.transitions);
        if components.is_empty() {
            return;
        }
        if self.max_transitions.is_none() && self.max_bounces.is_none() {
            findings.push(ProtocolValidationFinding::error(
                ProtocolValidationLocus::Definition,
                DomainError::InvariantViolated {
                    reason: "cyclic protocol definition requires max_transitions or max_bounces",
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
            findings.push(ProtocolValidationFinding::warning(
                ProtocolValidationLocus::state(first),
                DomainError::InvariantViolated {
                    reason: "cyclic protocol component has no human-controlled state",
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

    fn collect_state_repeat_findings(&self, findings: &mut Vec<ProtocolValidationFinding>) {
        for state in self.states.values() {
            let Some(policy) = state.repeat_policy() else {
                continue;
            };
            let step_id = policy.until().step_id();
            match self.steps.get(step_id) {
                None => findings.push(ProtocolValidationFinding::error(
                    ProtocolValidationLocus::state(state.id().clone()),
                    DomainError::NotFound {
                        what: "protocol_state.repeat.until.step",
                    },
                )),
                Some(step) if step.state_id() != state.id() => {
                    findings.push(ProtocolValidationFinding::error(
                        ProtocolValidationLocus::state(state.id().clone()),
                        DomainError::InvariantViolated {
                            reason: "state repeat condition must reference a step in that state",
                        },
                    ));
                }
                Some(_) => {}
            }
        }
    }

    fn collect_initial_state_findings(&self, findings: &mut Vec<ProtocolValidationFinding>) {
        if self.states.is_empty() {
            findings.push(ProtocolValidationFinding::error(
                ProtocolValidationLocus::Definition,
                DomainError::EmptyCollection {
                    field: "protocol_definition.states",
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
            findings.push(ProtocolValidationFinding::error(
                ProtocolValidationLocus::Definition,
                DomainError::InvariantViolated {
                    reason: "protocol definition must have exactly one initial state",
                },
            ));
        }
    }

    fn collect_transition_graph_findings(&self, findings: &mut Vec<ProtocolValidationFinding>) {
        let mut state_trigger_pairs = BTreeSet::new();
        for transition in self.transitions {
            let locus = ProtocolValidationLocus::transition(
                transition.from().clone(),
                transition.trigger().clone(),
            );
            let Some(from) = self.states.get(transition.from()) else {
                findings.push(ProtocolValidationFinding::error(
                    locus.clone(),
                    DomainError::NotFound {
                        what: "protocol_transition.from_state",
                    },
                ));
                continue;
            };
            if !self.states.contains_key(transition.to()) {
                findings.push(ProtocolValidationFinding::error(
                    locus.clone(),
                    DomainError::NotFound {
                        what: "protocol_transition.to_state",
                    },
                ));
                continue;
            }
            if from.is_terminal() {
                findings.push(ProtocolValidationFinding::error(
                    locus,
                    DomainError::InvariantViolated {
                        reason: "terminal protocol states cannot have outgoing transitions",
                    },
                ));
                continue;
            }
            if !state_trigger_pairs
                .insert((transition.from().clone(), transition.trigger().clone()))
            {
                findings.push(ProtocolValidationFinding::error(
                    locus,
                    DomainError::AlreadyExists {
                        what: "protocol_transition.state_trigger",
                    },
                ));
                continue;
            }
            for guard_name in transition.required_guards() {
                let Some(guard) = self.guards.get(guard_name) else {
                    findings.push(ProtocolValidationFinding::error(
                        locus.clone(),
                        DomainError::NotFound {
                            what: "protocol_transition.guard",
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
                    findings.push(ProtocolValidationFinding::error(
                        locus.clone(),
                        DomainError::InvariantViolated {
                            reason: "exhausted-repeat guard must reference a step in the transition source state",
                        },
                    ));
                }
            }
        }
    }

    fn collect_step_findings(&self, findings: &mut Vec<ProtocolValidationFinding>) {
        for (step_id, step) in self.steps {
            let locus = ProtocolValidationLocus::step(step_id.clone());
            let Some(state) = self.states.get(step.state_id()) else {
                findings.push(ProtocolValidationFinding::error(
                    locus,
                    DomainError::NotFound {
                        what: "protocol_step.state",
                    },
                ));
                continue;
            };
            if state.is_terminal() {
                findings.push(ProtocolValidationFinding::error(
                    locus.clone(),
                    DomainError::InvariantViolated {
                        reason: "terminal protocol states cannot own executable steps",
                    },
                ));
            }
            if step.spawn().is_some() && step.repeat_policy().is_some() {
                findings.push(ProtocolValidationFinding::error(
                    locus.clone(),
                    DomainError::InvariantViolated {
                        reason: "child-spawning protocol steps cannot repeat",
                    },
                ));
            }
            if step.spawn().is_some() && step.aggregation().is_some() {
                findings.push(ProtocolValidationFinding::error(
                    locus.clone(),
                    DomainError::InvariantViolated {
                        reason: "a protocol step cannot both spawn children and aggregate a prior state",
                    },
                ));
            }
            if let Some(binding) = step.dynamic_role_binding() {
                if binding.allowed_roles().is_empty() {
                    findings.push(ProtocolValidationFinding::error(
                        locus.clone(),
                        DomainError::EmptyCollection {
                            field: "dynamic_role_binding.allowed_roles",
                        },
                    ));
                }
                for role_id in binding.allowed_roles() {
                    match self.roles.get(role_id) {
                        None => findings.push(ProtocolValidationFinding::error(
                            locus.clone(),
                            DomainError::NotFound {
                                what: "protocol_step.dynamic_role",
                            },
                        )),
                        Some(role) if !role.allows(&RoleAction::step(step_id.clone())) => {
                            findings.push(ProtocolValidationFinding::error(
                                locus.clone(),
                                DomainError::InvariantViolated {
                                    reason: "dynamic role must be authorised for the protocol step",
                                },
                            ));
                        }
                        Some(_) => {}
                    }
                }
            }
        }
    }

    fn collect_guard_findings(&self, findings: &mut Vec<ProtocolValidationFinding>) {
        for (guard_name, guard) in self.guards {
            if let Some(step_id) = guard.condition().referenced_step_id() {
                let Some(step) = self.steps.get(step_id) else {
                    findings.push(ProtocolValidationFinding::error(
                        ProtocolValidationLocus::guard(guard_name.clone()),
                        DomainError::NotFound {
                            what: "protocol_guard.step",
                        },
                    ));
                    continue;
                };
                if matches!(guard.condition(), GuardCondition::StepRepeatExhausted(_))
                    && step.repeat_policy().is_none()
                {
                    findings.push(ProtocolValidationFinding::error(
                        ProtocolValidationLocus::guard(guard_name.clone()),
                        DomainError::InvariantViolated {
                            reason: "exhausted-repeat guard must reference a repeating step",
                        },
                    ));
                }
                if let GuardCondition::ChildrenCompleted(condition) = guard.condition() {
                    let Some(spawn) = step.spawn() else {
                        findings.push(ProtocolValidationFinding::error(
                            ProtocolValidationLocus::guard(guard_name.clone()),
                            DomainError::InvariantViolated {
                                reason:
                                    "children_completed guard must reference a child-spawning step",
                            },
                        ));
                        continue;
                    };
                    if let crate::value_objects::ChildJoin::Quorum { count } = condition.join() {
                        if usize::from(count.get()) > spawn.children().len() {
                            findings.push(ProtocolValidationFinding::error(
                                ProtocolValidationLocus::guard(guard_name.clone()),
                                DomainError::OutOfRange {
                                    field: "child_quorum",
                                    value: f64::from(count.get()),
                                    min: 1.0,
                                    max: spawn.children().len() as f64,
                                },
                            ));
                        }
                    }
                }
            }
        }
    }

    fn collect_role_findings(&self, findings: &mut Vec<ProtocolValidationFinding>) {
        let transition_triggers = self
            .transitions
            .iter()
            .map(|transition| transition.trigger().clone())
            .collect::<BTreeSet<_>>();

        for (role_id, role) in self.roles {
            for action in role.allowed_actions() {
                if let Some(step_id) = action.step_id() {
                    if !self.steps.contains_key(step_id) {
                        findings.push(ProtocolValidationFinding::error(
                            ProtocolValidationLocus::role(role_id.clone()),
                            DomainError::NotFound {
                                what: "protocol_role.step_action",
                            },
                        ));
                    }
                }
                if let Some(trigger) = action.transition_trigger() {
                    if !transition_triggers.contains(trigger) {
                        findings.push(ProtocolValidationFinding::error(
                            ProtocolValidationLocus::role(role_id.clone()),
                            DomainError::NotFound {
                                what: "protocol_role.transition_action",
                            },
                        ));
                    }
                }
            }
        }
    }

    fn collect_concurrent_state_findings(&self, findings: &mut Vec<ProtocolValidationFinding>) {
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
                findings.push(ProtocolValidationFinding::error(
                    ProtocolValidationLocus::state(state.id().clone()),
                    DomainError::InvariantViolated {
                        reason: "concurrent protocol steps must have distinct role owners",
                    },
                ));
            }
            if possible_roles.len() > 3 {
                findings.push(ProtocolValidationFinding::warning(
                    ProtocolValidationLocus::state(state.id().clone()),
                    DomainError::InvariantViolated {
                        reason: "concurrent protocol state has more than three role owners",
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
                findings.push(ProtocolValidationFinding::error(
                    ProtocolValidationLocus::state(state.id().clone()),
                    DomainError::InvariantViolated {
                        reason: "concurrent protocol state requires an outgoing join guard",
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
                            findings.push(ProtocolValidationFinding::error(
                                ProtocolValidationLocus::guard(guard_name.clone()),
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
        transition: &ProtocolTransition,
        steps: &[&ProtocolStep],
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
}
