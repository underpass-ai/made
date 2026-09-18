use std::collections::BTreeSet;

use made_core::error::DomainError;
use made_core::value_objects::CeremonyId;

/// A bounded, distinct selection of sessions, preserving caller order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CeremonyReportIds(Vec<CeremonyId>);

impl CeremonyReportIds {
    pub const MAX_ITEMS: usize = 100;

    pub fn new(ids: Vec<CeremonyId>) -> Result<Self, DomainError> {
        const FIELD: &str = "ceremony_report.ceremony_ids";
        if ids.is_empty() {
            return Err(DomainError::EmptyCollection { field: FIELD });
        }
        if ids.len() > Self::MAX_ITEMS {
            return Err(DomainError::OutOfRange {
                field: FIELD,
                value: ids.len() as f64,
                min: 1.0,
                max: Self::MAX_ITEMS as f64,
            });
        }
        let ids = ids
            .into_iter()
            .map(|id| CeremonyId::new(id.as_str()))
            .collect::<Result<Vec<_>, _>>()?;
        let mut seen = BTreeSet::new();
        for id in &ids {
            if !seen.insert(id) {
                return Err(DomainError::InvalidDocument {
                    reason: format!("the report names the ceremony `{id}` more than once, and duplicates are refused"),
                });
            }
        }
        Ok(Self(ids))
    }

    #[must_use]
    pub fn as_slice(&self) -> &[CeremonyId] {
        &self.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_empty_oversized_and_duplicate_selections_and_preserves_order_at_the_cap() {
        assert!(CeremonyReportIds::new(Vec::new()).is_err());
        let ids = (0..=CeremonyReportIds::MAX_ITEMS)
            .rev()
            .map(|i| CeremonyId::new(format!("session-{i}")).unwrap())
            .collect::<Vec<_>>();
        assert!(CeremonyReportIds::new(ids.clone()).is_err());
        assert!(CeremonyReportIds::new(vec![ids[0].clone(), ids[0].clone()]).is_err());
        let expected = ids[1..].to_vec();
        assert_eq!(
            CeremonyReportIds::new(expected.clone()).unwrap().as_slice(),
            expected
        );
    }
}
