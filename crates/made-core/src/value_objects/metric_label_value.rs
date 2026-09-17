/// A Prometheus label value.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MetricLabelValue(String);

impl MetricLabelValue {
    #[must_use]
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}
