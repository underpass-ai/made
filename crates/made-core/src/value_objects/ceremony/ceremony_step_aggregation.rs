use serde::{Deserialize, Serialize};

use super::StepOutputField;

/// How a step consumes the completed outputs of the concurrent state
/// immediately before it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "strategy", rename_all = "snake_case")]
pub enum CeremonyStepAggregation {
    /// Give every sibling output to the step's existing handler.
    Synthesize,
    /// Complete without a handler when one value has a strict majority.
    Vote { output_field: StepOutputField },
}

impl CeremonyStepAggregation {
    #[must_use]
    pub const fn synthesize() -> Self {
        Self::Synthesize
    }

    #[must_use]
    pub const fn vote(output_field: StepOutputField) -> Self {
        Self::Vote { output_field }
    }

    #[must_use]
    pub const fn output_field(&self) -> Option<&StepOutputField> {
        match self {
            Self::Synthesize => None,
            Self::Vote { output_field } => Some(output_field),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strategies_have_one_stable_tagged_shape() {
        assert_eq!(
            serde_json::to_value(CeremonyStepAggregation::synthesize()).unwrap(),
            serde_json::json!({"strategy": "synthesize"})
        );
        let vote = CeremonyStepAggregation::vote(StepOutputField::new("choice").unwrap());
        let encoded = serde_json::to_value(&vote).unwrap();
        assert_eq!(
            encoded,
            serde_json::json!({"strategy": "vote", "output_field": "choice"})
        );
        assert_eq!(
            serde_json::from_value::<CeremonyStepAggregation>(encoded).unwrap(),
            vote
        );
    }
}
