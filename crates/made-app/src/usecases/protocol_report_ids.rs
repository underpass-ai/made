use std::collections::BTreeSet;

use made_core::error::DomainError;
use made_core::value_objects::ProtocolId;

/// A bounded, distinct selection of sessions, preserving caller order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProtocolReportIds(Vec<ProtocolId>);

impl ProtocolReportIds {
    pub const MAX_ITEMS: usize = 100;

    pub fn new(ids: Vec<ProtocolId>) -> Result<Self, DomainError> {
        const FIELD: &str = "protocol_report.protocol_ids";
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
            .map(|id| ProtocolId::new(id.as_str()))
            .collect::<Result<Vec<_>, _>>()?;
        let mut seen = BTreeSet::new();
        for id in &ids {
            if !seen.insert(id) {
                return Err(DomainError::InvalidDocument {
                    reason: format!("the report names the protocol `{id}` more than once, and duplicates are refused"),
                });
            }
        }
        Ok(Self(ids))
    }

    #[must_use]
    pub fn as_slice(&self) -> &[ProtocolId] {
        &self.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_empty_oversized_and_duplicate_selections_and_preserves_order_at_the_cap() {
        assert!(ProtocolReportIds::new(Vec::new()).is_err());
        let ids = (0..=ProtocolReportIds::MAX_ITEMS)
            .rev()
            .map(|i| ProtocolId::new(format!("session-{i}")).unwrap())
            .collect::<Vec<_>>();
        assert!(ProtocolReportIds::new(ids.clone()).is_err());
        assert!(ProtocolReportIds::new(vec![ids[0].clone(), ids[0].clone()]).is_err());
        let expected = ids[1..].to_vec();
        assert_eq!(
            ProtocolReportIds::new(expected.clone()).unwrap().as_slice(),
            expected
        );
    }
}
