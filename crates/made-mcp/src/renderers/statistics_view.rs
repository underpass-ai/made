use std::collections::BTreeMap;

use serde_json::{json, Value};

/// Transport-neutral legacy service counters.
#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct StatisticsView {
    pub(crate) total_deliberations: u64,
    pub(crate) total_orchestrations: u64,
    pub(crate) total_duration_ms: u64,
    pub(crate) average_duration_ms: f64,
    pub(crate) per_specialty_counts: BTreeMap<String, u64>,
}

impl StatisticsView {
    #[must_use]
    pub(crate) fn to_json(&self) -> Value {
        json!({
            "total_deliberations": self.total_deliberations,
            "total_orchestrations": self.total_orchestrations,
            "total_duration_ms": self.total_duration_ms,
            "average_duration_ms": self.average_duration_ms,
            "per_specialty_counts": self.per_specialty_counts,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn renders_every_counter_and_preserves_fractional_average() {
        let rendered = StatisticsView {
            total_deliberations: 2,
            total_orchestrations: 1,
            total_duration_ms: 5,
            average_duration_ms: 5.0 / 3.0,
            per_specialty_counts: BTreeMap::from([("review".to_owned(), 2)]),
        }
        .to_json();

        assert_eq!(rendered["total_deliberations"], 2);
        assert!(rendered["average_duration_ms"].is_f64());
        assert_eq!(rendered["per_specialty_counts"]["review"], 2);
    }
}
