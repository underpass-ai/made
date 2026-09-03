use std::fmt;

/// An audit-journal property an adapter failed to satisfy.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConformanceFailure {
    property: &'static str,
    detail: String,
}

impl ConformanceFailure {
    pub(super) fn new(property: &'static str, detail: impl Into<String>) -> Self {
        Self {
            property,
            detail: detail.into(),
        }
    }

    #[must_use]
    pub fn property(&self) -> &'static str {
        self.property
    }

    #[must_use]
    pub fn detail(&self) -> &str {
        &self.detail
    }
}

impl fmt::Display for ConformanceFailure {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "audit journal conformance failed: {} — {}",
            self.property, self.detail
        )
    }
}

impl std::error::Error for ConformanceFailure {}
