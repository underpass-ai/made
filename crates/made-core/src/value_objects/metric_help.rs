/// Human-readable Prometheus family help text.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MetricHelp(String);

impl MetricHelp {
    #[must_use]
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}
