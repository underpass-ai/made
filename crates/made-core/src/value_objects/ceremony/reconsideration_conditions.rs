use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

use crate::error::DomainError;

/// Distinct reasons to ask again. Constructors validate new commands; transparent
/// deserialization retains previously accepted journal payloads without rewriting them.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ReconsiderationConditions(Vec<String>);

impl ReconsiderationConditions {
    pub const MAX_ITEMS: usize = 100;
    const MAX_CONDITION_LEN: usize = 1_024;
    const FIELD: &'static str = "ceremony_guard_deferral.reconsider_when";

    pub fn new(conditions: Vec<String>) -> Result<Self, DomainError> {
        if conditions.is_empty() {
            return Err(DomainError::EmptyCollection { field: Self::FIELD });
        }
        if conditions.len() > Self::MAX_ITEMS {
            return Err(DomainError::OutOfRange {
                field: Self::FIELD,
                value: conditions.len() as f64,
                min: 1.0,
                max: Self::MAX_ITEMS as f64,
            });
        }
        let mut seen = BTreeSet::new();
        let mut validated = Vec::with_capacity(conditions.len());
        for condition in conditions {
            let trimmed = condition.trim();
            if trimmed.is_empty() {
                return Err(DomainError::EmptyField { field: Self::FIELD });
            }
            if trimmed.len() > Self::MAX_CONDITION_LEN {
                return Err(DomainError::FieldTooLong {
                    field: Self::FIELD,
                    actual: trimmed.len(),
                    max: Self::MAX_CONDITION_LEN,
                });
            }
            if trimmed
                .chars()
                .any(|ch| ch.is_control() && !matches!(ch, '\n' | '\r' | '\t'))
            {
                return Err(DomainError::InvalidCharacters { field: Self::FIELD });
            }
            if !seen.insert(trimmed.to_owned()) {
                return Err(DomainError::InvalidDocument {
                    reason: format!("collection `{}` contains duplicate conditions", Self::FIELD),
                });
            }
            validated.push(trimmed.to_owned());
        }
        Ok(Self(validated))
    }

    #[must_use]
    pub fn as_slice(&self) -> &[String] {
        &self.0
    }

    pub(crate) fn validate(&self) -> Result<(), DomainError> {
        Self::new(self.0.clone()).map(|_| ())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_conditions_are_bounded_distinct_after_trimming_and_ordered() {
        assert!(ReconsiderationConditions::new(Vec::new()).is_err());
        let conditions = (0..=ReconsiderationConditions::MAX_ITEMS)
            .rev()
            .map(|i| format!("condition-{i}"))
            .collect::<Vec<_>>();
        assert!(ReconsiderationConditions::new(conditions.clone()).is_err());
        let expected = conditions[1..].to_vec();
        assert_eq!(
            ReconsiderationConditions::new(expected.clone())
                .unwrap()
                .as_slice(),
            expected
        );
        for invalid in [
            vec!["wait".into(), " wait ".into()],
            vec![" ".into()],
            vec!["a".repeat(1_025)],
            vec!["bad\0condition".into()],
        ] {
            assert!(ReconsiderationConditions::new(invalid).is_err());
        }
        assert_eq!(
            ReconsiderationConditions::new(vec!["  line one\nline two  ".into()])
                .unwrap()
                .as_slice(),
            ["line one\nline two"]
        );
    }

    #[test]
    fn legacy_payloads_round_trip_without_rewriting_but_cannot_author_new_commands() {
        for legacy in [
            vec!["wait".to_owned(); 101],
            vec![" wait ".into(), "wait".into()],
        ] {
            let wire = serde_json::to_value(&legacy).unwrap();
            let restored: ReconsiderationConditions = serde_json::from_value(wire.clone()).unwrap();
            assert_eq!(serde_json::to_value(&restored).unwrap(), wire);
            assert!(restored.validate().is_err());
        }
    }
}
