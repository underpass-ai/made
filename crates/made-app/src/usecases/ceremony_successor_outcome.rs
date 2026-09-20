use made_core::entities::CeremonyInstance;
use made_core::value_objects::SuccessionPlan;

/// The successor, and the handoff that opened it.
///
/// Both, because a caller that only got the instance back would have
/// to read the predecessor's stream to learn what was actually sealed
/// on its behalf — including on the retry that verified an opening it
/// had already made.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CeremonySuccessorOutcome {
    pub successor: CeremonyInstance,
    pub plan: SuccessionPlan,
}
