//! [`ProtocolInstanceView`] — what a caller needs to know about a live
//! working session.
//!
//! Derived once, here, and rendered by whoever is asking: the embedded
//! adapter turns it into JSON, the gRPC adapter into protobuf. Each
//! computing its own would make "the same working session over either
//! transport" a claim maintained by hand, and it would hold only until
//! one of them changed.
//!
//! Only the derived facts live in the view. Interventions, guard
//! deferrals and context are carried by the instance already, so the
//! view lends it out rather than copying it — a projection that
//! re-modelled half the domain would be a second domain.

use made_core::entities::{ProtocolDefinition, ProtocolInstance};
use made_core::error::DomainError;
use made_core::value_objects::{GuardCondition, GuardName, MaxParallel, StepId};
use time::OffsetDateTime;

use super::protocol_guard_view::ProtocolGuardView;
use super::protocol_step_view::ProtocolStepView;
use super::protocol_transition_view::ProtocolTransitionView;

/// The derived state of one working session.
#[derive(Debug, Clone)]
pub struct ProtocolInstanceView<'a> {
    instance: &'a ProtocolInstance,
    definition: &'a ProtocolDefinition,
    steps: Vec<ProtocolStepView<'a>>,
    transitions: Vec<ProtocolTransitionView<'a>>,
    waiting_for_human: Vec<&'a GuardName>,
    next_step_id: Option<&'a StepId>,
    claimable_step_ids: Vec<&'a StepId>,
    completed: bool,
}

impl<'a> ProtocolInstanceView<'a> {
    /// Derive the view.
    ///
    /// Fails rather than panics when a declared step has no record: a
    /// definition and an instance that do not correspond is a real
    /// inconsistency, and a projection is the wrong place to decide it
    /// cannot happen.
    pub fn project(
        instance: &'a ProtocolInstance,
        definition: &'a ProtocolDefinition,
    ) -> Result<Self, DomainError> {
        Self::project_at(
            instance,
            definition,
            OffsetDateTime::UNIX_EPOCH,
            MaxParallel::SERVER_MAX,
        )
    }

    pub fn project_at(
        instance: &'a ProtocolInstance,
        definition: &'a ProtocolDefinition,
        now: OffsetDateTime,
        host_ceiling: MaxParallel,
    ) -> Result<Self, DomainError> {
        let steps = definition
            .steps_in_declaration_order()
            .map(|step| {
                instance
                    .step_record(step.id())
                    .map(|record| ProtocolStepView::new(step, record))
                    .ok_or(DomainError::NotFound {
                        what: "protocol_step_record",
                    })
            })
            .collect::<Result<Vec<_>, _>>()?;

        let transitions = definition
            .available_transitions(instance.current_state())
            .map(|transition| {
                let guards = transition
                    .required_guards()
                    .iter()
                    .map(|name| {
                        let guard = definition.guards().get(name).ok_or(DomainError::NotFound {
                            what: "protocol_transition.guard",
                        })?;
                        Ok(ProtocolGuardView::new(
                            name,
                            matches!(guard.condition(), GuardCondition::HumanApproval),
                            instance
                                .guard_is_satisfied_for_transition(definition, transition, guard),
                        ))
                    })
                    .collect::<Result<Vec<_>, DomainError>>()?;
                Ok(ProtocolTransitionView::new(
                    transition,
                    instance.transition_is_enabled_at(definition, transition, now),
                    instance.state_repeat_permits(definition, transition)
                        && definition.repeat_requirements_are_satisfied_for_transition(
                            transition,
                            instance.step_records(),
                        )
                        && !instance.has_live_step_leases_at(definition, now),
                    guards,
                ))
            })
            .collect::<Result<Vec<_>, DomainError>>()?;

        // Only guards on transitions whose automated conditions already
        // hold. A human guard behind unfinished work is not waiting on
        // anybody yet, and reporting it would send someone to approve
        // something that cannot proceed.
        let waiting_for_human = transitions
            .iter()
            .filter(|transition| transition.waits_only_on_people())
            .flat_map(ProtocolTransitionView::guards)
            .filter(|guard| guard.is_human() && !guard.is_satisfied())
            .map(ProtocolGuardView::name)
            .collect::<Vec<_>>();

        let claimable_step_ids = instance.claimable_step_ids_at(definition, now, host_ceiling)?;
        let next_step_id = claimable_step_ids.first().copied();

        Ok(Self {
            instance,
            definition,
            steps,
            transitions,
            waiting_for_human,
            next_step_id,
            claimable_step_ids,
            completed: instance.is_completed(definition),
        })
    }

    #[must_use]
    pub fn instance(&self) -> &'a ProtocolInstance {
        self.instance
    }

    #[must_use]
    pub fn definition(&self) -> &'a ProtocolDefinition {
        self.definition
    }

    #[must_use]
    pub fn steps(&self) -> &[ProtocolStepView<'a>] {
        &self.steps
    }

    #[must_use]
    pub fn transitions(&self) -> &[ProtocolTransitionView<'a>] {
        &self.transitions
    }

    /// Guards a person must decide before this session can move.
    #[must_use]
    pub fn waiting_for_human(&self) -> &[&'a GuardName] {
        &self.waiting_for_human
    }

    /// Who sits in each seat for this session. Read straight off the
    /// instance: seating is state, not something derived.
    #[must_use]
    pub fn participant_bindings(
        &self,
    ) -> &std::collections::BTreeMap<
        made_core::value_objects::RoleId,
        made_core::value_objects::ProtocolParticipantBinding,
    > {
        self.instance.participant_bindings()
    }

    /// Every reason asserted about this session, in the order they
    /// were asserted.
    ///
    /// Read straight off the instance rather than derived: a reason is
    /// something a seat said, and a projection has nothing to add to it
    /// beyond letting it out.
    #[must_use]
    pub fn reasons(&self) -> &[made_core::value_objects::ProtocolReason] {
        self.instance.reasons()
    }

    #[must_use]
    pub fn next_step_id(&self) -> Option<&'a StepId> {
        self.next_step_id
    }

    #[must_use]
    pub fn claimable_step_ids(&self) -> &[&'a StepId] {
        &self.claimable_step_ids
    }

    #[must_use]
    pub fn is_completed(&self) -> bool {
        self.completed
    }

    /// Which protocol this one succeeds, when it was opened as a
    /// successor.
    ///
    /// Read straight off the instance, like seating and reasons: a
    /// succession is something that was sealed, not something a
    /// projection works out. Exposed on the view so both directions of
    /// the relation — "which protocol replaced this one" and "which one
    /// did this replace" — are answerable without reading raw streams.
    #[must_use]
    pub fn succession(&self) -> Option<&'a made_core::value_objects::ProtocolSuccession> {
        self.instance.succession()
    }

    /// The handoff this protocol sealed, when it sealed one.
    #[must_use]
    pub fn successor_plan(&self) -> Option<&'a made_core::value_objects::SuccessionPlan> {
        self.instance.successor_plan()
    }
}

#[cfg(test)]
mod tests;
