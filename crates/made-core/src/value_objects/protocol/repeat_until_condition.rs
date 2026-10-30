use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::{StepOutput, StepOutputField};

/// A deterministic condition that ends semantic step repetition.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum RepeatUntilCondition {
    /// Stop when one top-level structured output field equals the declared
    /// JSON value exactly. Missing fields do not satisfy the condition.
    OutputFieldEquals {
        field: StepOutputField,
        expected: Value,
    },
}

impl RepeatUntilCondition {
    #[must_use]
    pub fn output_field_equals(field: StepOutputField, expected: Value) -> Self {
        Self::OutputFieldEquals { field, expected }
    }

    #[must_use]
    pub fn is_satisfied(&self, output: &StepOutput) -> bool {
        match self {
            Self::OutputFieldEquals { field, expected } => {
                output.attributes().get(field.as_str()) == Some(expected)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use serde_json::json;

    use super::*;
    use crate::value_objects::Attributes;

    fn output(entries: impl IntoIterator<Item = (&'static str, Value)>) -> StepOutput {
        StepOutput::new(
            Attributes::new(
                entries
                    .into_iter()
                    .map(|(key, value)| (key.to_owned(), value))
                    .collect::<BTreeMap<_, _>>(),
            )
            .unwrap(),
        )
    }

    #[test]
    fn equality_is_structured_and_missing_is_false() {
        let condition = RepeatUntilCondition::output_field_equals(
            StepOutputField::new("ready").unwrap(),
            json!(true),
        );

        assert!(condition.is_satisfied(&output([("ready", json!(true))])));
        assert!(!condition.is_satisfied(&output([("ready", json!("true"))])));
        assert!(!condition.is_satisfied(&StepOutput::empty()));
    }
}
