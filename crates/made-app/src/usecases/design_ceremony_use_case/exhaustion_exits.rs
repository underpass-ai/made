//! Routed exits for group repeats that run out of iterations.
//!
//! A repeating group whose last permitted iteration ends without its
//! `until` condition holding stops the ceremony in that group: no
//! iteration is left and the forward transition waits for a condition
//! that can no longer become true. A group repeat may name a terminal
//! for that outcome. The design then adds, for each such group, one
//! terminal state, one `state_repeat_exhausted` guard naming the group's
//! state and one transition from the group to that terminal, owned by
//! the role that owns the group's forward transition.
//!
//! The forward transition is untouched, so a design without the field
//! builds byte for byte the definition it built before.

use std::collections::{BTreeMap, BTreeSet};

use made_core::error::DomainError;
use made_core::value_objects::{
    CeremonyGuard, CeremonyState, CeremonyTransition, GuardCondition, GuardName, RoleAction,
    RoleId, StateId, StateRepeatExhaustedGuardCondition, StepId, TransitionTrigger,
};

use super::{CeremonyDesignDocument, COMPLETED_STATE};
use crate::usecases::{CeremonyDesignGroup, CeremonyDesignStageEntry};

/// Trigger and guard name of a group's exhaustion exit.
fn exit_name(group: &CeremonyDesignGroup) -> String {
    format!("{}_repeat_exhausted", group.id())
}

fn routed_groups(
    document: &CeremonyDesignDocument,
) -> impl Iterator<Item = (&CeremonyDesignGroup, &StepId)> {
    document
        .stage_entries()
        .iter()
        .filter_map(|entry| match entry {
            CeremonyDesignStageEntry::Group(group) => group
                .repeat()
                .and_then(|repeat| repeat.exhausted_terminal())
                .map(|terminal| (group, terminal)),
            CeremonyDesignStageEntry::Leaf(_) | CeremonyDesignStageEntry::Pattern(_) => None,
        })
}

/// Refuse an exhaustion terminal that would collide with a generated
/// state, or an exit name that would collide with a declared step.
pub(super) fn validate(
    document: &CeremonyDesignDocument,
    entry_ids: &[String],
    stage_ids: &[String],
) -> Result<(), DomainError> {
    for (group, terminal) in routed_groups(document) {
        let state = terminal.as_str().to_ascii_uppercase();
        if state == COMPLETED_STATE || entry_ids.iter().any(|id| id.eq_ignore_ascii_case(&state)) {
            return Err(DomainError::InvalidDocument {
                reason: format!(
                    "group `{}` exhaustion terminal `{terminal}` collides with a stage or the completed state",
                    group.id()
                ),
            });
        }
        let name = exit_name(group);
        if stage_ids.contains(&name) {
            return Err(DomainError::InvalidDocument {
                reason: format!(
                    "stage id `{name}` collides with the generated exhaustion exit of group `{}`",
                    group.id()
                ),
            });
        }
    }
    Ok(())
}

/// Add every routed exhaustion exit to the definition being assembled.
pub(super) fn add(
    document: &CeremonyDesignDocument,
    states: &mut Vec<CeremonyState>,
    guards: &mut Vec<CeremonyGuard>,
    transitions: &mut Vec<CeremonyTransition>,
    actions: &mut BTreeMap<RoleId, BTreeSet<RoleAction>>,
) -> Result<(), DomainError> {
    let mut terminals = BTreeSet::new();
    for (group, terminal) in routed_groups(document) {
        let source = StateId::new(group.id().as_str().to_ascii_uppercase())?;
        let destination = StateId::new(terminal.as_str().to_ascii_uppercase())?;
        if terminals.insert(destination.clone()) {
            states.push(CeremonyState::terminal(destination.clone()));
        }
        let name = exit_name(group);
        let guard = GuardName::new(&name)?;
        guards.push(CeremonyGuard::new(
            guard.clone(),
            GuardCondition::StateRepeatExhausted(StateRepeatExhaustedGuardCondition::new(
                source.clone(),
            )),
        ));
        let trigger = TransitionTrigger::new(&name)?;
        let owner = group
            .steps()
            .first()
            .expect("validated non-empty group")
            .step()
            .owner_role_id();
        actions
            .get_mut(owner)
            .expect("validated group owner")
            .insert(RoleAction::transition(trigger.clone()));
        transitions.push(CeremonyTransition::new(
            source,
            destination,
            trigger,
            vec![guard],
        )?);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use made_core::entities::CeremonyInstance;
    use made_core::value_objects::{
        Attributes, CeremonyContext, CeremonyDescription, CeremonyId, CeremonyName, IdempotencyKey,
        LeaseOwnerId, OutputName, StateExecution, StateIteration, StepInstructions, StepLease,
        StepOutput, StepOutputField, StepResult,
    };
    use serde_json::json;
    use time::{Duration, OffsetDateTime};

    use super::*;
    use crate::usecases::ceremony_design_participant::CeremonyDesignParticipant;
    use crate::usecases::{
        CeremonyDesignGroupRepeat, CeremonyDesignGroupRepeatUntil, CeremonyDesignGroupStep,
        CeremonyDesignJoin, CeremonyDesignStage, DesignCeremonyUseCase,
    };

    fn stage(id: &str, owner: &str) -> CeremonyDesignStage {
        CeremonyDesignStage::new(
            StepId::new(id).unwrap(),
            RoleId::new(owner).unwrap(),
            StepInstructions::new(format!("Do the {id} work.")).unwrap(),
            None,
            None,
            None,
            made_core::value_objects::Rounds::new(0).unwrap(),
            None,
        )
    }

    fn review(terminal: Option<&str>, after: &str) -> CeremonyDesignDocument {
        let repeat = CeremonyDesignGroupRepeat::new(
            StateIteration::new(2).unwrap(),
            CeremonyDesignGroupRepeatUntil::new(
                StepId::new("outcome").unwrap(),
                StepOutputField::new("outcome").unwrap(),
                json!("approved"),
            ),
        );
        let repeat = match terminal {
            Some(terminal) => repeat.with_exhausted_terminal(StepId::new(terminal).unwrap()),
            None => repeat,
        };
        CeremonyDesignDocument::new(
            CeremonyName::new("pr_review").unwrap(),
            None,
            CeremonyDescription::new("Decide whether the change is approved.").unwrap(),
            Vec::new(),
            Vec::new(),
            vec![OutputName::new("decision").unwrap()],
            vec![
                CeremonyDesignParticipant::new(RoleId::new("author").unwrap(), []),
                CeremonyDesignParticipant::new(RoleId::new("reviewer").unwrap(), []),
            ],
            Vec::new(),
            None,
            None,
            None,
            None,
        )
        .with_stage_entries(vec![
            CeremonyDesignStageEntry::Group(
                CeremonyDesignGroup::new(
                    StepId::new("review_cycle").unwrap(),
                    StateExecution::Sequential,
                    vec![
                        CeremonyDesignGroupStep::new(stage("verdict", "reviewer")),
                        CeremonyDesignGroupStep::new(stage("outcome", "author")),
                    ],
                    CeremonyDesignJoin::AllStepsCompleted,
                )
                .with_repeat(repeat),
            ),
            CeremonyDesignStageEntry::Leaf(stage(after, "author")),
        ])
    }

    fn refused(document: &CeremonyDesignDocument) -> String {
        DesignCeremonyUseCase::new()
            .execute(document)
            .unwrap_err()
            .to_string()
    }

    #[test]
    fn a_routed_group_repeat_gains_one_terminal_guard_and_owned_exit() {
        let draft = DesignCeremonyUseCase::new()
            .execute(&review(Some("exhausted"), "close_review"))
            .unwrap()
            .definition()
            .clone();
        assert!(
            draft.analyze().is_valid(),
            "{:?}",
            draft.analyze().findings()
        );
        assert!(draft
            .states()
            .iter()
            .any(|state| state.id().as_str() == "EXHAUSTED" && state.is_terminal()));
        let exit = draft
            .transitions()
            .iter()
            .find(|transition| transition.trigger().as_str() == "review_cycle_repeat_exhausted")
            .unwrap();
        assert_eq!(exit.from().as_str(), "REVIEW_CYCLE");
        assert_eq!(exit.to().as_str(), "EXHAUSTED");
        assert!(draft.guards().iter().any(|guard| matches!(
            guard.condition(),
            GuardCondition::StateRepeatExhausted(condition)
                if condition.state_id().as_str() == "REVIEW_CYCLE"
        )));
        assert!(draft
            .roles()
            .iter()
            .find(|role| role.id().as_str() == "reviewer")
            .unwrap()
            .allows(&RoleAction::transition(exit.trigger().clone())));
    }

    #[test]
    fn an_unrouted_group_repeat_designs_no_exit() {
        let draft = DesignCeremonyUseCase::new()
            .execute(&review(None, "close_review"))
            .unwrap()
            .definition()
            .clone();
        assert!(draft
            .states()
            .iter()
            .all(|state| state.id().as_str() != "EXHAUSTED"));
        assert!(draft
            .guards()
            .iter()
            .all(|guard| !matches!(guard.condition(), GuardCondition::StateRepeatExhausted(_))));
    }

    #[test]
    fn an_exhaustion_terminal_cannot_reuse_a_generated_state_or_exit_name() {
        assert!(refused(&review(Some("completed"), "close_review")).contains("collides"));
        assert!(refused(&review(Some("close_review"), "close_review")).contains("collides"));
        assert!(refused(&review(Some("review_cycle"), "close_review")).contains("collides"));
        assert!(
            refused(&review(Some("exhausted"), "review_cycle_repeat_exhausted"))
                .contains("generated exhaustion exit")
        );
    }

    #[test]
    fn a_designed_exit_carries_an_exhausted_instance_to_its_terminal() {
        let definition = DesignCeremonyUseCase::new()
            .execute(&review(Some("exhausted"), "close_review"))
            .unwrap()
            .definition()
            .clone()
            .publish()
            .unwrap();
        let start = OffsetDateTime::UNIX_EPOCH;
        let mut instance = CeremonyInstance::start(
            CeremonyId::new("pr-review-1").unwrap(),
            &definition,
            CeremonyContext::empty(),
            start,
        )
        .unwrap();
        for round in 1..=2 {
            for (id, output) in [
                ("verdict", StepOutput::empty()),
                (
                    "outcome",
                    StepOutput::new(
                        Attributes::new(BTreeMap::from([(
                            "outcome".to_owned(),
                            json!("changes_requested"),
                        )]))
                        .unwrap(),
                    ),
                ),
            ] {
                let step = StepId::new(id).unwrap();
                let lease = StepLease::new(
                    LeaseOwnerId::new("host").unwrap(),
                    IdempotencyKey::new(format!("{id}-{round}")).unwrap(),
                    start,
                    start + Duration::minutes(5),
                )
                .unwrap();
                instance
                    .start_step(&definition, &step, lease, start)
                    .unwrap();
                instance
                    .apply_step_result(
                        &definition,
                        &step,
                        instance.step_claim_fence(&step).unwrap(),
                        StepResult::completed(output).unwrap(),
                        start,
                    )
                    .unwrap();
            }
        }
        assert!(instance
            .apply_transition(
                &definition,
                &TransitionTrigger::new("review_cycle_completed").unwrap(),
                start,
            )
            .is_err());
        let terminal = instance
            .apply_transition(
                &definition,
                &TransitionTrigger::new("review_cycle_repeat_exhausted").unwrap(),
                start,
            )
            .unwrap();
        assert_eq!(terminal.as_str(), "EXHAUSTED");
        assert!(instance.is_terminal(&definition));
    }
}
