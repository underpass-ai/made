use crate::value_objects::CritiqueFeedback;

/// Free-form feedback targeting a peer proposal.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Critique {
    pub feedback: CritiqueFeedback,
}
