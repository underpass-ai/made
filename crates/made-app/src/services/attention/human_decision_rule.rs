//! The reading the rules cannot make on their own: a session has just
//! arrived somewhere only a person can take it out of.
//!
//! Separate from [`super::attention_rules`] because it is the one
//! derivation that needs more than the record. Which states a person
//! has to answer for is written in the definition, and a rule that
//! took a definition would stop being a function of the feed.
//!
//! # Why the definition and not the instance
//!
//! Whether the guard has *already* been approved is a fact about the
//! session right now, and reading it here would make the derivation
//! move under its own identity: the projection would offer
//! `human_decision_requested`, a person would answer, and the read
//! path — which derives the event again from the same record — would
//! find nothing and quietly drop the delivery it was holding. So the
//! reading is a function of the record and the definition only, and
//! the answer arrives as news of its own. A host revalidates the
//! session before acting, which is what the thin envelope is for.

use made_core::entities::{CeremonyDefinition, CeremonyEvent};
use made_core::error::DomainError;
use made_core::ports::PositionedRecord;
use made_core::value_objects::{
    AttentionEventId, AttentionKind, AttentionReason, GuardCondition, GuardName, StateId,
};

use super::{AttentionEvent, EventRef, ResultAcceptance};

/// Whether this record could possibly make the reading below.
///
/// Asked before a definition is resolved, because resolving one folds
/// a whole session and almost every record in the feed is not a
/// transition. The projection walks the global feed; paying for a fold
/// per record would make the loop's cost the deployment's traffic.
#[must_use]
pub fn moves_the_session(record: &PositionedRecord) -> bool {
    matches!(
        record.record.event(),
        Some(CeremonyEvent::TransitionApplied(_))
    )
}

/// Every human guard the state this transition entered is waiting on.
///
/// One event per guard and per visit. The visit comes for free: a
/// visit is one `TransitionApplied` record, and the identity carries
/// the record's position. The guard is spelled into the identity too,
/// so a state fenced by two different people is two pieces of news
/// rather than one that names them both and is acted on once.
pub fn human_decisions_requested(
    record: &PositionedRecord,
    definition: &CeremonyDefinition,
) -> Result<Vec<AttentionEvent>, DomainError> {
    let Some(CeremonyEvent::TransitionApplied(applied)) = record.record.event() else {
        return Ok(Vec::new());
    };
    let entered = applied.transition.to_state();
    human_guards_out_of(definition, entered)
        .into_iter()
        .map(|guard| requested(record, entered, &guard))
        .collect()
}

/// The guards a person must answer before this state can be left, in
/// the order the definition holds them.
fn human_guards_out_of(definition: &CeremonyDefinition, state: &StateId) -> Vec<GuardName> {
    let mut guards: Vec<GuardName> = definition
        .available_transitions(state)
        .flat_map(|transition| transition.required_guards().iter())
        .filter(|name| {
            definition
                .guards()
                .get(*name)
                .is_some_and(|guard| matches!(guard.condition(), GuardCondition::HumanApproval))
        })
        .cloned()
        .collect();
    guards.sort();
    guards.dedup();
    guards
}

fn requested(
    record: &PositionedRecord,
    entered: &StateId,
    guard: &GuardName,
) -> Result<AttentionEvent, DomainError> {
    let sealed = &record.record;
    // The guard is part of the identity, not only of the reason:
    // two guards on one visit are two decisions, and one identity
    // for both would be delivered once and closed once.
    let id = AttentionEventId::derive(
        sealed.ceremony_id(),
        record.position,
        &format!("{}:guard:{guard}", sealed.event_id().as_str()),
        AttentionKind::HumanDecisionRequested,
    )?;
    let source = EventRef::new(
        sealed.event_id().clone(),
        sealed.sequence(),
        sealed.record_hash(),
    );
    Ok(AttentionEvent::new(
        id,
        AttentionKind::HumanDecisionRequested,
        sealed.ceremony_id().clone(),
        source,
        record.position,
        sealed.occurred_at(),
        AttentionReason::new(format!(
            "state {entered} needs a person to answer the human guard {guard}; \
             tell them, and do not answer it yourself"
        ))?,
        ResultAcceptance::NotApplicable,
    )
    .caused_by(
        sealed.correlation_id().cloned(),
        sealed.causation_id().cloned(),
    ))
}

#[cfg(test)]
mod tests;
