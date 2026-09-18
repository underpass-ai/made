use std::collections::BTreeSet;

use made_core::error::DomainError;
use made_core::value_objects::StepOutputField;

const MAX_PROJECTED_FIELDS: usize = 100;
const FIELD: &str = "ceremony_step.config.project_winner_fields";
const RESERVED: [&str; 4] = [
    "task_id",
    "winner_proposal_id",
    "winner_content",
    "candidates_total",
];

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct ProjectedWinnerFields(Vec<StepOutputField>);

impl ProjectedWinnerFields {
    pub(crate) fn new(fields: Vec<String>) -> Result<Self, DomainError> {
        if fields.len() > MAX_PROJECTED_FIELDS {
            return Err(DomainError::OutOfRange {
                field: FIELD,
                value: fields.len() as f64,
                min: 0.0,
                max: MAX_PROJECTED_FIELDS as f64,
            });
        }
        let mut seen = BTreeSet::new();
        let mut typed = Vec::with_capacity(fields.len());
        for raw in fields {
            let field = StepOutputField::new(raw)
                .map_err(|_| DomainError::InvalidCharacters { field: FIELD })?;
            if RESERVED.contains(&field.as_str()) {
                return Err(DomainError::InvalidDocument {
                    reason: format!(
                        "projected winner field `{field}` collides with handler metadata"
                    ),
                });
            }
            if !seen.insert(field.clone()) {
                return Err(DomainError::InvalidDocument {
                    reason: format!("projected winner field `{field}` is duplicated"),
                });
            }
            typed.push(field);
        }
        Ok(Self(typed))
    }

    pub(crate) fn iter(&self) -> impl Iterator<Item = &StepOutputField> {
        self.0.iter()
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validates_bounds_uniqueness_and_reserved_metadata() {
        assert!(ProjectedWinnerFields::new(vec!["approved".to_owned()]).is_ok());
        assert!(ProjectedWinnerFields::new(vec!["".to_owned()]).is_err());
        assert!(ProjectedWinnerFields::new(vec!["approved".to_owned(); 2]).is_err());
        assert!(ProjectedWinnerFields::new(vec!["winner_content".to_owned()]).is_err());
        assert!(ProjectedWinnerFields::new(
            (0..=MAX_PROJECTED_FIELDS)
                .map(|index| format!("field_{index}"))
                .collect(),
        )
        .is_err());
    }
}
