/// One complete Prometheus text exposition snapshot.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PrometheusText(String);

impl PrometheusText {
    #[must_use]
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}
